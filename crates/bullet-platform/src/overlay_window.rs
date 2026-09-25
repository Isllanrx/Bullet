use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use std::num::NonZeroIsize;

use bullet_core::overlay::OverlayCommand;
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{debug, info, warn};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, CreateSolidBrush, SetWindowRgn};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_TOPMOST, MSG,
    PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetForegroundWindow,
    SetWindowPos, ShowWindow, TranslateMessage, WM_APP, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::w;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebViewBuilder};

const OVERLAY_HTML: &str = include_str!("overlay_ui.html");

use crate::client_window::{ClientWindowState, WindowRect, client_window_state, overlay_placement};
use crate::error::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WindowControl {
    Focus,
    Blur,
    Drag,
    Hide,
}

impl WindowControl {
    fn try_parse(payload: &str) -> Option<Self> {
        serde_json::from_str(payload).ok()
    }
}

pub const OVERLAY_WIDTH: i32 = 360;

pub const OVERLAY_HEIGHT: i32 = 520;

pub const OVERLAY_PADDING: i32 = 16;

pub const OVERLAY_CORNER_RADIUS: i32 = 14;

const WM_OVERLAY_SHOW: u32 = WM_APP + 1;

const WM_OVERLAY_HIDE: u32 = WM_APP + 2;

const WM_OVERLAY_QUIT: u32 = WM_APP + 3;

const WM_OVERLAY_SCRIPT: u32 = WM_APP + 4;

fn pack_point(x: i32, y: i32) -> isize {
    let x = x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as isize;
    let y = y.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as isize;
    (x & 0xFFFF) | ((y & 0xFFFF) << 16)
}

fn unpack_point(packed: isize) -> (i32, i32) {
    let x = (packed & 0xFFFF) as u16 as i16 as i32;
    let y = ((packed >> 16) & 0xFFFF) as u16 as i16 as i32;
    (x, y)
}

#[derive(Clone)]
pub struct OverlayController {
    hwnd: isize,
    alive: Arc<AtomicBool>,

    pending_scripts: Arc<Mutex<Vec<String>>>,
}

impl OverlayController {
    #[must_use]
    pub fn window_handle(&self) -> isize {
        self.hwnd
    }

    pub fn show_at(&self, rect: WindowRect) {
        if !self.alive.load(Ordering::SeqCst) {
            return;
        }

        unsafe {
            if let Err(e) = PostMessageW(
                HWND(self.hwnd as *mut _),
                WM_OVERLAY_SHOW,
                WPARAM(pack_point(rect.left, rect.top) as usize),
                LPARAM(pack_point(rect.width(), rect.height())),
            ) {
                warn!(error = %e, "Could not post show request to the overlay window");
            }
        }
    }

    pub fn hide(&self) {
        if !self.alive.load(Ordering::SeqCst) {
            return;
        }

        unsafe {
            if let Err(e) = PostMessageW(
                HWND(self.hwnd as *mut _),
                WM_OVERLAY_HIDE,
                WPARAM(0),
                LPARAM(0),
            ) {
                warn!(error = %e, "Could not post hide request to the overlay window");
            }
        }
    }

    pub fn set_catalog(&self, catalog_json: String) {
        self.eval_script(format!("window.bulletOverlay.setCatalog({catalog_json});"));
    }

    pub fn eval_script(&self, script: String) {
        if !self.alive.load(Ordering::SeqCst) {
            return;
        }

        match self.pending_scripts.lock() {
            Ok(mut queue) => queue.push(script),
            Err(e) => {
                warn!(error = %e, "Overlay script queue is poisoned; dropping this update");
                return;
            }
        }

        unsafe {
            if let Err(e) = PostMessageW(
                HWND(self.hwnd as *mut _),
                WM_OVERLAY_SCRIPT,
                WPARAM(0),
                LPARAM(0),
            ) {
                warn!(error = %e, "Could not notify the overlay about queued work");
            }
        }
    }

    pub fn shutdown(&self) {
        if !self.alive.swap(false, Ordering::SeqCst) {
            return;
        }

        unsafe {
            // ignore-ok: the target window is our own and `alive` was checked; a failure means it is already closed
            let _ = PostMessageW(
                HWND(self.hwnd as *mut _),
                WM_OVERLAY_QUIT,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

pub struct OverlayWindow {
    controller: OverlayController,
}

impl OverlayWindow {
    pub fn spawn() -> Result<(Self, UnboundedReceiver<OverlayCommand>), PlatformError> {
        let (ready_tx, ready_rx) = channel::<Result<isize, String>>();
        let (command_tx, command_rx) = unbounded_channel::<OverlayCommand>();
        let alive = Arc::new(AtomicBool::new(true));
        let pending_scripts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        {
            let alive = alive.clone();
            let pending_scripts = pending_scripts.clone();
            thread::spawn(move || {
                run_overlay_message_loop(ready_tx, alive, pending_scripts, command_tx)
            });
        }

        let hwnd = ready_rx
            .recv()
            .map_err(|_| PlatformError::Window("overlay window thread died at startup".into()))?
            .map_err(PlatformError::Window)?;

        info!("Overlay window and WebView surface created");
        Ok((
            Self {
                controller: OverlayController {
                    hwnd,
                    alive,
                    pending_scripts,
                },
            },
            command_rx,
        ))
    }

    #[must_use]
    pub fn controller(&self) -> OverlayController {
        self.controller.clone()
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        self.controller.shutdown();
    }
}

#[must_use]
pub fn decide_placement(state: ClientWindowState, wanted: bool) -> Option<WindowRect> {
    if !wanted {
        return None;
    }
    match state {
        ClientWindowState::Visible(rect) => Some(overlay_placement(
            rect,
            OVERLAY_WIDTH,
            OVERLAY_HEIGHT,
            OVERLAY_PADDING,
        )),
        ClientWindowState::Hidden | ClientWindowState::Absent => None,
    }
}

#[derive(Debug, Default)]
pub struct OverlayTracker {
    last_client_rect: Option<WindowRect>,
    was_wanted: bool,
}

impl OverlayTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, controller: &OverlayController, wanted: bool) -> Option<WindowRect> {
        if !wanted {
            if self.was_wanted {
                controller.hide();
                self.was_wanted = false;
                self.last_client_rect = None;
            }
            return None;
        }

        match client_window_state() {
            ClientWindowState::Visible(client_rect) => {
                let rect =
                    overlay_placement(client_rect, OVERLAY_WIDTH, OVERLAY_HEIGHT, OVERLAY_PADDING);

                if self.last_client_rect != Some(client_rect) {
                    controller.show_at(rect);
                    self.last_client_rect = Some(client_rect);
                }
                self.was_wanted = true;
                Some(rect)
            }
            ClientWindowState::Hidden | ClientWindowState::Absent => {
                if self.was_wanted {
                    controller.hide();
                    self.was_wanted = false;
                    self.last_client_rect = None;
                }
                None
            }
        }
    }
}

static GLOBAL_TRACKER: Mutex<Option<OverlayTracker>> = Mutex::new(None);

pub fn track_once(controller: &OverlayController, wanted: bool) -> Option<WindowRect> {
    let mut lock = GLOBAL_TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    let tracker = lock.get_or_insert_with(OverlayTracker::new);
    tracker.tick(controller, wanted)
}

fn run_overlay_message_loop(
    ready_tx: Sender<Result<isize, String>>,
    alive: Arc<AtomicBool>,
    pending_scripts: Arc<Mutex<Vec<String>>>,
    command_tx: UnboundedSender<OverlayCommand>,
) {
    let class_name = w!("BulletOverlayWindowClass");

    unsafe {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: Default::default(),
            lpszClassName: class_name,
            hbrBackground: CreateSolidBrush(COLORREF(0x0014_0F0B)),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name,
            w!("Bullet"),
            WS_POPUP,
            0,
            0,
            OVERLAY_WIDTH,
            OVERLAY_HEIGHT,
            None,
            None,
            None,
            None,
        )
    };

    let hwnd = match hwnd {
        Ok(hwnd) if !hwnd.is_invalid() => hwnd,
        other => {
            warn!(result = ?other, "Failed to create the overlay window");
            let _ = ready_tx.send(Err(format!("overlay window creation failed: {other:?}"))); // ignore-ok: nobody is waiting any more; the thread tears itself down below
            alive.store(false, Ordering::SeqCst);
            return;
        }
    };

    let host = OverlayWindowHandle(hwnd);

    crate::paths::ensure_webview2_data_dir();

    let hwnd_raw = hwnd.0 as isize;
    let webview = match WebViewBuilder::new()
        .with_html(OVERLAY_HTML)
        .with_ipc_handler(move |request| {
            let payload = request.body();

            match WindowControl::try_parse(payload) {
                Some(WindowControl::Focus) => {
                    unsafe {
                        let _ = SetForegroundWindow(HWND(hwnd_raw as *mut _)); // ignore-ok: best-effort focus grant; typing simply fails silently if this is refused
                    }
                    return;
                }
                Some(WindowControl::Blur) => {
                    if let Some(client) = crate::client_window::find_client_hwnd() {
                        unsafe {
                            let _ = SetForegroundWindow(client); // ignore-ok: best-effort; the client keeps working even if this particular call is refused
                        }
                    }
                    return;
                }
                Some(WindowControl::Drag) => {
                    unsafe {
                        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
                        use windows::Win32::UI::WindowsAndMessaging::{
                            HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN,
                        };

                        // ignore-ok: initiating window drag via Win32 non-client message
                        let _ = ReleaseCapture();

                        // ignore-ok: non-client click message forwarded to start system window drag
                        let _ = SendMessageW(
                            HWND(hwnd_raw as *mut _),
                            WM_NCLBUTTONDOWN,
                            WPARAM(HTCAPTION as usize),
                            LPARAM(0),
                        );
                    }
                    return;
                }
                Some(WindowControl::Hide) => {
                    unsafe {
                        // ignore-ok: user requested close/hide via overlay titlebar button
                        let _ = ShowWindow(HWND(hwnd_raw as *mut _), SW_HIDE);
                    }
                    return;
                }
                None => {}
            }

            match OverlayCommand::parse(payload) {
                Ok(command) => {
                    info!(?command, "Overlay UI command received");
                    if command_tx.send(command).is_err() {
                        debug!("Nobody is listening for overlay commands any more");
                    }
                }

                Err(e) => {
                    warn!(error = %e, payload = %payload, "Unreadable message from the overlay UI");
                }
            }
        })
        .with_transparent(false)
        .with_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT).into(),
        })
        .build_as_child(&host)
    {
        Ok(webview) => webview,
        Err(e) => {
            warn!(error = %e, "Could not create the WebView2 overlay surface");
            let _ = ready_tx.send(Err(format!("WebView2 surface unavailable: {e}"))); // ignore-ok: nobody is waiting any more; the thread tears itself down below
            alive.store(false, Ordering::SeqCst);
            return;
        }
    };

    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            OVERLAY_WIDTH + 1,
            OVERLAY_HEIGHT + 1,
            OVERLAY_CORNER_RADIUS,
            OVERLAY_CORNER_RADIUS,
        );
        if SetWindowRgn(hwnd, region, true) == 0 {
            warn!("Could not apply rounded-corner region to the overlay window");
        }
    }

    if ready_tx.send(Ok(hwnd.0 as isize)).is_err() {
        alive.store(false, Ordering::SeqCst);
        return;
    }

    let mut msg = MSG::default();

    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_OVERLAY_QUIT => break,
                WM_OVERLAY_SHOW => {
                    let (x, y) = unpack_point(msg.wParam.0 as isize);
                    let (w, h) = unpack_point(msg.lParam.0);
                    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE); // ignore-ok: a refused reposition is retried by the next tracking tick, 200 ms later
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE); // ignore-ok: returns the previous visibility, not an error
                    if let Err(e) = webview.set_bounds(Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(w, h).into(),
                    }) {
                        debug!(error = %e, "Could not resize the overlay WebView");
                    }
                }
                WM_OVERLAY_HIDE => {
                    let _ = ShowWindow(hwnd, SW_HIDE); // ignore-ok: returns the previous visibility, not an error
                }
                WM_OVERLAY_SCRIPT => {
                    let queued: Vec<String> = pending_scripts
                        .lock()
                        .map(|mut queue| std::mem::take(&mut *queue))
                        .unwrap_or_default();
                    for script in queued {
                        if let Err(e) = webview.evaluate_script(&script) {
                            warn!(error = %e, "Could not run a script in the overlay UI");
                        }
                    }
                }
                _ => {
                    let _ = TranslateMessage(&msg); // ignore-ok: returns whether a key event was translated; this pump forwards either way
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    drop(webview);
    alive.store(false, Ordering::SeqCst);
    debug!("Overlay message loop finished");
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

struct OverlayWindowHandle(HWND);

impl HasWindowHandle for OverlayWindowHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?;
        let handle = Win32WindowHandle::new(raw);

        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}

#[cfg(test)]
#[path = "overlay_window_tests.rs"]
mod tests;
