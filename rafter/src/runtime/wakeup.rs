//! Wakeup channel for passive rendering.
//!
//! This module provides a mechanism for waking up the event loop when
//! something changes (e.g., state update, async task completion).
//!
//! The event loop blocks indefinitely when idle. When `State::set()` or
//! similar is called, a wakeup signal is sent through this channel to
//! trigger a render check.

use std::cell::RefCell;
use tokio::sync::mpsc;

/// Sender half of the wakeup channel.
///
/// Clone-able, can be sent to async tasks.
#[derive(Clone)]
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
}

/// Create a new wakeup channel pair.
pub fn channel() -> (WakeupSender, WakeupReceiver) {
    // Small buffer - we just need to wake up, not queue many signals
    let (tx, rx) = mpsc::channel(16);
    (WakeupSender { tx }, WakeupReceiver { rx })
}

// Thread-local storage for the wakeup sender.
// This allows State::set() to send wakeups without having direct access
// to the channel.
thread_local! {
    static WAKEUP_SENDER: RefCell<Option<WakeupSender>> = const { RefCell::new(None) };
}

/// Install a wakeup sender for the current thread.
///
/// Called by the runtime when starting the event loop.
pub fn install_sender(sender: WakeupSender) {
    WAKEUP_SENDER.with(|s| {
        *s.borrow_mut() = Some(sender);
    });
}

/// Remove the wakeup sender for the current thread.
///
/// Called by the runtime when exiting the event loop.
pub fn uninstall_sender() {
    WAKEUP_SENDER.with(|s| {
        *s.borrow_mut() = None;
    });
}

/// Send a wakeup signal if a sender is installed.
///
/// Called by `State::set()`, `State::update()`, etc. to notify the event
/// loop that something changed and a render may be needed.
///
/// Safe to call even if no runtime is active (does nothing).
pub fn send_wakeup() {
    WAKEUP_SENDER.with(|s| {
        if let Some(sender) = s.borrow().as_ref() {
            sender.send();
        }
    });
}
