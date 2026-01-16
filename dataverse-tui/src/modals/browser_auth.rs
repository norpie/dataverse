//! Browser authentication modal.
//!
//! Shared modal for OAuth2 browser flow authentication.
//! Takes environment URL, client ID, and tenant ID as inputs.
//! Returns `Some(AccessToken)` on success, `None` on cancel/error.

use dataverse_lib::auth::{AccessToken, BrowserFlow};
use dataverse_lib::error::AuthError;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Text};
use tokio_util::sync::CancellationToken;

use crate::widgets::Spinner;

/// Current state of the browser auth modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AuthState {
    #[default]
    Authenticating,
    Error,
}

/// Browser authentication modal.
///
/// Handles the OAuth2 browser flow for authentication.
/// Automatically opens the browser and waits for the user to complete auth.
///
/// # Example
///
/// ```ignore
/// let token = gx.modal(BrowserAuthModal::new(
///     "https://org.crm.dynamics.com",
///     "client-id",
///     "tenant-id",
/// )).await;
///
/// if let Some(access_token) = token {
///     // Auth succeeded
/// }
/// ```
#[modal]
pub struct BrowserAuthModal {
    #[state(skip)]
    env_url: String,
    #[state(skip)]
    client_id: String,
    #[state(skip)]
    tenant_id: String,

    state: AuthState,
    auth_url: String,
    error: Option<String>,
    cancel_token: Option<CancellationToken>,
}

impl BrowserAuthModal {
    /// Create a new browser auth modal.
    pub fn new(
        env_url: impl Into<String>,
        client_id: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            env_url: env_url.into(),
            client_id: client_id.into(),
            tenant_id: tenant_id.into(),
            ..Default::default()
        }
    }
}

#[modal_impl]
impl BrowserAuthModal {
    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    #[on_start]
    async fn start_auth(&self, mx: &ModalContext<Option<AccessToken>>) {
        let flow = BrowserFlow::new(&self.client_id, &self.tenant_id);
        let pending = match flow.start(&self.env_url).await {
            Ok(p) => p,
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.state.set(AuthState::Error);
                return;
            }
        };

        self.auth_url.set(pending.auth_url.clone());

        // Open browser automatically
        let _ = pending.open_browser();

        // Create cancel token for this auth attempt
        let token = CancellationToken::new();
        self.cancel_token.set(Some(token.clone()));

        match pending.wait_with_cancel(token).await {
            Ok(access_token) => {
                mx.close(Some(access_token));
            }
            Err(AuthError::BrowserCancelled) => {
                mx.close(None);
            }
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.state.set(AuthState::Error);
            }
        }
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<AccessToken>>) {
        if let Some(token) = self.cancel_token.get() {
            token.cancel();
        }
        mx.close(None);
    }

    #[handler]
    async fn retry(&self, mx: &ModalContext<Option<AccessToken>>) {
        self.error.set(None);
        self.state.set(AuthState::Authenticating);
        self.start_auth(mx).await;
    }

    #[handler]
    async fn open_browser(&self) {
        let url = self.auth_url.get();
        if !url.is_empty() {
            let _ = open::that(&url);
        }
    }

    fn element(&self) -> Element {
        match self.state.get() {
            AuthState::Authenticating => self.render_authenticating(),
            AuthState::Error => self.render_error(),
        }
    }

    fn render_authenticating(&self) -> Element {
        let auth_url = self.auth_url.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Authenticating") style (bold, fg: interact)
                text (content: "Complete authentication in your browser.") style (fg: muted)

                column (gap: 1) {
                    text (content: "Authorization URL:") style (fg: muted)
                    text (content: auth_url) style (fg: muted)
                }

                button (label: "Open Browser", id: "open_browser") on_activate: open_browser()

                row (gap: 1) {
                    spinner (id: "auth-spinner")
                    text (content: "Waiting for authentication...") style (fg: muted)
                }

                button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
            }
        }
    }

    fn render_error(&self) -> Element {
        let error_msg = self.error.get().unwrap_or_else(|| "Unknown error".to_string());

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Authentication Error") style (bold, fg: interact)
                text (content: "Authentication failed.") style (fg: muted)

                column (gap: 1) style (bg: background, padding: (1, 2)) {
                    text (content: error_msg) style (fg: error)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Retry", hint: "enter", id: "retry") on_activate: retry()
                }
            }
        }
    }
}
