use std::sync::OnceLock;
use std::sync::mpsc;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use slint::winit_030::{WinitWindowAccessor, winit};
use tracing::{debug, error};

use crate::error::PlatformError;

static UI_THREAD: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn run_on_ui(job: impl FnOnce() + Send + 'static) -> Result<(), PlatformError> {
    start()?;
    slint::invoke_from_event_loop(job)
        .map_err(|e| PlatformError::Window(format!("the interface thread is gone: {e}")))
}

fn start() -> Result<(), PlatformError> {
    UI_THREAD
        .get_or_init(|| {
            let (ready_tx, ready_rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("bullet-ui".into())
                .spawn(move || {
                    let selected = slint::BackendSelector::new()
                        .backend_name("winit".into())
                        .renderer_name("software".into())
                        .with_winit_window_attributes_hook(|attributes| {
                            attributes.with_active(false)
                        })
                        .select();
                    let failed = selected.is_err();
                    if ready_tx.send(selected.map_err(|e| e.to_string())).is_err() || failed {
                        return;
                    }
                    if let Err(e) = slint::run_event_loop_until_quit() {
                        error!(error = %e, "The interface event loop stopped");
                    }
                })
                .map_err(|e| format!("could not start the interface thread: {e}"))?;
            ready_rx
                .recv()
                .map_err(|_| "the interface thread ended during startup".to_owned())?
        })
        .clone()
        .map_err(PlatformError::Window)
}

pub(crate) fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

pub(crate) fn hwnd_now(window: &slint::Window) -> Option<isize> {
    window.with_winit_window(hwnd_of).flatten()
}

pub(crate) fn image(bytes: &[u8]) -> Option<slint::Image> {
    slint::Image::load_from_data(bytes, None).ok()
}

pub(crate) fn when_created<C: ComponentHandle + 'static>(
    component: &C,
    then: impl FnOnce(&C, isize) + 'static,
) {
    let weak = component.as_weak();
    let spawned = slint::spawn_local(async move {
        let Some(component) = weak.upgrade() else {
            return;
        };
        match component.window().winit_window().await {
            Ok(window) => {
                if let Some(hwnd) = hwnd_of(&window) {
                    then(&component, hwnd);
                }
            }
            Err(e) => debug!(error = %e, "A window closed before it was created"),
        }
    });
    if let Err(e) = spawned {
        debug!(error = %e, "Could not wait for a window to be created");
    }
}
