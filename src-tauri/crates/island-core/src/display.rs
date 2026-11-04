//! Physical desktop coordinates and display selection, independent of Win32 handles.
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    pub aliases: Vec<String>,
    pub name: String,
    pub device: String,
    pub bounds: Rect,
    pub work_area: Rect,
    pub scale: f64,
    pub primary: bool,
}
impl Monitor {
    pub fn matches(&self, saved: &str) -> bool {
        self.id.eq_ignore_ascii_case(saved)
            || self.aliases.iter().any(|id| id.eq_ignore_ascii_case(saved))
            || self.device.eq_ignore_ascii_case(saved)
    }
    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty()
            || !self.scale.is_finite()
            || !(0.5..=8.0).contains(&self.scale)
            || !valid(self.bounds)
            || !valid(self.work_area)
            || !contains(self.bounds, self.work_area)
        {
            return Err(AppError::new(
                "invalid_input",
                "Die Bildschirmdaten sind ungültig.",
            ));
        }
        Ok(())
    }
}
pub fn select<'a>(
    monitors: &'a [Monitor],
    requested: Option<&str>,
) -> AppResult<(&'a Monitor, bool)> {
    if let Some(selected) =
        requested.and_then(|id| monitors.iter().find(|monitor| monitor.matches(id)))
    {
        selected.validate()?;
        return Ok((selected, false));
    }
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or(monitors.first())
        .ok_or_else(|| AppError::new("not_found", "Kein Bildschirm verfügbar."))?;
    monitor.validate()?;
    Ok((monitor, requested.is_some()))
}
fn valid(rect: Rect) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
}
pub fn contains(outer: Rect, inner: Rect) -> bool {
    valid(inner)
        && inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right <= outer.right
        && inner.bottom <= outer.bottom
}
/// Round only physical offsets/sizes; never scale a virtual desktop origin.
pub fn island_rect(monitor: &Monitor, width: f64, height: f64, gap: f64) -> AppResult<Rect> {
    monitor.validate()?;
    if ![width, height, gap].iter().all(|value| value.is_finite())
        || width <= 0.0
        || height <= 0.0
        || gap < 0.0
    {
        return Err(AppError::new(
            "invalid_input",
            "Die Inselgröße ist ungültig.",
        ));
    }
    let work = monitor.work_area;
    let available_w = (work.right as i64 - work.left as i64) as f64;
    let available_h = (work.bottom as i64 - work.top as i64) as f64;
    let scale = monitor.scale;
    let top = (gap * scale)
        .round()
        .min((available_h - 36.0 * scale).max(0.0));
    let w = (width * scale)
        .round()
        .min((available_w - 32.0 * scale).max(1.0))
        .max(1.0);
    let h = (height * scale)
        .round()
        .min(
            (available_h - 32.0 * scale)
                .min(available_h - top - 12.0 * scale)
                .max(1.0),
        )
        .max(1.0);
    let left = work.left as i64 + ((available_w - w) / 2.0).round() as i64;
    let top = work.top as i64 + top as i64;
    let result = Rect {
        left: left as i32,
        top: top as i32,
        right: (left + w as i64) as i32,
        bottom: (top + h as i64) as i32,
    };
    if !contains(work, result) {
        return Err(AppError::new(
            "invalid_input",
            "Die Insel liegt außerhalb der Arbeitsfläche.",
        ));
    }
    Ok(result)
}

#[derive(Clone, Copy)]
pub struct ActivityRegions {
    pub main_left: i32,
    pub main_right: i32,
    pub gap: i32,
}

pub struct ActivityBubbleLayout {
    pub window: Rect,
    pub regions: ActivityRegions,
}

/// Reserve equal space around the main island so its center does not move.
pub fn media_bubble_layout(
    monitor: &Monitor,
    width: f64,
    gap: f64,
) -> AppResult<ActivityBubbleLayout> {
    let mut window = island_rect(monitor, width + 88.0, 36.0, gap)?;
    let bubble_gap = (8.0 * monitor.scale).round() as i32;
    let extension = (36.0 * monitor.scale).round() as i32 + bubble_gap;
    let main_width = window.right - window.left - 2 * extension;
    if main_width <= 0 {
        return Err(AppError::new(
            "invalid_input",
            "Für die Aktivitätsblasen fehlt Platz.",
        ));
    }
    window.left += extension;
    Ok(ActivityBubbleLayout {
        window,
        regions: ActivityRegions {
            main_left: 0,
            main_right: main_width,
            gap: bubble_gap,
        },
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Foreground {
    pub bounds: Rect,
    pub visible: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub decorated: bool,
    pub excluded: bool,
}
pub fn covers_island(window: Foreground, monitor: &Monitor, island: Rect) -> bool {
    let bounds = window.bounds;
    if !window.visible || window.minimized || window.excluded || !valid(bounds) {
        return false;
    }
    let center_x = (i64::from(bounds.left) + i64::from(bounds.right)) / 2;
    let center_y = (i64::from(bounds.top) + i64::from(bounds.bottom)) / 2;
    let screen = monitor.bounds;
    let maximized_here = window.maximized
        && (i64::from(screen.left)..i64::from(screen.right)).contains(&center_x)
        && (i64::from(screen.top)..i64::from(screen.bottom)).contains(&center_y);
    maximized_here
        || (bounds.left < island.right
            && bounds.right > island.left
            && bounds.top < island.bottom
            && bounds.bottom > island.top)
}
/// A maximized framed application is not a borderless fullscreen application.
pub fn fullscreen(window: Foreground, monitor: &Monitor) -> bool {
    if !window.visible || window.minimized || window.excluded || window.decorated {
        return false;
    }
    let a = window.bounds;
    let b = monitor.bounds;
    valid(a)
        && [
            a.left as i64 - b.left as i64,
            a.top as i64 - b.top as i64,
            a.right as i64 - b.right as i64,
            a.bottom as i64 - b.bottom as i64,
        ]
        .iter()
        .all(|delta| delta.abs() <= 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn monitor(id: &str, x: i32, y: i32, scale: f64) -> Monitor {
        Monitor {
            id: id.into(),
            aliases: vec![],
            name: "Anzeige".into(),
            device: format!("GDI-{id}"),
            bounds: Rect {
                left: x,
                top: y,
                right: x + 1920,
                bottom: y + 1080,
            },
            work_area: Rect {
                left: x,
                top: y,
                right: x + 1920,
                bottom: y + 1040,
            },
            scale,
            primary: id == "primary",
        }
    }
    #[test]
    fn scaled_sizes_preserve_negative_origins_and_fit_small_work_areas() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for (x, y) in [(0, 0), (-1920, 0), (0, -1080), (-1920, -1080)] {
                let m = monitor("primary", x, y, scale);
                let rect = island_rect(&m, 480.0, 320.0, 12.0).unwrap();
                assert_eq!(rect.right - rect.left, (480.0 * scale) as i32);
                assert_eq!(rect.top, y + (12.0 * scale) as i32);
                assert_eq!(rect.left, x + (1920 - (480.0 * scale) as i32) / 2);
                assert!(contains(m.work_area, rect));
                let mut small = m;
                small.bounds = Rect {
                    left: x,
                    top: y,
                    right: x + 320,
                    bottom: y + 240,
                };
                small.work_area = small.bounds;
                for (w, h, gap) in [
                    (180.0, 36.0, 48.0),
                    (200.0, 36.0, 0.0),
                    (320.0, 104.0, 48.0),
                    (480.0, 320.0, 48.0),
                ] {
                    assert!(contains(
                        small.work_area,
                        island_rect(&small, w, h, gap).unwrap()
                    ));
                }
            }
        }
    }
    #[test]
    fn activity_bubbles_keep_the_island_centered_and_scale_the_gaps() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let m = monitor("other", -1920, -1080, scale);
            let base = island_rect(&m, 180.0, 36.0, 12.0).unwrap();
            let extension = (8.0 * scale).round() as i32 + (36.0 * scale).round() as i32;
            let layout = media_bubble_layout(&m, 180.0, 12.0).unwrap();
            let regions = layout.regions;
            assert!((layout.window.left + regions.main_left - base.left).abs() <= 1);
            assert_eq!(layout.window.top, base.top);
            assert_eq!(
                regions.main_right - regions.main_left,
                base.right - base.left
            );
            assert_eq!(regions.gap, (8.0 * scale).round() as i32);
            assert_eq!(regions.main_left, 0);
            assert_eq!(
                layout.window.right - layout.window.left - regions.main_right,
                extension
            );
            assert!(contains(m.work_area, layout.window));
        }
    }
    #[test]
    fn device_identity_survives_reordering_hotplug_and_primary_fallback() {
        let primary = monitor("primary", 0, 0, 1.0);
        let mut target = monitor("device-path", -1920, 0, 1.5);
        target.aliases.push("clone-device".into());
        let saved = "DEVICE-PATH";
        assert_eq!(
            select(&[target.clone(), primary.clone()], Some(saved))
                .unwrap()
                .0
                .id,
            "device-path"
        );
        assert_eq!(
            select(std::slice::from_ref(&primary), Some(saved))
                .unwrap()
                .0
                .id,
            "primary"
        );
        assert!(
            select(std::slice::from_ref(&primary), Some(saved))
                .unwrap()
                .1
        );
        assert_eq!(
            select(&[primary, target.clone()], Some(saved))
                .unwrap()
                .0
                .id,
            "device-path"
        );
        assert!(!select(&[target.clone()], Some("clone-device")).unwrap().1);
        assert!(!select(&[target.clone()], Some(&target.device)).unwrap().1);
        assert!(select(&[], Some(saved)).is_err());
    }
    #[test]
    fn fullscreen_is_monitor_specific_and_excludes_desktop_minimized_and_maximized_frames() {
        let a = monitor("primary", 0, 0, 1.0);
        let b = monitor("other", -1920, 0, 1.5);
        let full = Foreground {
            bounds: a.bounds,
            visible: true,
            ..Default::default()
        };
        assert!(fullscreen(full, &a));
        assert!(!fullscreen(full, &b));
        for bad in [
            Foreground {
                decorated: true,
                maximized: true,
                ..full
            },
            Foreground {
                minimized: true,
                ..full
            },
            Foreground {
                visible: false,
                ..full
            },
            Foreground {
                excluded: true,
                ..full
            },
            Foreground {
                bounds: a.work_area,
                ..full
            },
        ] {
            assert!(!fullscreen(bad, &a));
        }
        let mut exact = full;
        exact.bounds.left -= 2;
        assert!(fullscreen(exact, &a));
        exact.bounds.left -= 1;
        assert!(!fullscreen(exact, &a));
    }
    #[test]
    fn smart_uses_visible_overlap_or_maximization_on_the_selected_monitor() {
        let selected = monitor("other", -1920, 0, 1.5);
        let island = island_rect(&selected, 180.0, 36.0, 12.0).unwrap();
        let mut window = Foreground {
            bounds: island,
            visible: true,
            decorated: true,
            ..Default::default()
        };
        assert!(covers_island(window, &selected, island));
        window.bounds.top = island.bottom;
        window.bounds.bottom = island.bottom + 500;
        assert!(!covers_island(window, &selected, island));
        window.maximized = true;
        assert!(covers_island(window, &selected, island));
        assert!(!covers_island(
            Foreground {
                minimized: true,
                ..window
            },
            &selected,
            island
        ));
        assert!(!covers_island(
            Foreground {
                excluded: true,
                ..window
            },
            &selected,
            island
        ));
        window.bounds = monitor("primary", 0, 0, 1.0).work_area;
        assert!(!covers_island(window, &selected, island));
    }
    #[test]
    fn invalid_layouts_are_errors_instead_of_nan_or_offscreen_coordinates() {
        let mut m = monitor("primary", 0, 0, 1.0);
        for scale in [0.0, f64::NAN, f64::INFINITY] {
            m.scale = scale;
            assert!(island_rect(&m, 480.0, 320.0, 12.0).is_err());
        }
        m.scale = 1.0;
        assert!(island_rect(&m, f64::NAN, 320.0, 12.0).is_err());
        m.work_area.right = m.bounds.right + 1;
        assert!(m.validate().is_err());
    }
    #[test]
    fn small_surface_includes_top_gap_and_keeps_both_shapes_inside() {
        let mut m = monitor("primary", 0, 0, 1.0);
        m.bounds = Rect {
            left: 0,
            top: 0,
            right: 320,
            bottom: 240,
        };
        m.work_area = m.bounds;
        assert_eq!(
            island_rect(&m, 480.0, 320.0, 48.0).unwrap(),
            Rect {
                left: 16,
                top: 48,
                right: 304,
                bottom: 228
            }
        );
        assert_eq!(
            island_rect(&m, 480.0, 320.0, 0.0).unwrap(),
            Rect {
                left: 16,
                top: 0,
                right: 304,
                bottom: 208
            }
        );
        for (width, height) in [(100, 80), (32, 36), (10, 10)] {
            m.bounds = Rect {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            m.work_area = m.bounds;
            assert!(contains(
                m.work_area,
                island_rect(&m, 480.0, 320.0, 48.0).unwrap()
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
