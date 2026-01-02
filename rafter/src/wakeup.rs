//! Wakeup channel for passive rendering.
//!
//! The event loop blocks when idle. When `State::set()` or similar is called,
//! a wakeup signal is sent to trigger a render check.

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Sender half of the wakeup channel.
#[derive(Clone, Debug)]
pub struct WakeupSender {
    tx: mpsc::Sender<()>,
}

impl WakeupSender {
    /// Send a wakeup signal.
    ///
    /// Non-blocking. Errors are ignored (receiver dropped = shutting down).
    pub fn send(&self) {
        let _ = self.tx.try_send(());
    }
}

/// Receiver half of the wakeup channel.
pub struct WakeupReceiver {
    rx: mpsc::Receiver<()>,
}

impl WakeupReceiver {
    /// Wait for a wakeup signal.
    pub async fn recv(&mut self) -> Option<()> {
        self.rx.recv().await
    }

    /// Drain all pending wakeup signals.
    ///
    /// Called after receiving a wakeup to consume redundant signals.
    /// Multiple buffered wakeups collapse into a single render.
    pub fn drain(&mut self) {
        while self.rx.try_recv().is_ok() {}
    }
}

/// Create a new wakeup channel pair.
pub fn channel() -> (WakeupSender, WakeupReceiver) {
    let (tx, rx) = mpsc::channel(16);
    (WakeupSender { tx }, WakeupReceiver { rx })
}

/// Handle for installing wakeup sender into State/Resource.
///
/// Wraps an optional sender that can be installed later by the runtime.
#[derive(Debug, Default, Clone)]
pub struct WakeupHandle {
    inner: Arc<Mutex<Option<WakeupSender>>>,
}

impl WakeupHandle {
    /// Create a new empty handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a wakeup sender.
    pub fn install(&self, sender: WakeupSender) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(sender);
        }
    }

    /// Send a wakeup signal if a sender is installed.
    pub fn send(&self) {
        if let Ok(guard) = self.inner.lock() {
            if let Some(sender) = guard.as_ref() {
                sender.send();
            }
        }
    }
}
