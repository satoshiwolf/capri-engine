use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, HTCLIENT, HTTRANSPARENT,
    LWA_ALPHA, MA_NOACTIVATE, MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS, MsgWaitForMultipleObjectsEx,
    PM_REMOVE, PeekMessageW, PostQuitMessage, QS_ALLINPUT, RegisterClassExW, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow,
    WM_CAPTURECHANGED, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{IUnknown, Interface, PCWSTR, w};

use crate::core::state::Stance;
use crate::desktop::monitors::virtual_desktop_bounds;

pub static IS_RUNNING: AtomicBool = AtomicBool::new(true);

#[deprecated(
    since = "0.2.0",
    note = "Use `OverlayInput` and `CompositionOverlay`. This type will be removed in 0.3.0."
)]
#[derive(Clone)]
pub struct WindowSharedState {
    pub mouse_pos: Arc<Mutex<POINT>>,
    pub is_mouse_down: Arc<Mutex<bool>>,
    pub should_hit_client: Arc<Mutex<bool>>,
}

#[allow(deprecated)]
impl WindowSharedState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Arc::new(Mutex::new(POINT { x: 0, y: 0 })),
            is_mouse_down: Arc::new(Mutex::new(false)),
            should_hit_client: Arc::new(Mutex::new(false)),
        }
    }
}

#[allow(deprecated)]
impl Default for WindowSharedState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct OverlayInput {
    cursor_x: Arc<AtomicI32>,
    cursor_y: Arc<AtomicI32>,
    mouse_down: Arc<AtomicBool>,
    capture_input: Arc<AtomicBool>,
}

impl OverlayInput {
    pub fn new() -> Self {
        Self {
            cursor_x: Arc::new(AtomicI32::new(0)),
            cursor_y: Arc::new(AtomicI32::new(0)),
            mouse_down: Arc::new(AtomicBool::new(false)),
            capture_input: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cursor(&self) -> (i32, i32) {
        (
            self.cursor_x.load(Ordering::Relaxed),
            self.cursor_y.load(Ordering::Relaxed),
        )
    }

    pub fn mouse_down(&self) -> bool {
        self.mouse_down.load(Ordering::Acquire)
    }

    pub fn set_capture_input(&self, capture: bool) {
        self.capture_input.store(capture, Ordering::Release);
    }

    fn set_cursor(&self, point: POINT) {
        self.cursor_x.store(point.x, Ordering::Relaxed);
        self.cursor_y.store(point.y, Ordering::Relaxed);
    }
}

impl Default for OverlayInput {
    fn default() -> Self {
        Self::new()
    }
}

pub fn should_capture_input(is_over_sprite: bool, mouse_down: bool, stance: Stance) -> bool {
    is_over_sprite || mouse_down || stance == Stance::Held
}

static GLOBAL_INPUT: OnceLock<OverlayInput> = OnceLock::new();

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self, String> {
        // SAFETY: COM is initialized for the lifetime of the main-thread overlay and
        // balanced by `CoUninitialize` in `Drop`.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| format!("CoInitializeEx: {error}"))?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: This pairs with the successful `CoInitializeEx` in `initialize`.
        unsafe {
            CoUninitialize();
        }
    }
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
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

unsafe extern "system" fn input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            if GLOBAL_INPUT
                .get()
                .is_some_and(|input| input.capture_input.load(Ordering::Acquire))
            {
                LRESULT(HTCLIENT as isize)
            } else {
                LRESULT(HTTRANSPARENT as isize)
            }
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            if let Some(input) = GLOBAL_INPUT.get()
                && input.mouse_down.swap(true, Ordering::AcqRel)
            {
                return LRESULT(0);
            }
            // SAFETY: The input window is alive for the duration of the capture and
            // belongs to the thread processing this message.
            unsafe {
                SetCapture(hwnd);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(input) = GLOBAL_INPUT.get()
                && !input.mouse_down.swap(false, Ordering::AcqRel)
            {
                return LRESULT(0);
            }
            // SAFETY: Releasing capture is safe when this window does not own it.
            unsafe {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if let Some(input) = GLOBAL_INPUT.get() {
                input.mouse_down.store(false, Ordering::Release);
            }
            LRESULT(0)
        }
        _ => {
            // SAFETY: Windows supplies the original window handle and message parameters.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

fn create_overlay_hwnd() -> Result<HWND, String> {
    // SAFETY: The instance handle and static UTF-16 strings remain valid while
    // `RegisterClassExW` and `CreateWindowExW` consume them. The window procedure
    // remains valid for the process lifetime.
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {}", e))?;

        let class_name = w!("CapriEngineOverlayWindow");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: HINSTANCE(hinstance.0),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        let _ = RegisterClassExW(&wc);

        let (vx, vy, vr, vb) = virtual_desktop_bounds();
        let vw = vr - vx;
        let vh = vb - vy;

        let ex_style =
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP | WS_EX_NOACTIVATE;
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
            Some(HINSTANCE(hinstance.0)),
            None,
        )
        .map_err(|e| format!("CreateWindowExW: {}", e))?;

        let _ = SetWindowPos(
            hwnd,
            Some(HWND(-1isize as *mut _)), // HWND_TOPMOST
            vx,
            vy,
            vw,
            vh,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );

        Ok(hwnd)
    }
}

fn create_input_hwnd() -> Result<HWND, String> {
    // SAFETY: The input class and window use process-lifetime resources. The input
    // window remains hidden until an opaque sprite pixel is under the cursor.
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {}", e))?;
        let class_name = w!("CapriEngineInputWindow");
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(input_wndproc),
            hInstance: HINSTANCE(hinstance.0),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&window_class);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name,
            w!("Capri Input"),
            WS_POPUP,
            0,
            0,
            16,
            16,
            None,
            None,
            Some(HINSTANCE(hinstance.0)),
            None,
        )
        .map_err(|error| format!("CreateWindowExW input: {error}"))?;

        SetLayeredWindowAttributes(hwnd, COLORREF(0), 1, LWA_ALPHA)
            .map_err(|error| format!("SetLayeredWindowAttributes: {error}"))?;
        SetWindowPos(
            hwnd,
            Some(HWND(-1isize as *mut _)),
            0,
            0,
            16,
            16,
            SWP_NOACTIVATE,
        )
        .map_err(|error| format!("SetWindowPos input: {error}"))?;
        Ok(hwnd)
    }
}

#[deprecated(
    since = "0.2.0",
    note = "Use `CompositionOverlay::create()`. This function will be removed in 0.3.0."
)]
#[allow(deprecated)]
pub fn create_overlay_window(_state: WindowSharedState) -> Result<HWND, String> {
    create_overlay_hwnd()
}

pub struct CompositionOverlay {
    hwnd: HWND,
    input_hwnd: HWND,
    origin: (i32, i32),
    size: (u32, u32),
    input: OverlayInput,
    visual: IDCompositionVisual,
    _target: IDCompositionTarget,
    _device: IDCompositionDesktopDevice,
    _com: ComApartment,
    input_visible: bool,
}

impl CompositionOverlay {
    pub fn create() -> Result<Self, String> {
        let com = ComApartment::initialize()?;

        // SAFETY: DPI awareness is process-wide and must be selected before the overlay
        // window is created. Failure is non-fatal when awareness was already configured.
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let hwnd = create_overlay_hwnd()?;
        // SAFETY: The visual window remains visible and composed by DirectComposition,
        // while disabling it removes it from mouse and keyboard input routing.
        unsafe {
            let _ = EnableWindow(hwnd, false);
        }
        let (left, top, right, bottom) = virtual_desktop_bounds();
        let size = ((right - left) as u32, (bottom - top) as u32);
        let input = OverlayInput::new();
        let _ = GLOBAL_INPUT.set(input.clone());
        let input_hwnd = create_input_hwnd()?;

        // SAFETY: The DirectComposition device is created without an external rendering
        // device, and the target and visual are owned for the lifetime of this overlay.
        let (device, target, visual) = unsafe {
            let device: IDCompositionDesktopDevice =
                DCompositionCreateDevice2(None::<&IUnknown>)
                    .map_err(|error| format!("DCompositionCreateDevice2: {error}"))?;
            let target = device
                .CreateTargetForHwnd(hwnd, true)
                .map_err(|error| format!("CreateTargetForHwnd: {error}"))?;
            let visual = device
                .CreateVisual()
                .map_err(|error| format!("CreateVisual: {error}"))?;
            target
                .SetRoot(&visual)
                .map_err(|error| format!("IDCompositionTarget::SetRoot: {error}"))?;
            device
                .Commit()
                .map_err(|error| format!("IDCompositionDevice::Commit: {error}"))?;
            (device, target, visual)
        };

        let mut overlay = Self {
            hwnd,
            input_hwnd,
            origin: (left, top),
            size,
            input,
            visual: visual.into(),
            _target: target,
            _device: device,
            _com: com,
            input_visible: false,
        };
        overlay.sync_input_capture(false)?;
        Ok(overlay)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn input(&self) -> &OverlayInput {
        &self.input
    }

    pub fn input_hwnd(&self) -> HWND {
        self.input_hwnd
    }

    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn commit(&self) -> Result<(), String> {
        // SAFETY: The DirectComposition device is owned by this overlay and remains
        // valid for the lifetime of all visuals attached to it.
        unsafe {
            self._device
                .Commit()
                .map_err(|error| format!("IDCompositionDevice::Commit: {error}"))
        }
    }

    pub fn poll_cursor(&self) -> Result<(i32, i32), String> {
        let mut point = POINT::default();
        // SAFETY: `point` is a valid writable POINT for the duration of the call.
        unsafe {
            GetCursorPos(&mut point).map_err(|error| format!("GetCursorPos: {error}"))?;
        }
        self.input.set_cursor(point);
        Ok((point.x, point.y))
    }

    pub fn sync_input_capture(&mut self, capture: bool) -> Result<(), String> {
        if capture {
            if self.input.mouse_down() {
                return Ok(());
            }
            let (cursor_x, cursor_y) = self.input.cursor();
            // SAFETY: The input window is a live topmost popup owned by this overlay.
            // Moving it under the cursor only changes its hit-test area.
            unsafe {
                SetWindowPos(
                    self.input_hwnd,
                    Some(HWND(-1isize as *mut _)),
                    cursor_x - 8,
                    cursor_y - 8,
                    16,
                    16,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
                .map_err(|error| format!("SetWindowPos input: {error}"))?;
            }
            self.input_visible = true;
            return Ok(());
        }

        if !self.input_visible {
            return Ok(());
        }
        // SAFETY: Hiding the input window and releasing capture when it is hidden
        // prevents stale button state after the cursor leaves the sprite.
        unsafe {
            let _ = ReleaseCapture();
            self.input.mouse_down.store(false, Ordering::Release);
            let _ = ShowWindow(self.input_hwnd, SW_HIDE);
        }
        self.input_visible = false;
        Ok(())
    }

    #[deprecated(
        since = "0.2.1",
        note = "Use `sync_input_capture()` to coordinate the dedicated input window. This method will be removed in 0.3.0."
    )]
    pub fn set_click_through(&mut self, click_through: bool) -> Result<(), String> {
        self.sync_input_capture(!click_through)
    }

    pub fn create_surface(
        &self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, String> {
        // SAFETY: The DirectComposition visual is owned by this overlay and outlives
        // the returned surface; `wgpu` retains its own reference to the visual.
        unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
                self.visual.as_raw(),
            ))
        }
        .map_err(|error| format!("create composition surface: {error}"))
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

pub fn wait_for_messages(timeout_ms: u32) {
    // SAFETY: There are no object handles to wait on, and the timeout only delays
    // until window input arrives or the target frame time elapses.
    unsafe {
        let _ = MsgWaitForMultipleObjectsEx(
            None,
            timeout_ms,
            QS_ALLINPUT,
            MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS(0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_input_follows_sprite_and_drag_state() {
        assert!(should_capture_input(true, false, Stance::Stand));
        assert!(should_capture_input(false, true, Stance::Stand));
        assert!(should_capture_input(false, false, Stance::Held));
        assert!(!should_capture_input(false, false, Stance::Fly));
    }
}
