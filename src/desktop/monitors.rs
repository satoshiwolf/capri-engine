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
        let x = p.x as i32;
        let y = p.y as i32;
        x >= self.bounds.0 && x < self.bounds.2 && y >= self.bounds.1 && y < self.bounds.3
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
