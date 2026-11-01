use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWL_STYLE, GetClassNameW, GetWindowLongW, GetWindowRect,
    GetWindowTextW, IsIconic, IsWindow, IsWindowVisible, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

#[derive(Debug, Clone)]
pub struct Ledge {
    pub hwnd: isize,
    pub title: String,
    pub left: i32,
    pub right: i32,
    pub y: i32,
    pub is_floor: bool,
}

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

/// Returns the exposed ledge intervals after subtracting windows higher in the Z-order.
pub fn open_spans(left: i32, right: i32, y: i32, above_rects: &[RECT]) -> Vec<(i32, i32)> {
    let mut spans = vec![(left, right)];

    for cover in above_rects {
        if y < cover.top || y >= cover.bottom {
            continue;
        }

        let mut next_spans = Vec::new();
        for (span_left, span_right) in spans {
            if cover.right <= span_left || cover.left >= span_right {
                next_spans.push((span_left, span_right));
                continue;
            }

            if cover.left > span_left {
                next_spans.push((span_left, cover.left));
            }
            if cover.right < span_right {
                next_spans.push((cover.right, span_right));
            }
        }
        spans = next_spans;
    }

    spans
}

pub fn scan_ledges(skip_hwnds: &[isize]) -> Vec<Ledge> {
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

        // Exclude windows on inactive virtual desktops.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_spans_subtraction() {
        let cover = RECT {
            left: 50,
            top: 50,
            right: 150,
            bottom: 150,
        };

        let spans = open_spans(0, 200, 100, &[cover]);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0], (0, 50));
        assert_eq!(spans[1], (150, 200));
    }
}
