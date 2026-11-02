use arc_swap::ArcSwapOption;
use glam::Vec2;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, HTCLIENT,
    HTTRANSPARENT, MA_NOACTIVATE, MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS,
    MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostQuitMessage, QS_ALLINPUT,
    RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow,
    WM_CAPTURECHANGED, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{IUnknown, Interface, PCWSTR, w};

use crate::core::error::{EngineError, EngineResult};
use crate::core::state::{Facing, Stance};
use crate::core::visual::VisualTransform;
use crate::desktop::monitors::virtual_desktop_bounds;
use crate::render::hittest::AlphaBitmask;

pub static IS_RUNNING: AtomicBool = AtomicBool::new(true);

#[deprecated(
    since = "0.2.0",
    note = "Use `OverlayInput` and `CompositionOverlay`. This type will be removed in 0.4.0."
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
    hit_test: Arc<ArcSwapOption<HitTestSnapshot>>,
}

struct HitTestSnapshot {
    mask: Option<Arc<AlphaBitmask>>,
    transform: VisualTransform,
    pivot: Vec2,
}

impl OverlayInput {
    pub fn new() -> Self {
        Self {
            cursor_x: Arc::new(AtomicI32::new(0)),
            cursor_y: Arc::new(AtomicI32::new(0)),
            mouse_down: Arc::new(AtomicBool::new(false)),
            hit_test: Arc::new(ArcSwapOption::empty()),
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

    #[deprecated(
        since = "0.3.35",
        note = "Use `set_hit_test()` so mask and transform are published atomically. This method will be removed in 0.4.0."
    )]
    pub fn set_hit_test_mask(&self, mask: Option<Arc<AlphaBitmask>>) {
        let current = self.hit_test.load();
        let transform = current
            .as_ref()
            .map_or_else(default_hit_transform, |snapshot| snapshot.transform);
        let pivot = current
            .as_ref()
            .map_or(Vec2::ZERO, |snapshot| snapshot.pivot);
        self.set_hit_test(mask, transform, pivot);
    }

    #[deprecated(
        since = "0.3.35",
        note = "Use `set_hit_test()` so mask and transform are published atomically. This method will be removed in 0.4.0."
    )]
    pub fn set_hit_test_transform(&self, transform: VisualTransform, pivot: Vec2) {
        let mask = self
            .hit_test
            .load_full()
            .and_then(|snapshot| snapshot.mask.clone());
        self.set_hit_test(mask, transform, pivot);
    }

    pub fn set_hit_test(
        &self,
        mask: Option<Arc<AlphaBitmask>>,
        transform: VisualTransform,
        pivot: Vec2,
    ) {
        self.hit_test.store(Some(Arc::new(HitTestSnapshot {
            mask,
            transform,
            pivot,
        })));
    }

    pub fn hit_test(&self, screen_point: Vec2) -> bool {
        let Some(snapshot) = self.hit_test.load_full() else {
            return false;
        };
        snapshot.mask.as_ref().is_some_and(|mask| {
            mask.hit_test_transformed(screen_point, snapshot.transform, snapshot.pivot)
        })
    }

    fn set_cursor(&self, point: POINT) {
        self.cursor_x.store(point.x, Ordering::Relaxed);
        self.cursor_y.store(point.y, Ordering::Relaxed);
    }
}

fn default_hit_transform() -> VisualTransform {
    VisualTransform::new(Vec2::ZERO, 0.0, Vec2::ONE, Facing::Right)
}

impl Default for OverlayInput {
    fn default() -> Self {
        Self::new()
    }
}

pub fn should_capture_input(is_over_sprite: bool, mouse_down: bool, stance: Stance) -> bool {
    is_over_sprite || mouse_down || stance == Stance::Held
}

static GLOBAL_INPUT: LazyLock<ArcSwapOption<OverlayInput>> = LazyLock::new(ArcSwapOption::empty);

struct OwnedHwnd(HWND);

impl OwnedHwnd {
    fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    fn raw(&self) -> HWND {
        self.0
    }

    fn into_raw(mut self) -> HWND {
        let hwnd = self.0;
        self.0 = HWND(std::ptr::null_mut());
        hwnd
    }
}

impl Drop for OwnedHwnd {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            // SAFETY: This wrapper owns a window created on the current UI thread and
            // destroys it exactly once before the handle is forgotten.
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> EngineResult<Self> {
        // SAFETY: COM is initialized for the lifetime of the main-thread overlay and
        // balanced by `CoUninitialize` in `Drop`.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| EngineError::window("CoInitializeEx", error.to_string()))?;
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
            let x = (lparam.0 as u16 as i16) as f32;
            let y = ((lparam.0 >> 16) as u16 as i16) as f32;
            let point = Vec2::new(x, y);
            if GLOBAL_INPUT
                .load()
                .as_ref()
                .is_some_and(|input| input.hit_test(point))
            {
                LRESULT(HTCLIENT as isize)
            } else {
                LRESULT(HTTRANSPARENT as isize)
            }
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            if let Some(input) = GLOBAL_INPUT.load().as_ref()
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
            if let Some(input) = GLOBAL_INPUT.load().as_ref()
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
            if let Some(input) = GLOBAL_INPUT.load().as_ref() {
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

fn create_overlay_hwnd() -> EngineResult<OwnedHwnd> {
    // SAFETY: The instance handle and static UTF-16 strings remain valid while
    // `RegisterClassExW` and `CreateWindowExW` consume them. The window procedure
    // remains valid for the process lifetime.
    unsafe {
        let hinstance = GetModuleHandleW(None)
            .map_err(|error| EngineError::window("GetModuleHandleW", error.to_string()))?;

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

        let ex_style = WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOREDIRECTIONBITMAP
            | WS_EX_NOACTIVATE
            | WS_EX_TRANSPARENT
            | WS_EX_LAYERED;
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
        .map_err(|error| EngineError::window("CreateWindowExW", error.to_string()))?;

        let _ = SetWindowPos(
            hwnd,
            Some(HWND(-1isize as *mut _)), // HWND_TOPMOST
            vx,
            vy,
            vw,
            vh,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );

        Ok(OwnedHwnd::new(hwnd))
    }
}

fn create_input_hwnd() -> EngineResult<OwnedHwnd> {
    // SAFETY: The input class and window use process-lifetime resources. The input
    // window remains hidden until an opaque sprite pixel is under the cursor.
    unsafe {
        let hinstance = GetModuleHandleW(None)
            .map_err(|error| EngineError::window("GetModuleHandleW", error.to_string()))?;
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
            WS_EX_TOPMOST
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | WS_EX_NOREDIRECTIONBITMAP
                | WS_EX_TRANSPARENT
                | WS_EX_LAYERED,
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
        .map_err(|error| EngineError::window("CreateWindowExW input", error.to_string()))?;

        SetWindowPos(
            hwnd,
            Some(HWND(-1isize as *mut _)),
            0,
            0,
            16,
            16,
            SWP_NOACTIVATE,
        )
        .map_err(|error| EngineError::window("SetWindowPos input", error.to_string()))?;
        Ok(OwnedHwnd::new(hwnd))
    }
}

#[allow(deprecated)]
pub fn try_create_overlay_window(_state: WindowSharedState) -> EngineResult<HWND> {
    create_overlay_hwnd().map(OwnedHwnd::into_raw)
}

#[deprecated(
    since = "0.3.31",
    note = "Use `try_create_overlay_window()`. This function will be removed in 1.0.0."
)]
#[allow(deprecated)]
pub fn create_overlay_window(state: WindowSharedState) -> Result<HWND, String> {
    try_create_overlay_window(state).map_err(|error| error.to_string())
}

pub struct CompositionOverlay {
    visual: IDCompositionVisual,
    _target: IDCompositionTarget,
    _device: IDCompositionDesktopDevice,
    input_hwnd: OwnedHwnd,
    hwnd: OwnedHwnd,
    input: OverlayInput,
    _com: ComApartment,
    origin: (i32, i32),
    size: (u32, u32),
    input_visible: bool,
}

impl CompositionOverlay {
    pub fn try_create() -> EngineResult<Self> {
        let com = ComApartment::initialize()?;

        // SAFETY: DPI awareness is process-wide and must be selected before the overlay
        // window is created. Failure is non-fatal when awareness was already configured.
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let hwnd = create_overlay_hwnd()?;
        let (left, top, right, bottom) = virtual_desktop_bounds();
        let size = ((right - left) as u32, (bottom - top) as u32);
        let input = OverlayInput::new();
        let input_hwnd = create_input_hwnd()?;

        // SAFETY: The DirectComposition device is created without an external rendering
        // device, and the target and visual are owned for the lifetime of this overlay.
        let (device, target, visual) = unsafe {
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(None::<&IUnknown>)
                .map_err(|error| {
                EngineError::window("DCompositionCreateDevice2", error.to_string())
            })?;
            let target = device
                .CreateTargetForHwnd(hwnd.raw(), true)
                .map_err(|error| EngineError::window("CreateTargetForHwnd", error.to_string()))?;
            let visual = device
                .CreateVisual()
                .map_err(|error| EngineError::window("CreateVisual", error.to_string()))?;
            target.SetRoot(&visual).map_err(|error| {
                EngineError::window("IDCompositionTarget::SetRoot", error.to_string())
            })?;
            device.Commit().map_err(|error| {
                EngineError::window("IDCompositionDevice::Commit", error.to_string())
            })?;
            (device, target, visual)
        };
        GLOBAL_INPUT.store(Some(Arc::new(input.clone())));
        IS_RUNNING.store(true, Ordering::SeqCst);

        let mut overlay = Self {
            visual: visual.into(),
            _target: target,
            _device: device,
            input_hwnd,
            hwnd,
            input,
            _com: com,
            origin: (left, top),
            size,
            input_visible: false,
        };
        overlay.try_sync_input_capture(false)?;
        Ok(overlay)
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_create()`. This method will be removed in 1.0.0."
    )]
    pub fn create() -> Result<Self, String> {
        Self::try_create().map_err(|error| error.to_string())
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd.raw()
    }

    pub fn input(&self) -> &OverlayInput {
        &self.input
    }

    pub fn input_hwnd(&self) -> HWND {
        self.input_hwnd.raw()
    }

    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn try_commit(&self) -> EngineResult<()> {
        // SAFETY: The DirectComposition device is owned by this overlay and remains
        // valid for the lifetime of all visuals attached to it.
        unsafe {
            self._device.Commit().map_err(|error| {
                EngineError::window("IDCompositionDevice::Commit", error.to_string())
            })
        }
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_commit()`. This method will be removed in 1.0.0."
    )]
    pub fn commit(&self) -> Result<(), String> {
        self.try_commit().map_err(|error| error.to_string())
    }

    pub fn try_poll_cursor(&self) -> EngineResult<(i32, i32)> {
        let mut point = POINT::default();
        // SAFETY: `point` is a valid writable POINT for the duration of the call.
        unsafe {
            GetCursorPos(&mut point)
                .map_err(|error| EngineError::window("GetCursorPos", error.to_string()))?;
        }
        self.input.set_cursor(point);
        Ok((point.x, point.y))
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_poll_cursor()`. This method will be removed in 1.0.0."
    )]
    pub fn poll_cursor(&self) -> Result<(i32, i32), String> {
        self.try_poll_cursor().map_err(|error| error.to_string())
    }

    pub fn try_sync_input_capture(&mut self, capture: bool) -> EngineResult<()> {
        if capture {
            if self.input.mouse_down() {
                return Ok(());
            }
            let (cursor_x, cursor_y) = self.input.cursor();
            // SAFETY: The input window is a live topmost popup owned by this overlay.
            // Moving it under the cursor only changes its hit-test area.
            unsafe {
                SetWindowPos(
                    self.input_hwnd.raw(),
                    Some(HWND(-1isize as *mut _)),
                    cursor_x - 8,
                    cursor_y - 8,
                    16,
                    16,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
                .map_err(|error| EngineError::window("SetWindowPos input", error.to_string()))?;
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
            let _ = ShowWindow(self.input_hwnd.raw(), SW_HIDE);
        }
        self.input_visible = false;
        Ok(())
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_sync_input_capture()`. This method will be removed in 1.0.0."
    )]
    pub fn sync_input_capture(&mut self, capture: bool) -> Result<(), String> {
        self.try_sync_input_capture(capture)
            .map_err(|error| error.to_string())
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_sync_input_capture()` to coordinate the dedicated input window. This method will be removed in 1.0.0."
    )]
    #[allow(deprecated)]
    pub fn set_click_through(&mut self, click_through: bool) -> Result<(), String> {
        self.sync_input_capture(!click_through)
    }

    pub fn try_create_surface(
        &self,
        instance: &wgpu::Instance,
    ) -> EngineResult<wgpu::Surface<'static>> {
        // SAFETY: The DirectComposition visual is owned by this overlay and outlives
        // the returned surface; `wgpu` retains its own reference to the visual.
        unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(
                self.visual.as_raw(),
            ))
        }
        .map_err(|error| EngineError::gpu("create composition surface", error.to_string()))
    }

    #[deprecated(
        since = "0.3.31",
        note = "Use `try_create_surface()`. This method will be removed in 1.0.0."
    )]
    pub fn create_surface(
        &self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, String> {
        self.try_create_surface(instance)
            .map_err(|error| error.to_string())
    }
}

impl Drop for CompositionOverlay {
    fn drop(&mut self) {
        GLOBAL_INPUT.store(None);
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

    #[test]
    fn wm_hit_snapshot_reads_latest_mask_and_transform() {
        let input = OverlayInput::new();
        let mut alpha = vec![0; 20 * 20];
        alpha[10 * 20 + 10] = 255;
        let mask = AlphaBitmask::new(20, 20, alpha);
        let transform =
            VisualTransform::new(Vec2::new(100.0, 100.0), 0.0, Vec2::ONE, Facing::Right);

        input.set_hit_test(Some(Arc::new(mask)), transform, Vec2::new(10.0, 10.0));

        assert!(input.hit_test(Vec2::new(100.0, 100.0)));
        assert!(!input.hit_test(Vec2::new(100.0, 80.0)));
    }
}
