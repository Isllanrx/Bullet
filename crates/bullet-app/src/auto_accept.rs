//! Accepts the ready check when the user turned "Accept matches automatically" on in the tray.

use std::time::Duration;

use bullet_core::phase::GamePhase;
use bullet_core::state::StateReceiver;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// The ready-check popup needs a moment before the client takes an accept; answering on the
/// very first phase event is refused as often as not.
const ACCEPT_DELAY: Duration = Duration::from_millis(1200);

const ATTEMPTS: u32 = 3;

pub async fn run(mut state_rx: StateReceiver, token: CancellationToken) {
    let mut in_ready_check = false;
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            changed = state_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                let now = state_rx.borrow_and_update().phase == GamePhase::ReadyCheck;
                let entered = now && !in_ready_check;
                in_ready_check = now;
                if entered && bullet_platform::preferences::AUTO_ACCEPT.is_enabled() {
                    accept(&state_rx, &token).await;
                }
            }
        }
    }
}

async fn accept(state_rx: &StateReceiver, token: &CancellationToken) {
    for attempt in 1..=ATTEMPTS {
        tokio::select! {
            _ = token.cancelled() => return,
            () = tokio::time::sleep(ACCEPT_DELAY) => {}
        }
        // The user may have accepted, declined or dodged by hand meanwhile.
        if state_rx.borrow().phase != GamePhase::ReadyCheck
            || !bullet_platform::preferences::AUTO_ACCEPT.is_enabled()
        {
            return;
        }
        let Some(client) = client().await else {
            warn!("Match found, but the client API is unavailable; not accepted automatically");
            return;
        };
        match client.accept_ready_check().await {
            Ok(true) => {
                info!(attempt, "Match accepted automatically");
                return;
            }
            Ok(false) => debug!(attempt, "The client did not take the automatic accept yet"),
            Err(e) => debug!(attempt, error = %e, "Automatic accept request failed"),
        }
    }
    warn!(
        attempts = ATTEMPTS,
        "Match found, but the automatic accept was not taken"
    );
}

async fn client() -> Option<bullet_lcu::client::LcuClient> {
    let lockfile = tokio::task::spawn_blocking(|| bullet_lcu::lockfile::Lockfile::discover(None))
        .await
        .ok()?
        .ok()?;
    bullet_lcu::client::LcuClient::new(&lockfile, bullet_lcu::client::DEFAULT_LCU_TIMEOUT).ok()
}
