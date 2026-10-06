use std::cell::Cell;
use std::num::NonZeroIsize;
use std::sync::OnceLock;

use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use tracing::debug;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_WINDOW, CreateSolidBrush, EndPaint, FillRect, HBRUSH, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetSystemMetrics, HICON, ICON_BIG, ICON_SMALL, IDC_ARROW, LoadCursorW, LoadIconW, MINMAXINFO,
    MSG, PostMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SendMessageW, SetForegroundWindow, SetTimer, ShowWindow, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_CLOSE, WM_DESTROY, WM_GETMINMAXINFO, WM_PAINT, WM_SETICON, WM_SIZE,
    WM_TIMER, WNDCLASSW, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    WS_THICKFRAME, WS_VISIBLE,
};
use windows::core::{HSTRING, PCWSTR};

use crate::error::PlatformError;

const WM_HOST_RESIZED: u32 = WM_APP + 100;
pub(crate) const WM_HOST_TICK: u32 = WM_APP + 101;
const BACKDROP: COLORREF = COLORREF(0x000C0805);

thread_local! {
    static MIN_SIZE: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

static BACKDROP_BRUSH: OnceLock<isize> = OnceLock::new();

pub(crate) struct HostHandle(pub(crate) HWND);

impl HasWindowHandle for HostHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?;
        let handle = Win32WindowHandle::new(raw);
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}

#[allow(clippy::manual_dangling_ptr)]
pub(crate) fn load_bullet_icon() -> Option<HICON> {
    let module = unsafe { GetModuleHandleW(None) }.ok()?;
    let icon = unsafe { LoadIconW(HINSTANCE(module.0), PCWSTR(1 as *const u16)) }.ok()?;
    (!icon.is_invalid()).then_some(icon)
}

pub(crate) fn post(hwnd: isize, message: u32) {
    if let Err(e) = unsafe { PostMessageW(HWND(hwnd as *mut _), message, WPARAM(0), LPARAM(0)) } {
        debug!(error = %e, message, "A dialog message was not posted; its window is gone");
    }
}

pub(crate) fn close(hwnd: isize) {
    post(hwnd, WM_CLOSE);
}

pub(crate) struct DialogSpec<'a> {
    pub class: PCWSTR,
    pub title: &'a str,
    pub size: (i32, i32),
    pub min_size: Option<(i32, i32)>,
}

pub(crate) struct Dialog {
    hwnd: HWND,
    web_context: wry::WebContext,
}

impl Dialog {
    pub(crate) fn create(spec: &DialogSpec<'_>) -> Result<Self, PlatformError> {
        let icon = load_bullet_icon().unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(dialog_wnd_proc),
            lpszClassName: spec.class,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            hIcon: icon,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        unsafe {
            let _ = RegisterClassW(&class); // ignore-ok: a second registration fails only because the class exists
        }

        MIN_SIZE.set(spec.min_size.unwrap_or_default());
        let resizable = if spec.min_size.is_some() {
            WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX
        } else {
            WINDOW_STYLE(0)
        };
        let (width, height) = spec.size;
        let x = (unsafe { GetSystemMetrics(SM_CXSCREEN) } - width) / 2;
        let y = (unsafe { GetSystemMetrics(SM_CYSCREEN) } - height) / 2;
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                spec.class,
                &HSTRING::from(spec.title),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE | resizable,
                x,
                y,
                width,
                height,
                None,
                None,
                None,
                None,
            )
        }
        .map_err(|e| PlatformError::Window(format!("could not create the window: {e}")))?;

        if !icon.is_invalid() {
            for size in [ICON_BIG, ICON_SMALL] {
                unsafe {
                    // ignore-ok: WM_SETICON returns the previous icon, not a status
                    let _ = SendMessageW(
                        hwnd,
                        WM_SETICON,
                        WPARAM(size as usize),
                        LPARAM(icon.0 as isize),
                    );
                }
            }
        }
        let dark: i32 = 1;
        unsafe {
            // ignore-ok: cosmetic; older Windows builds refuse this attribute by design
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );
        }
        Ok(Self {
            hwnd,
            web_context: wry::WebContext::new(crate::paths::webview2_data_dir()),
        })
    }

    pub(crate) fn raw(&self) -> isize {
        self.hwnd.0 as isize
    }

    pub(crate) fn webview(
        &mut self,
        html: String,
        ipc: impl Fn(wry::http::Request<String>) + 'static,
    ) -> Result<wry::WebView, PlatformError> {
        let built = wry::WebViewBuilder::new_with_web_context(&mut self.web_context)
            .with_html(html)
            .with_bounds(client_bounds(self.hwnd))
            .with_ipc_handler(ipc)
            .build_as_child(&HostHandle(self.hwnd));
        built.map_err(|e| {
            self.destroy();
            PlatformError::Window(format!("could not create the WebView: {e}"))
        })
    }

    pub(crate) fn show(&self, foreground: bool) {
        unsafe {
            // ignore-ok: ShowWindow returns the previous visibility, not a status
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            if foreground {
                // ignore-ok: Windows may refuse the foreground; the window is still shown
                let _ = SetForegroundWindow(self.hwnd);
            }
        }
    }

    pub(crate) fn tick_every(&self, id: usize, ms: u32) {
        if unsafe { SetTimer(self.hwnd, id, ms, None) } == 0 {
            debug!(id, "A dialog timer could not be started");
        }
    }

    pub(crate) fn destroy(&self) {
        unsafe {
            // ignore-ok: the only failure is an already-gone window, which is the wanted outcome
            let _ = DestroyWindow(self.hwnd);
        }
    }

    pub(crate) fn run(&self, webview: &wry::WebView, mut on_message: impl FnMut(u32, usize)) {
        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
            match msg.message {
                WM_HOST_RESIZED => {
                    if let Err(e) = webview.set_bounds(client_bounds(self.hwnd)) {
                        debug!(error = %e, "A dialog WebView could not be resized");
                    }
                }
                message if message >= WM_APP => on_message(message, msg.wParam.0),
                _ => unsafe {
                    // ignore-ok: reports whether a key message was translated; nothing depends on it
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                },
            }
        }
    }
}

fn client_bounds(hwnd: HWND) -> wry::Rect {
    let mut rect = RECT::default();
    unsafe {
        // ignore-ok: a zeroed rect sizes the WebView at 0x0 until the next WM_SIZE
        let _ = GetClientRect(hwnd, &mut rect);
    }
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(0, 0).into(),
        size: wry::dpi::LogicalSize::new(
            (rect.right - rect.left).max(0) as u32,
            (rect.bottom - rect.top).max(0) as u32,
        )
        .into(),
    }
}

fn backdrop_brush() -> HBRUSH {
    let raw = *BACKDROP_BRUSH.get_or_init(|| unsafe { CreateSolidBrush(BACKDROP) }.0 as isize);
    HBRUSH(raw as *mut _)
}

unsafe extern "system" fn dialog_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            unsafe {
                // ignore-ok: our own window; a lost resize is redone by the next WM_SIZE
                let _ = PostMessageW(hwnd, WM_HOST_RESIZED, WPARAM(0), LPARAM(0));
            }
            LRESULT(0)
        }
        WM_TIMER => {
            unsafe {
                // ignore-ok: our own window; a lost tick is redone by the next one
                let _ = PostMessageW(hwnd, WM_HOST_TICK, wparam, LPARAM(0));
            }
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let (width, height) = MIN_SIZE.get();
            let info = lparam.0 as *mut MINMAXINFO;
            if width > 0 && !info.is_null() {
                unsafe {
                    (*info).ptMinTrackSize.x = width;
                    (*info).ptMinTrackSize.y = height;
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                // ignore-ok: a zeroed rect paints nothing; this fill is only the WebView backdrop
                let _ = GetClientRect(hwnd, &mut rect);
                FillRect(hdc, &rect, backdrop_brush());
                // ignore-ok: EndPaint always succeeds for a PAINTSTRUCT from BeginPaint
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                // ignore-ok: the only failure is an already-gone window, which is the wanted outcome
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
