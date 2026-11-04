//! Display identities and physical foreground bounds. Never changes system topology.
use island_core::{
    display::Rect,
    display::{Foreground, Monitor},
    error::{AppError, AppResult},
};
use windows::{
    core::{BOOL, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::*,
        },
        UI::{Shell::GetScaleFactorForMonitor, WindowsAndMessaging::*},
    },
};

fn error(message: &str) -> AppError {
    AppError::new("window_error", message)
}
fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(
        &units[..units
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(units.len())],
    )
}
fn rect(value: RECT) -> Rect {
    Rect {
        left: value.left,
        top: value.top,
        right: value.right,
        bottom: value.bottom,
    }
}
struct Enumeration {
    monitors: Vec<Monitor>,
    error: Option<AppError>,
}
unsafe extern "system" fn enumerate_monitor(
    handle: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let enumeration = &mut *(data.0 as *mut Enumeration);
    let result = (|| -> AppResult<Monitor> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(handle, (&mut info as *mut MONITORINFOEXW).cast()).as_bool() {
            return Err(error("Die Bildschirmgrenzen konnten nicht gelesen werden."));
        }
        let device = text(&info.szDevice);
        let mut aliases = Vec::new();
        let mut names = Vec::new();
        for index in 0..64 {
            let mut display = DISPLAY_DEVICEW {
                cb: size_of::<DISPLAY_DEVICEW>() as u32,
                ..Default::default()
            };
            if !EnumDisplayDevicesW(
                PCWSTR(info.szDevice.as_ptr()),
                index,
                &mut display,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
            .as_bool()
            {
                break;
            }
            if display.StateFlags.0 & DISPLAY_DEVICE_ACTIVE.0 != 0 {
                let id = text(&display.DeviceID).to_lowercase();
                if !id.is_empty() {
                    aliases.push(id);
                }
                let name = text(&display.DeviceString);
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
        aliases.sort();
        aliases.dedup();
        names.sort();
        names.dedup();
        let id = aliases
            .first()
            .cloned()
            .ok_or_else(|| error("Eine stabile Bildschirm-Gerätekennung ist nicht verfügbar."))?;
        let scale = GetScaleFactorForMonitor(handle)
            .map_err(|_| error("Die Bildschirmskalierung konnte nicht gelesen werden."))?
            .0 as f64
            / 100.0;
        let monitor = Monitor {
            id,
            aliases,
            name: if names.is_empty() {
                "Bildschirm".into()
            } else {
                names.join(" / ")
            },
            device,
            bounds: rect(info.monitorInfo.rcMonitor),
            work_area: rect(info.monitorInfo.rcWork),
            scale,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        };
        monitor.validate()?;
        Ok(monitor)
    })();
    match result {
        Ok(monitor) => enumeration.monitors.push(monitor),
        Err(error) => {
            enumeration.error = Some(error);
            return false.into();
        }
    }
    true.into()
}
pub fn monitors() -> AppResult<Vec<Monitor>> {
    let _dpi = crate::window::DpiContext::physical()?;
    let mut enumeration = Enumeration {
        monitors: vec![],
        error: None,
    };
    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enumerate_monitor),
            LPARAM(&mut enumeration as *mut Enumeration as isize),
        )
    };
    if let Some(error) = enumeration.error {
        return Err(error);
    }
    if !success.as_bool() || enumeration.monitors.is_empty() {
        return Err(error("Keine aktiven Bildschirme verfügbar."));
    }
    enumeration
        .monitors
        .sort_by(|a, b| b.primary.cmp(&a.primary).then(a.id.cmp(&b.id)));
    Ok(enumeration.monitors)
}
pub fn monitor_id_for_device(device: &str) -> AppResult<String> {
    monitors()?
        .into_iter()
        .find(|monitor| monitor.device.eq_ignore_ascii_case(device))
        .map(|monitor| monitor.id)
        .ok_or_else(|| error("Der Fenstermonitor ist nicht mehr verbunden."))
}
/// No titles, process arguments, or preview pixels are collected.
pub fn foreground(window: HWND, excluded_pid: u32) -> AppResult<Foreground> {
    let _dpi = crate::window::DpiContext::physical()?;
    if !unsafe { IsWindow(Some(window)) }.as_bool() {
        return Ok(Foreground::default());
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    let mut class = [0u16; 256];
    let length = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
    let class = String::from_utf16_lossy(&class[..length]);
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) } as u32;
    let extended = unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32;
    let mut cloaked = 0u32;
    let cloak_known = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok();
    let mut bounds = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut bounds as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .map_err(|_| error("Die sichtbaren Fenstergrenzen konnten nicht gelesen werden."))?;
    Ok(Foreground {
        bounds: rect(bounds),
        visible: unsafe { IsWindowVisible(window) }.as_bool(),
        minimized: unsafe { IsIconic(window) }.as_bool(),
        maximized: unsafe { IsZoomed(window) }.as_bool(),
        decorated: style & (WS_CAPTION.0 | WS_THICKFRAME.0) != 0,
        excluded: pid == excluded_pid
            || !cloak_known
            || cloaked != 0
            || unsafe { GetAncestor(window, GA_ROOT) } != window
            || style & WS_CHILD.0 != 0
            || extended & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) != 0
            || [
                "Progman",
                "WorkerW",
                "Shell_TrayWnd",
                "Shell_SecondaryTrayWnd",
            ]
            .contains(&class.as_str()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_monitor_device_interfaces_and_physical_bounds_are_stable() {
        let first = monitors().unwrap();
        let second = monitors().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.iter().filter(|m| m.primary).count(), 1);
        for monitor in &first {
            assert!(!monitor.id.eq_ignore_ascii_case(&monitor.device));
            monitor.validate().unwrap();
        }
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/evidence/native");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("monitors.json"),
            serde_json::to_vec_pretty(&first).unwrap(),
        )
        .unwrap();
    }
}
