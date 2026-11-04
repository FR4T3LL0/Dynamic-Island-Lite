//! Short-lived UI-thread geometry transitions. No worker owns an HWND or pumps input.
use island_core::error::{AppError, AppResult};
use serde::Serialize;
use serde_json::{json, Value};
use std::{cell::RefCell, collections::VecDeque, time::Instant};
use tauri::{Emitter, Manager, WebviewWindow};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, RECT},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{
            GetWindowRect, GetWindowThreadProcessId, KillTimer, SetTimer, SetWindowPos,
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
            SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        },
    },
};

// Window-local ID, separate from framework timers. Killed after every transition.
const TIMER: usize = 0x4449_534c;
// Request the minimum WM_TIMER interval. On a coarse Windows timer, asking
// for 16 ms can skip a ~15.6 ms tick and delay the next frame by another tick.
const FRAME_INTERVAL_MS: u32 = 10;

#[derive(Clone, Copy)]
pub enum Curve {
    Settle,
    Spring,
}

impl Curve {
    fn progress(self, fraction: f64) -> f64 {
        if fraction <= 0.0 {
            return 0.0;
        }
        if fraction >= 1.0 {
            return 1.0;
        }
        match self {
            Self::Settle => {
                // Critically damped, normalized to reach the exact target.
                let t = fraction * 7.0;
                (1.0 - (1.0 + t) * (-t).exp()) / (1.0 - 8.0 * (-7.0_f64).exp())
            }
            Self::Spring => {
                let t = fraction * 10.0;
                let damping = 0.73_f64;
                let frequency = (1.0 - damping * damping).sqrt();
                let (sin, cos) = (frequency * t).sin_cos();
                1.0 - (-damping * t).exp() * (cos + damping / frequency * sin)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Frame {
    fn between(self, target: Self, progress: f64) -> Self {
        let distance = (target.width - self.width)
            .abs()
            .max((target.height - self.height).abs())
            .max((target.x - self.x).abs())
            .max((target.y - self.y).abs());
        // Ease into a six-pixel bound without flattening the top of the spring.
        let limit = 6.0 / f64::from(distance.max(1));
        let progress = if progress > 1.0 {
            1.0 - limit * (-(progress - 1.0) / limit).exp_m1()
        } else {
            progress
        };
        let width = interpolate(self.width, target.width, progress).max(1);
        // Round width and position around one physical pixel anchor. Rounding
        // the left edge separately makes the center alternate on odd widths.
        let center = interpolate(
            self.x + self.width / 2,
            target.x + target.width / 2,
            progress.min(1.0),
        );
        Self {
            x: center - width / 2,
            // The upper edge may be flush with the monitor or its AppBar.
            y: interpolate(self.y, target.y, progress.min(1.0)),
            width,
            height: interpolate(self.height, target.height, progress).max(1),
        }
    }
}

fn interpolate(from: i32, target: i32, progress: f64) -> i32 {
    (from as f64 + (target as f64 - from as f64) * progress).round() as i32
}
struct Motion {
    id: u64,
    window: WebviewWindow,
    hwnd: HWND,
    from: Frame,
    target: Frame,
    from_radius: i32,
    radius: i32,
    notch: bool,
    started: Instant,
    duration_ms: u32,
    curve: Curve,
    pointer_start: Option<(i32, i32)>,
}
#[derive(Default)]
struct Slot {
    generation: u64,
    ticks: u64,
    motion: Option<Motion>,
    radius: Option<i32>,
    trace: VecDeque<Value>,
}
thread_local! { static SLOT: RefCell<Slot> = RefCell::new(Slot::default()); }

fn error(error: impl std::fmt::Display) -> AppError {
    AppError::new("window_error", error.to_string())
}
fn own_ui(hwnd: HWND) -> AppResult<()> {
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid != std::process::id() || thread != unsafe { GetCurrentThreadId() } {
        return Err(AppError::new(
            "access_denied",
            "Die Inselgröße darf nur ihr eigener UI-Thread ändern.",
        ));
    }
    Ok(())
}
fn frame(hwnd: HWND) -> AppResult<Frame> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(error)?;
    Ok(Frame {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    })
}
pub fn system_animations_enabled() -> bool {
    let mut enabled = BOOL(0);
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok()
        && enabled.as_bool()
}
fn trace(value: Value) {
    if crate::diagnostic_root().is_none() {
        return;
    }
    SLOT.with_borrow_mut(|slot| {
        if slot.trace.len() == 512 {
            slot.trace.pop_front();
        }
        slot.trace.push_back(value);
    });
}
fn resize(hwnd: HWND, bounds: Frame) -> AppResult<()> {
    own_ui(hwnd)?;
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER,
        )
    }
    .map_err(error)
}
fn apply(hwnd: HWND, bounds: Frame, radius: i32, notch: bool) -> AppResult<()> {
    resize(hwnd, bounds)?;
    let radius = radius.min(bounds.width.min(bounds.height) / 2).max(0);
    // Resize handlers may reset a custom region; install the matching region last.
    island_win::shell::apply_region(hwnd, bounds.width, bounds.height, radius, notch)?;
    SLOT.with_borrow_mut(|slot| slot.radius = Some(radius));
    Ok(())
}
pub fn show_activity_bubbles(
    hwnd: HWND,
    bounds: Frame,
    radius: i32,
    notch: bool,
    regions: island_core::display::ActivityRegions,
) -> AppResult<()> {
    own_ui(hwnd)?;
    cancel();
    resize(hwnd, bounds)?;
    island_win::shell::apply_activity_region(
        hwnd,
        bounds.width,
        bounds.height,
        radius,
        notch,
        regions,
    )?;
    SLOT.with_borrow_mut(|slot| slot.radius = Some(radius));
    Ok(())
}
fn trace_frame(motion: &Motion, elapsed_ms: f64) {
    if crate::diagnostic_root().is_none() {
        return;
    }
    let Ok(bounds) = frame(motion.hwnd) else {
        return;
    };
    let (valid, rect, method, attempts) = island_win::shell::region_bounds(motion.hwnd)
        .map(|region| (true, region.bounds, region.method, region.attempts))
        .unwrap_or((false, RECT::default(), "unavailable", 3));
    let clock = motion
        .window
        .get_webview_window("clock-overlay")
        .filter(|window| window.is_visible().unwrap_or(false))
        .and_then(|window| frame(window.hwnd().ok()?).ok());
    trace(
        json!({"event":"frame","generation":motion.id,"elapsedMs":elapsed_ms,"bounds":bounds,"clock":clock,
        "region":{"valid":valid,"left":rect.left,"top":rect.top,"right":rect.right,"bottom":rect.bottom,"method":method,"readAttempts":attempts},"notch":motion.notch}),
    );
}
fn pointer() -> AppResult<(i32, i32)> {
    let mut point = windows::Win32::Foundation::POINT::default();
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetPhysicalCursorPos(&mut point) }
        .map_err(error)?;
    Ok((point.x, point.y))
}
fn marker(window: &WebviewWindow, id: u64, active: bool, pointer_moved: bool) {
    // A stale completion cannot clear a newer transition's marker.
    let script = if active {
        format!("document.documentElement.dataset.shellMotionPointerMoved='false';document.documentElement.dataset.shellMotion='{id}'")
    } else {
        format!("if(document.documentElement.dataset.shellMotion==='{id}'){{document.documentElement.dataset.shellMotionPointerMoved='{pointer_moved}';delete document.documentElement.dataset.shellMotion}}")
    };
    let _ = window.eval(script);
}
fn end(motion: &Motion, reason: &str) {
    if own_ui(motion.hwnd).is_ok() {
        let _ = unsafe { KillTimer(Some(motion.hwnd), TIMER) };
    }
    let pointer_moved = motion
        .pointer_start
        .zip(pointer().ok())
        .is_some_and(|(before, after)| before != after);
    marker(&motion.window, motion.id, false, pointer_moved);
    trace(
        json!({"event":"end","generation":motion.id,"reason":reason,"elapsedMs":motion.started.elapsed().as_secs_f64()*1000.0}),
    );
}
pub fn cancel() {
    let previous = SLOT.with_borrow_mut(|slot| {
        slot.generation = slot.generation.wrapping_add(1);
        slot.motion.take()
    });
    if let Some(previous) = previous {
        end(&previous, "cancelled");
    }
}
pub fn start(
    window: WebviewWindow,
    target: Frame,
    radius: i32,
    notch: bool,
    duration_ms: u32,
    curve: Curve,
) -> AppResult<()> {
    let hwnd = window.hwnd().map_err(error)?;
    own_ui(hwnd)?;
    if duration_ms > 0
        && SLOT.with_borrow(|slot| {
            slot.motion.as_ref().is_some_and(|motion| {
                motion.target == target && motion.radius == radius && motion.notch == notch
            })
        })
    {
        return Ok(());
    }
    cancel();
    let from = frame(hwnd)?;
    let (id, from_radius) =
        SLOT.with_borrow(|slot| (slot.generation, slot.radius.unwrap_or(radius)));
    let motion = Motion {
        id,
        window,
        hwnd,
        from,
        target,
        from_radius,
        radius,
        notch,
        started: Instant::now(),
        duration_ms,
        curve,
        pointer_start: pointer().ok(),
    };
    trace(
        json!({"event":"start","generation":id,"from":from,"target":target,"durationMs":duration_ms}),
    );
    if duration_ms == 0 || (from == target && from_radius == radius) {
        apply(hwnd, target, radius, notch)?;
        trace_frame(&motion, 0.0);
        end(&motion, "instant");
        return Ok(());
    }
    apply(hwnd, from, from_radius, notch)?;
    trace_frame(&motion, 0.0);
    let timer = unsafe { SetTimer(Some(hwnd), TIMER, FRAME_INTERVAL_MS, Some(timer_tick)) };
    if timer == 0 {
        apply(hwnd, target, radius, notch)?;
        return Err(error(windows::core::Error::from_thread()));
    }
    marker(&motion.window, id, true, false);
    SLOT.with_borrow_mut(|slot| slot.motion = Some(motion));
    Ok(())
}
fn tick(hwnd: HWND) -> AppResult<()> {
    let Some(motion) = SLOT.with_borrow_mut(|slot| slot.motion.take()) else {
        return Ok(());
    };
    if hwnd != motion.hwnd {
        SLOT.with_borrow_mut(|slot| slot.motion = Some(motion));
        return Ok(());
    }
    let elapsed_ms = motion.started.elapsed().as_secs_f64() * 1000.0;
    let done = elapsed_ms >= motion.duration_ms as f64 || !system_animations_enabled();
    let fraction = if done {
        1.0
    } else {
        elapsed_ms / motion.duration_ms as f64
    };
    let progress = motion.curve.progress(fraction);
    let bounds = motion.from.between(motion.target, progress);
    let radius = interpolate(motion.from_radius, motion.radius, progress.min(1.0));
    if let Err(error) = apply(hwnd, bounds, radius, motion.notch) {
        end(&motion, "failed");
        return Err(error);
    }
    trace_frame(&motion, elapsed_ms);
    let (generation, occupied) = SLOT.with_borrow(|slot| (slot.generation, slot.motion.is_some()));
    if generation != motion.id {
        if !occupied {
            end(&motion, "superseded");
        }
        return Ok(());
    }
    if done {
        end(&motion, "finished");
    } else {
        // Native window messages may reenter us; never restore a superseded target.
        SLOT.with_borrow_mut(|slot| {
            if slot.generation == motion.id && slot.motion.is_none() {
                slot.motion = Some(motion);
            }
        });
    }
    Ok(())
}
unsafe extern "system" fn timer_tick(hwnd: HWND, _: u32, id: usize, _: u32) {
    if id != TIMER {
        return;
    }
    SLOT.with_borrow_mut(|slot| slot.ticks = slot.ticks.wrapping_add(1));
    let window = SLOT.with_borrow(|slot| slot.motion.as_ref().map(|motion| motion.window.clone()));
    let result = std::panic::catch_unwind(|| tick(hwnd));
    let failure = match result {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error,
        Err(_) => AppError::new("window_error", "Der Größenwechsel wurde abgebrochen."),
    };
    cancel();
    if own_ui(hwnd).is_ok() {
        let _ = unsafe { KillTimer(Some(hwnd), TIMER) };
    }
    // Reporting is best effort; the callback must never unwind across the Windows ABI.
    if let Some(window) = window {
        let _ = window.eval("delete document.documentElement.dataset.shellMotion");
        let _ = window.emit("service-error", failure);
    }
}
pub fn probe(reset: bool, hwnd: HWND) -> AppResult<Value> {
    own_ui(hwnd)?;
    let bounds = frame(hwnd)?;
    let mut cursor = windows::Win32::Foundation::POINT::default();
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetPhysicalCursorPos(&mut cursor) }
        .map_err(|e| AppError::new("io_error", e.to_string()))?;
    Ok(SLOT.with_borrow_mut(|slot| {
        let result = json!({"active":slot.motion.is_some(),"generation":slot.generation,"ticks":slot.ticks,"bounds":bounds,"cursor":{"x":cursor.x,"y":cursor.y},"systemAnimationsEnabled":system_animations_enabled(),"trace":slot.trace});
        if reset { slot.trace.clear(); }
        result
    }))
}

#[cfg(test)]
mod tests {
    use super::{Curve, Frame};
    #[test]
    fn geometry_eases_monotonically_without_overshoot_and_handles_negative_origins() {
        let from = Frame {
            x: -700,
            y: -100,
            width: 180,
            height: 36,
        };
        let target = Frame {
            x: -850,
            y: -88,
            width: 480,
            height: 320,
        };
        assert_eq!(from.between(target, 0.0), from);
        assert_eq!(from.between(target, 1.0), target);
        let mut previous = from;
        for index in 1..=100 {
            let next = from.between(target, Curve::Settle.progress(index as f64 / 100.0));
            assert!(next.width >= previous.width && next.width <= target.width);
            assert!(next.height >= previous.height && next.height <= target.height);
            assert!(next.x <= previous.x && next.x >= target.x);
            previous = next;
        }
        assert_eq!(target.between(from, Curve::Settle.progress(1.1)), from);
        let frames: Vec<_> = (0..=100)
            .map(|index| from.between(target, Curve::Spring.progress(index as f64 / 100.0)))
            .collect();
        assert!(frames.iter().any(|frame| frame.width > target.width));
        assert!(frames
            .iter()
            .all(|frame| frame.width <= target.width + 6 && frame.y <= target.y));
        assert_eq!(frames[100], target);
        assert!(frames.iter().all(|frame| frame.x + frame.width / 2 == -610));
    }
}
