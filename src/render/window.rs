use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, HTCLIENT, HTTRANSPARENT,
    PM_REMOVE, PeekMessageW, PostQuitMessage, RegisterClassExW, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SetWindowPos, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST,
    WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::desktop::monitors::virtual_desktop_bounds;

pub static IS_RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Clone)]
pub struct WindowSharedState {
    pub mouse_pos: Arc<Mutex<POINT>>,
    pub is_mouse_down: Arc<Mutex<bool>>,
    pub should_hit_client: Arc<Mutex<bool>>,
}

impl WindowSharedState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Arc::new(Mutex::new(POINT { x: 0, y: 0 })),
            is_mouse_down: Arc::new(Mutex::new(false)),
            should_hit_client: Arc::new(Mutex::new(false)),
        }
    }
}

impl Default for WindowSharedState {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_STATE: OnceLock<WindowSharedState> = OnceLock::new();

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            // Solid sprite pixels hit the overlay; transparent pixels pass clicks through.
            if let Some(state) = GLOBAL_STATE.get() {
                let hit = *state.should_hit_client.lock().unwrap();
                if hit {
                    return LRESULT(HTCLIENT as isize);
                }
            }
            LRESULT(HTTRANSPARENT as isize)
        }
        WM_LBUTTONDOWN => {
            if let Some(state) = GLOBAL_STATE.get() {
                *state.is_mouse_down.lock().unwrap() = true;
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(state) = GLOBAL_STATE.get() {
                *state.is_mouse_down.lock().unwrap() = false;
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(state) = GLOBAL_STATE.get() {
                let mut pt = POINT::default();
                // SAFETY: `pt` is a valid writable POINT for the duration of the call.
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                }
                *state.mouse_pos.lock().unwrap() = pt;
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            IS_RUNNING.store(false, Ordering::SeqCst);
            // SAFETY: PostQuitMessage has no pointer preconditions.
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => {
            // SAFETY: Windows supplies the original window handle and message parameters.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

pub fn create_overlay_window(state: WindowSharedState) -> Result<HWND, String> {
    let _ = GLOBAL_STATE.set(state);
    // SAFETY: The instance handle and static UTF-16 strings remain valid while
    // RegisterClassExW and CreateWindowExW consume them. The window proc is
    // valid for the process lifetime.
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {}", e))?;

        let class_name = w!("CapriEngineOverlayWindow");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        let _ = RegisterClassExW(&wc);

        let (vx, vy, vr, vb) = virtual_desktop_bounds();
        let vw = vr - vx;
        let vh = vb - vy;

        let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP;
        let style = WS_POPUP;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("Capri Engine"),
            style,
            vx,
            vy,
            vw,
            vh,
            None,
            None,
            hinstance,
            None,
        )
        .map_err(|e| format!("CreateWindowExW: {}", e))?;

        let _ = SetWindowPos(
            hwnd,
            HWND(-1 as _), // HWND_TOPMOST
            vx,
            vy,
            vw,
            vh,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );

        Ok(hwnd)
    }
}

pub fn pump_messages() -> bool {
    // SAFETY: `msg` is a valid local MSG buffer owned by this call, and
    // DispatchMessageW receives only messages copied from the current queue.
    unsafe {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                return false;
            }
            let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
    true
}
