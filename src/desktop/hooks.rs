use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetSystemMetrics, GetWindowLongW,
    GetWindowRect, GetWindowTextW, IsIconic, IsWindow, IsWindowVisible, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

use crate::desktop::ledges::{Ledge, open_spans};
use crate::desktop::monitors::MonitorInfo;

#[derive(Debug, Clone)]
struct ScannedWindow {
    hwnd: HWND,
    title: String,
    bounds: RECT,
    is_seatable: bool,
}

const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "#32768",
    "tooltips_class32",
];

pub(crate) fn scan_ledges(skip_hwnds: &[isize]) -> Vec<Ledge> {
    let mut scanned: Vec<ScannedWindow> = Vec::new();

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: EnumWindows passes the pointer created from `scanned` below and invokes
        // this callback synchronously while that stack borrow remains valid.
        let list = unsafe { &mut *(lparam.0 as *mut Vec<ScannedWindow>) };

        if !IsWindow(Some(hwnd)).as_bool()
            || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
        {
            return true.into();
        }

        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return true.into();
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TOOLWINDOW.0) != 0 {
            return true.into();
        }

        let mut class_buf = [0u16; 128];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
        if SHELL_CLASSES.contains(&class_name.as_str()) {
            return true.into();
        }

        let mut bounds = RECT::default();
        let dwm_res = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut bounds as *mut _ as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if dwm_res.is_err() {
            let _ = GetWindowRect(hwnd, &mut bounds);
        }

        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        if width < 120 || height < 60 {
            return true.into();
        }

        let mut title_buf = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let has_caption = (style & 0x00C00000) == 0x00C00000;

        list.push(ScannedWindow {
            hwnd,
            title,
            bounds,
            is_seatable: has_caption,
        });

        true.into()
    }

    // SAFETY: `scanned` remains alive for the synchronous enumeration, and Windows only
    // passes its pointer to `enum_callback` for the duration of this call.
    unsafe {
        let lparam = LPARAM(&mut scanned as *mut _ as isize);
        let _ = EnumWindows(Some(enum_callback), lparam);
    }

    let mut found_ledges = Vec::new();
    let mut above_rects: Vec<RECT> = Vec::new();

    for win in scanned {
        let hwnd_val = win.hwnd.0 as isize;
        if skip_hwnds.contains(&hwnd_val) {
            continue;
        }

        if win.is_seatable {
            let spans = open_spans(
                win.bounds.left,
                win.bounds.right,
                win.bounds.top,
                &above_rects,
            );
            for (span_l, span_r) in spans {
                if (span_r - span_l) >= 40 {
                    found_ledges.push(Ledge {
                        hwnd: hwnd_val,
                        title: win.title.clone(),
                        left: span_l,
                        right: span_r,
                        y: win.bounds.top,
                        is_floor: false,
                    });
                }
            }
        }

        above_rects.push(win.bounds);
    }

    found_ledges
}

pub(crate) fn enumerate_monitors() -> Vec<MonitorInfo> {
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
        true.into()
    }

    // SAFETY: `list` is a valid mutable local passed through LPARAM to the synchronous
    // callback; the callback is the only code that dereferences it.
    unsafe {
        let lparam = LPARAM(&mut list as *mut _ as isize);
        let _ = EnumDisplayMonitors(None, None, Some(callback), lparam);
    }

    list
}

pub(crate) fn virtual_desktop_bounds() -> (i32, i32, i32, i32) {
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
