use island_core::{
    display::ActivityRegions,
    error::{AppError, AppResult},
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, IsWindow, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, GdiFlush, GetRgnBox,
        GetWindowRgn, GetWindowRgnBox, PtInRegion, SetWindowRgn, HRGN, RGN_OR,
    },
};

pub struct RegionBounds {
    pub bounds: RECT,
    pub method: &'static str,
    pub attempts: u8,
}
fn copied_region<T>(hwnd: HWND, inspect: impl Fn(HRGN) -> T) -> Option<T> {
    unsafe {
        // Keep a rejected destination alive so a retry gets a distinct object.
        let mut copies = [HRGN::default(); 2];
        let mut result = None;
        for copy in &mut copies {
            *copy = CreateRectRgn(0, 0, 0, 0);
            if copy.is_invalid() {
                break;
            }
            let _ = GdiFlush();
            if GetWindowRgn(hwnd, *copy).0 != 0 {
                result = Some(inspect(*copy));
                break;
            }
        }
        for copy in copies {
            if !copy.is_invalid() {
                let _ = DeleteObject(copy.into());
            }
        }
        result
    }
}
/// Read actual OS data only; fallback does not rewrite or infer a window shape.
pub fn region_bounds(hwnd: HWND) -> Option<RegionBounds> {
    // ERROR does not distinguish an absent region from a failed OS read.
    // Retry only the bounded read; never infer a rectangle or rewrite the shape.
    for attempt in 1..=3 {
        if let Some(mut region) = read_region_bounds(hwnd) {
            region.attempts = attempt;
            return Some(region);
        }
    }
    None
}
fn read_region_bounds(hwnd: HWND) -> Option<RegionBounds> {
    let mut bounds = RECT::default();
    unsafe {
        let _ = GdiFlush();
    }
    if unsafe { GetWindowRgnBox(hwnd, &mut bounds) }.0 != 0 {
        return Some(RegionBounds {
            bounds,
            method: "GetWindowRgnBox",
            attempts: 1,
        });
    }
    copied_region(hwnd, |copy| {
        let mut bounds = RECT::default();
        (unsafe { GetRgnBox(copy, &mut bounds) }.0 != 0).then_some(bounds)
    })
    .flatten()
    .map(|bounds| RegionBounds {
        bounds,
        method: "GetWindowRgn/GetRgnBox",
        attempts: 1,
    })
}

/// Coordinates are physical pixels relative to the outer window rectangle.
/// Without a valid form region, accepting a drop would guess at the visible target.
pub fn region_contains(hwnd: HWND, x: i32, y: i32) -> AppResult<bool> {
    // A resize can replace/reset the OS region between two separate reads.
    // Inspect one actual copy instead of checking bounds and then recopying.
    // An unavailable region is never a hit. The next drag tick may retry;
    // do not permanently cancel a live move because of a transient OS read.
    Ok(copied_region(hwnd, |copy| unsafe { PtInRegion(copy, x, y).as_bool() }).unwrap_or(false))
}

/// Keep the intentionally focusable overlay out of Alt+Tab and the taskbar.
pub fn apply_overlay_style(hwnd: HWND) -> AppResult<()> {
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(AppError::new(
                "not_found",
                "Das Inselfenster ist nicht mehr vorhanden.",
            ));
        }
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = (current & !(WS_EX_APPWINDOW.0 as isize)) | WS_EX_TOOLWINDOW.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) != next {
            return Err(AppError::new(
                "io_error",
                "Die Overlay-Eigenschaften konnten nicht gesetzt werden.",
            ));
        }
        // The TOPMOST style bit alone does not establish the actual Z-order band.
        // Reassert the configured overlay without activating it after framework
        // style changes and after another application window has been restored.
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|e| {
            AppError::new(
                "window_error",
                format!("Die Insel konnte nicht im Vordergrund gehalten werden: {e}"),
            )
        })?;
    }
    Ok(())
}

/// The OS region excludes invisible rounded corners from hit testing, not just painting.
/// On success the OS owns the region; on error we release it ourselves.
pub fn apply_region(
    hwnd: HWND,
    width: i32,
    height: i32,
    radius: i32,
    notch: bool,
) -> AppResult<()> {
    apply_regions(hwnd, width, height, radius, notch, None)
}

pub fn apply_activity_region(
    hwnd: HWND,
    width: i32,
    height: i32,
    radius: i32,
    notch: bool,
    regions: ActivityRegions,
) -> AppResult<()> {
    apply_regions(hwnd, width, height, radius, notch, Some(regions))
}

fn apply_regions(
    hwnd: HWND,
    width: i32,
    height: i32,
    radius: i32,
    notch: bool,
    split: Option<ActivityRegions>,
) -> AppResult<()> {
    // SetWindowRgn sends window-position messages. Confirm the final region
    // after their processing; a successful write alone is not a hit-test proof.
    for _ in 0..3 {
        set_region(hwnd, width, height, radius, notch, split)?;
        let region = region_bounds(hwnd);
        if let Some(RegionBounds { bounds, .. }) = region {
            if bounds.left == 0
                && bounds.top == 0
                && (bounds.right as i64 - width as i64).abs() <= 1
                && (bounds.bottom as i64 - height as i64).abs() <= 1
                && split.is_none_or(
                    |ActivityRegions {
                         main_left,
                         main_right,
                         gap,
                     }| {
                        copied_region(hwnd, |shape| unsafe {
                            let hit = |x| PtInRegion(shape, x, height / 2).as_bool();
                            hit((main_left + main_right) / 2)
                                && (main_left == 0
                                    || (hit((main_left - gap) / 2) && !hit(main_left - gap / 2)))
                                && (main_right == width
                                    || (!hit(main_right + gap / 2)
                                        && hit((main_right + gap + width) / 2)))
                        }) == Some(true)
                    },
                )
            {
                return Ok(());
            }
        }
    }
    Err(AppError::new(
        "window_error",
        "Windows hat die passende Fensterform nicht bestätigt.",
    ))
}
fn set_region(
    hwnd: HWND,
    width: i32,
    height: i32,
    radius: i32,
    notch: bool,
    split: Option<ActivityRegions>,
) -> AppResult<()> {
    if width <= 0 || height <= 0 || radius < 0 {
        return Err(AppError::new("invalid_input", "Ungültige Fenstergröße."));
    }
    let ActivityRegions {
        main_left,
        main_right,
        gap,
    } = split.unwrap_or(ActivityRegions {
        main_left: 0,
        main_right: width,
        gap: 0,
    });
    let radius = radius.min((main_right - main_left).min(height) / 2);
    unsafe {
        let region = CreateRoundRectRgn(
            main_left,
            0,
            main_right + 1,
            height + 1,
            radius * 2,
            radius * 2,
        );
        if region.is_invalid() {
            return Err(AppError::new(
                "io_error",
                "Die Fensterform konnte nicht erzeugt werden.",
            ));
        }
        if notch {
            let top = CreateRectRgn(main_left, 0, main_right, radius);
            if top.is_invalid() {
                let _ = DeleteObject(region.into());
                return Err(AppError::new(
                    "io_error",
                    "Die Notch konnte nicht erzeugt werden.",
                ));
            }
            let combined = CombineRgn(Some(region), Some(region), Some(top), RGN_OR);
            let _ = DeleteObject(top.into());
            if combined.0 == 0 {
                let _ = DeleteObject(region.into());
                return Err(AppError::new(
                    "io_error",
                    "Die Notch konnte nicht verbunden werden.",
                ));
            }
        }
        let bubbles = [(0, main_left - gap), (main_right + gap, width)];
        for (left, right) in bubbles.into_iter().filter(|(left, right)| left < right) {
            let diameter = (right - left).min(height);
            let bubble = CreateRoundRectRgn(left, 0, right + 1, height + 1, diameter, diameter);
            let combined = if bubble.is_invalid() {
                false
            } else {
                let result = CombineRgn(Some(region), Some(region), Some(bubble), RGN_OR);
                let _ = DeleteObject(bubble.into());
                result.0 != 0
            };
            if !combined {
                let _ = DeleteObject(region.into());
                return Err(AppError::new(
                    "io_error",
                    "Die Aktivitätsblase konnte nicht angewendet werden.",
                ));
            }
        }
        // Drain deferred GDI work before transferring this object's ownership
        // to USER32. The handoff must not race a pending graphics operation.
        let _ = GdiFlush();
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(region.into());
            return Err(AppError::new(
                "io_error",
                "Die Fensterform konnte nicht angewendet werden.",
            ));
        }
    }
    Ok(())
}
