//! Setup modal for initial account configuration.

use dataverse_lib::auth::{AccessToken, BrowserFlow};
use dataverse_lib::error::AuthError;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, Text};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::client_manager::ClientManager;
use crate::credentials::{Account, AuthType, CachedTokens, CredentialsProvider};
use crate::widgets::Spinner;

/// Wizard step for the setup modal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum SetupStep {
    #[default]
    Environment,
    Account,
    Authenticating,
    Error,
}

/// Setup modal for initial account configuration.
///
/// A 4-step wizard:
/// 1. Environment - URL and display name
/// 2. Account - Client ID, Tenant ID, account display name
/// 3. Authenticating - Waiting for browser auth
/// 4. Error - Only shown on failure
///
/// On success, toasts and closes with `Some(())`.
#[modal]
pub struct SetupModal {
    // Wizard state
    step: SetupStep,
    error: Option<String>,

    // Step 1: Environment
    env_url: String,
    env_display_name: String,

    // Step 2: Account
    client_id: String,
    tenant_id: String,
    account_display_name: String,

    // Step 3: Auth
    auth_url: String,
    cancel_token: Option<CancellationToken>,
}

#[modal_impl(kind = System)]
impl SetupModal {
    #[keybinds]
    fn keys() {
        bind("escape", cancel);
    }

    // =========================================================================
    // Validation
    // =========================================================================

    fn validate_step1(&self) -> Option<String> {
        let url = self.env_url.get();
        if url.trim().is_empty() {
            return Some("Environment URL is required".to_string());
        }
        if Url::parse(&url).is_err() {
            return Some("Invalid URL format".to_string());
        }
        if self.env_display_name.get().trim().is_empty() {
            return Some("Display name is required".to_string());
        }
        None
    }

    fn validate_step2(&self) -> Option<String> {
        if self.client_id.get().trim().is_empty() {
            return Some("Client ID is required".to_string());
        }
        if self.tenant_id.get().trim().is_empty() {
            return Some("Tenant ID is required".to_string());
        }
        if self.account_display_name.get().trim().is_empty() {
            return Some("Account display name is required".to_string());
        }
        None
    }

    // =========================================================================
    // Navigation Handlers
    // =========================================================================

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<()>>) {
        // Cancel any pending auth
        if let Some(token) = self.cancel_token.get() {
            token.cancel();
        }

        match self.step.get() {
            SetupStep::Environment => mx.close(None),
            SetupStep::Account => self.step.set(SetupStep::Environment),
            SetupStep::Authenticating => self.step.set(SetupStep::Account),
            SetupStep::Error => self.step.set(SetupStep::Account),
        }
    }

    #[handler]
    async fn next_step1(&self, gx: &GlobalContext) {
        if let Some(err) = self.validate_step1() {
            gx.toast(Toast::error(err));
            return;
        }
        self.step.set(SetupStep::Account);
    }

    #[handler]
    async fn next_step2(&self, gx: &GlobalContext, mx: &ModalContext<Option<()>>) {
        if let Some(err) = self.validate_step2() {
            gx.toast(Toast::error(err));
            return;
        }

        // Start authentication flow
        self.start_auth(gx, mx).await;
    }

    #[handler]
    async fn retry(&self, gx: &GlobalContext, mx: &ModalContext<Option<()>>) {
        self.error.set(None);
        self.start_auth(gx, mx).await;
    }

    // =========================================================================
    // Authentication
    // =========================================================================

    async fn start_auth(&self, gx: &GlobalContext, mx: &ModalContext<Option<()>>) {
        let flow = BrowserFlow::new(self.client_id.get(), self.tenant_id.get());
        let pending = match flow.start(&self.env_url.get()).await {
            Ok(p) => p,
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.step.set(SetupStep::Error);
                return;
            }
        };

        self.auth_url.set(pending.auth_url.clone());
        self.step.set(SetupStep::Authenticating);

        // Open browser automatically
        let _ = pending.open_browser();

        // Create fresh cancel token for this auth attempt
        let token = CancellationToken::new();
        self.cancel_token.set(Some(token.clone()));

        match pending.wait_with_cancel(token).await {
            Ok(access_token) => {
                self.on_auth_success(access_token, gx, mx).await;
            }
            Err(AuthError::BrowserCancelled) => {
                // User cancelled - go back to account step
                self.step.set(SetupStep::Account);
            }
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.step.set(SetupStep::Error);
            }
        }
    }

    #[handler]
    async fn open_browser(&self) {
        let url = self.auth_url.get();
        if !url.is_empty() {
            let _ = open::that(&url);
        }
    }

    async fn on_auth_success(
        &self,
        token: AccessToken,
        gx: &GlobalContext,
        mx: &ModalContext<Option<()>>,
    ) {
        let credentials = gx.data::<CredentialsProvider>();

        // Create environment
        let env = match credentials
            .create_environment(&self.env_url.get(), &self.env_display_name.get())
            .await
        {
            Ok(e) => e,
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.step.set(SetupStep::Error);
                return;
            }
        };

        // Create account
        let account = Account::new(
            self.account_display_name.get(),
            AuthType::Browser,
            self.client_id.get(),
            Some(self.tenant_id.get()),
            None,
            None,
            None,
        );
        let account = match credentials.create_account(&account).await {
            Ok(a) => a,
            Err(e) => {
                self.error.set(Some(e.to_string()));
                self.step.set(SetupStep::Error);
                return;
            }
        };

        // Save tokens
        let cached = CachedTokens {
            access_token: token.access_token,
            expires_at: token.expires_at,
            refresh_token: token.refresh_token,
        };
        if let Err(e) = credentials
            .save_tokens(account.id, env.id, &cached)
            .await
        {
            self.error.set(Some(e.to_string()));
            self.step.set(SetupStep::Error);
            return;
        }

        // Set as active session
        if let Err(e) = credentials
            .set_active_session(Some(account.id), Some(env.id))
            .await
        {
            self.error.set(Some(e.to_string()));
            self.step.set(SetupStep::Error);
            return;
        }

        // Verify connection
        let client_manager = gx.data::<ClientManager>();
        if let Err(e) = client_manager.get_client(account.id, env.id).await {
            self.error.set(Some(e.to_string()));
            self.step.set(SetupStep::Error);
            return;
        }

        // Success! Toast and close
        gx.toast(Toast::success("Connected successfully!"));
        mx.close(Some(()));
    }

    // =========================================================================
    // UI
    // =========================================================================

    fn element(&self) -> Element {
        match self.step.get() {
            SetupStep::Environment => self.render_environment_step(),
            SetupStep::Account => self.render_account_step(),
            SetupStep::Authenticating => self.render_authenticating_step(),
            SetupStep::Error => self.render_error_step(),
        }
    }

    fn render_environment_step(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Setup - Environment") style (bold, fg: accent)
                text (content: "Enter your Dataverse environment details.") style (fg: muted)

                column (gap: 1) {
                    text (content: "Environment URL") style (fg: muted)
                    input (state: self.env_url, id: "env_url", placeholder: "https://org.crm.dynamics.com")
                        style (bg: background)

                    text (content: "Display Name") style (fg: muted)
                    input (state: self.env_display_name, id: "env_display_name", placeholder: "My Environment")
                        style (bg: background)
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Next", hint: "enter", id: "next") on_activate: next_step1()
                }
            }
        }
    }

    fn render_account_step(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Setup - Account") style (bold, fg: accent)
                text (content: "Enter your Azure AD application details.") style (fg: muted)

                column (gap: 1) {
                    text (content: "Client ID") style (fg: muted)
                    input (state: self.client_id, id: "client_id", placeholder: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
                        style (bg: background)

                    text (content: "Tenant ID") style (fg: muted)
                    input (state: self.tenant_id, id: "tenant_id", placeholder: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
                        style (bg: background)

                    text (content: "Account Display Name") style (fg: muted)
                    input (state: self.account_display_name, id: "account_display_name", placeholder: "My Account")
                        style (bg: background)
                }

                row (width: fill, justify: between) {
                    button (label: "Back", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Next", hint: "enter", id: "next") on_activate: next_step2()
                }
            }
        }
    }

    fn render_authenticating_step(&self) -> Element {
        let auth_url = self.auth_url.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Setup - Authenticating") style (bold, fg: accent)
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

                button (label: "Back", hint: "esc", id: "cancel") on_activate: cancel()
            }
        }
    }

    fn render_error_step(&self) -> Element {
        let error_msg = self.error.get().unwrap_or_else(|| "Unknown error".to_string());

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Setup - Error") style (bold, fg: accent)
                text (content: "Authentication failed.") style (fg: muted)

                column (gap: 1) style (bg: background, padding: (1, 2)) {
                    text (content: error_msg) style (fg: error)
                }

                row (width: fill, justify: between) {
                    button (label: "Back", hint: "esc", id: "cancel") on_activate: cancel()
                    button (label: "Retry", hint: "enter", id: "retry") on_activate: retry()
                }
            }
        }
    }
}
