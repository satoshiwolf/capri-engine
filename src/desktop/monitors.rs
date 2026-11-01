use glam::Vec2;
use windows::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

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
    let mut list = Vec::new();

    unsafe extern "system" fn callback(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list_ptr = lparam.0 as *mut Vec<MonitorInfo>;
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            let r = info.monitorInfo.rcMonitor;
            let w = info.monitorInfo.rcWork;
            let name_raw = &info.szDevice;
            let len = name_raw
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(name_raw.len());
            let name = String::from_utf16_lossy(&name_raw[..len]);

            let is_primary = (info.monitorInfo.dwFlags & 1) != 0;

            // SAFETY: `lparam` points to `list`, which remains alive for the synchronous
            // EnumDisplayMonitors call and is writable for the duration of this callback.
            unsafe {
                (*list_ptr).push(MonitorInfo {
                    name,
                    bounds: (r.left, r.top, r.right, r.bottom),
                    work_area: (w.left, w.top, w.right, w.bottom),
                    is_primary,
                });
            }
        }
        BOOL(1)
    }

    // SAFETY: `list` is a valid mutable local passed through LPARAM to the synchronous
    // callback; the callback is the only code that dereferences it.
    unsafe {
        let lparam = LPARAM(&mut list as *mut _ as isize);
        let _ = EnumDisplayMonitors(None, None, Some(callback), lparam);
    }

    list
}

pub fn virtual_desktop_bounds() -> (i32, i32, i32, i32) {
    // SAFETY: These GetSystemMetrics queries take no pointers and return virtual-screen
    // dimensions for the current desktop.
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (x, y, x + w, y + h)
    }
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
