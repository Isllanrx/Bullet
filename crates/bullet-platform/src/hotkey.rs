use std::sync::mpsc::Sender;

use tracing::{info, warn};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey, VK_B,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

use crate::tray::TrayEvent;

const MARK_PROBLEM_ID: i32 = 0xB11E;

pub const MARK_PROBLEM_KEYS: &str = "Ctrl+Shift+B";

pub fn spawn_mark_problem_hotkey(events: Sender<TrayEvent>) {
    let spawned = std::thread::Builder::new()
        .name("bullet-hotkey".into())
        .spawn(move || {
            let registered = unsafe {
                RegisterHotKey(
                    None,
                    MARK_PROBLEM_ID,
                    MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                    u32::from(VK_B.0),
                )
            };
            if let Err(e) = registered {
                warn!(
                    keys = MARK_PROBLEM_KEYS,
                    error = %e,
                    "The mark-a-problem shortcut is taken by another program; use the control panel button"
                );
                return;
            }
            info!(keys = MARK_PROBLEM_KEYS, "Shortcut ready: marks a problem during the match");
            let mut msg = MSG::default();
            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
                if msg.message == WM_HOTKEY
                    && msg.wParam.0 == MARK_PROBLEM_ID as usize
                    && events.send(TrayEvent::MarkProblem).is_err()
                {
                    break;
                }
            }
            unsafe {
                let _ = UnregisterHotKey(None, MARK_PROBLEM_ID); // ignore-ok: the thread is ending; Windows drops the hotkey with it anyway
            }
        });
    if let Err(e) = spawned {
        warn!(error = %e, "The mark-a-problem shortcut thread could not start; use the control panel button");
    }
}
