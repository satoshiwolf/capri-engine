use glam::Vec2;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    /// Screen pixels as `(left, top, right, bottom)`.
    pub bounds: (i32, i32, i32, i32),
    /// Work-area pixels excluding the taskbar, as `(left, top, right, bottom)`.
    pub work_area: (i32, i32, i32, i32),
    pub is_primary: bool,
}

impl MonitorInfo {
    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.bounds.0 as f32
            && p.x < self.bounds.2 as f32
            && p.y >= self.bounds.1 as f32
            && p.y < self.bounds.3 as f32
    }
}

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    crate::desktop::hooks::enumerate_monitors()
}

pub fn virtual_desktop_bounds() -> (i32, i32, i32, i32) {
    crate::desktop::hooks::virtual_desktop_bounds()
}

pub fn can_cross_border(
    current_idx: usize,
    side: i32,
    y: f32,
    monitors: &[MonitorInfo],
) -> Option<usize> {
    let current = monitors.get(current_idx)?;
    let y_i = y as i32;
    if y_i < current.bounds.1 || y_i >= current.bounds.3 {
        return None;
    }

    for (idx, mon) in monitors.iter().enumerate() {
        if idx == current_idx {
            continue;
        }

        if side < 0 {
            let touches = (mon.bounds.2 - current.bounds.0).abs() <= 10;
            let covers_y = y_i >= mon.bounds.1 && y_i < mon.bounds.3;
            if touches && covers_y {
                return Some(idx);
            }
        } else if side > 0 {
            let touches = (mon.bounds.0 - current.bounds.2).abs() <= 10;
            let covers_y = y_i >= mon.bounds.1 && y_i < mon.bounds.3;
            if touches && covers_y {
                return Some(idx);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(name: &str, bounds: (i32, i32, i32, i32), is_primary: bool) -> MonitorInfo {
        MonitorInfo {
            name: name.to_string(),
            bounds,
            work_area: bounds,
            is_primary,
        }
    }

    #[test]
    fn adjacent_monitors_cross_in_both_directions() {
        let monitors = [
            monitor("left", (-1920, 0, 0, 1080), true),
            monitor("right", (0, 0, 1920, 1080), false),
        ];

        assert_eq!(can_cross_border(0, 1, 500.0, &monitors), Some(1));
        assert_eq!(can_cross_border(1, -1, 500.0, &monitors), Some(0));
    }

    #[test]
    fn separated_monitors_do_not_cross() {
        let monitors = [
            monitor("left", (0, 0, 1920, 1080), true),
            monitor("right", (1931, 0, 3840, 1080), false),
        ];

        assert_eq!(can_cross_border(0, 1, 500.0, &monitors), None);
        assert_eq!(can_cross_border(1, -1, 500.0, &monitors), None);
    }

    #[test]
    fn vertical_overlap_controls_crossing_for_offset_monitors() {
        let monitors = [
            monitor("primary", (0, 0, 1920, 1080), true),
            monitor("offset", (1920, 400, 3840, 1480), false),
        ];

        assert_eq!(can_cross_border(0, 1, 300.0, &monitors), None);
        assert_eq!(can_cross_border(0, 1, 500.0, &monitors), Some(1));
        assert_eq!(can_cross_border(0, 1, 1480.0, &monitors), None);
    }

    #[test]
    fn border_crossing_requires_position_inside_current_monitor() {
        let monitors = [
            monitor("main", (0, 100, 1920, 1080), true),
            monitor("neighbor", (1920, 0, 3840, 900), false),
        ];

        assert_eq!(can_cross_border(0, 1, 50.0, &monitors), None);
        assert_eq!(can_cross_border(0, 1, 500.0, &monitors), Some(1));
        assert_eq!(can_cross_border(0, 1, 1080.0, &monitors), None);
    }

    #[test]
    fn monitor_contains_point_uses_half_open_bounds() {
        let monitor = monitor("negative", (-1920, -200, 0, 880), true);

        assert!(monitor.contains_point(Vec2::new(-1920.0, -200.0)));
        assert!(monitor.contains_point(Vec2::new(-0.1, 879.9)));
        assert!(!monitor.contains_point(Vec2::new(0.0, 0.0)));
        assert!(!monitor.contains_point(Vec2::new(-1920.0, 880.0)));
    }
}
