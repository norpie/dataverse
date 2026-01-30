//! Client management modal with tabbed interface.
//!
//! Uses page routing for tab navigation with a shared layout wrapper.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{
    Button, Input, List, ListItem, ListState, Select, SelectState, SelectionMode, Text,
};
use tuidom::{Color, Element, Style};

use crate::credentials::CredentialsProvider;
use crate::modals::{BrowserAuthModal, ConfirmModal};

use super::{EnvironmentAdded, EnvironmentRemoved, SessionChanged};

/// Page enum for the client management modal tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Active,
    Environments,
    Accounts,
}

/// Connection status for an environment-account pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// No tokens stored.
    #[default]
    NotConnected,
    /// Tokens exist and are valid.
    Connected,
    /// Tokens exist but are expired (needs re-auth).
    Expired,
}

/// Buffer time before expiry to consider tokens as "expiring soon" (5 minutes).
const TOKEN_EXPIRY_BUFFER_SECS: i64 = 300;

/// Environment list item.
#[derive(Clone, Debug)]
struct EnvListItem {
    id: i64,
    display_name: String,
    url: String,
}

impl ListItem for EnvListItem {
    type Key = i64;

    fn key(&self) -> i64 {
        self.id
    }

    fn render(&self) -> Element {
        Element::row()
            .child(Element::text(&self.display_name))
            .child(
                Element::text(format!(" ({})", self.url))
                    .style(Style::new().foreground(Color::var("muted"))),
            )
    }
}

/// Account list item.
#[derive(Clone, Debug)]
struct AccListItem {
    id: i64,
    display_name: String,
    client_id: String,
}

impl ListItem for AccListItem {
    type Key = i64;

    fn key(&self) -> i64 {
        self.id
    }

    fn render(&self) -> Element {
        Element::row()
            .child(Element::text(&self.display_name))
            .child(
                Element::text(format!(" ({})", self.client_id))
                    .style(Style::new().foreground(Color::var("muted"))),
            )
    }
}

/// Modal for managing environments, accounts, and their connections.
#[modal(default, size = Lg, pages)]
pub struct ClientManagementModal {
    // Active tab state
    env_select: SelectState<i64>,
    acc_select: SelectState<i64>,
    connection_status: ConnectionStatus,

    // Environments tab state
    env_list: ListState<EnvListItem>,
    env_adding: bool,
    env_add_url: String,
    env_add_name: String,

    // Accounts tab state
    acc_list: ListState<AccListItem>,
    acc_adding: bool,
    acc_add_client_id: String,
    acc_add_tenant_id: String,
    acc_add_name: String,
}

#[modal_impl(kind = System, layout = layout)]
impl ClientManagementModal {
    fn default_result(&self) {}

    #[on_start]
    async fn load_data(&self, gx: &GlobalContext) {
        self.reload_data(gx).await;
    }

    async fn reload_data(&self, gx: &GlobalContext) {
        let credentials = gx.data::<CredentialsProvider>();

        // Load active session first to know what to pre-select
        let active_session = credentials.get_active_session().await.ok();
        let active_env_id = active_session.as_ref().and_then(|s| s.environment_id);
        let active_acc_id = active_session.as_ref().and_then(|s| s.account_id);

        // Load environments
        if let Ok(envs) = credentials.list_environments().await {
            // For Active tab select
            let options: Vec<(i64, String)> = envs
                .iter()
                .map(|e| (e.id, e.display_name.clone()))
                .collect();

            let mut select_state = SelectState::new(options);
            if let Some(env_id) = active_env_id {
                select_state = select_state.with_value(env_id);
            }
            self.env_select.set(select_state);

            // For Environments tab list
            let list_items: Vec<EnvListItem> = envs
                .into_iter()
                .map(|e| EnvListItem {
                    id: e.id,
                    display_name: e.display_name,
                    url: e.url,
                })
                .collect();
            self.env_list
                .set(ListState::new(list_items).with_selection(SelectionMode::Single));
        }

        // Load accounts
        if let Ok(accs) = credentials.list_accounts().await {
            // For Active tab select
            let options: Vec<(i64, String)> = accs
                .iter()
                .map(|a| (a.id, a.display_name.clone()))
                .collect();

            let mut select_state = SelectState::new(options);
            if let Some(acc_id) = active_acc_id {
                select_state = select_state.with_value(acc_id);
            }
            self.acc_select.set(select_state);

            // For Accounts tab list
            let list_items: Vec<AccListItem> = accs
                .into_iter()
                .map(|a| AccListItem {
                    id: a.id,
                    display_name: a.display_name,
                    client_id: a.client_id,
                })
                .collect();
            self.acc_list
                .set(ListState::new(list_items).with_selection(SelectionMode::Single));
        }

        // Check connection status for current selection
        if let (Some(acc_id), Some(env_id)) = (active_acc_id, active_env_id) {
            let status = match credentials.get_tokens(acc_id, env_id).await {
                Ok(Some(tokens)) => {
                    if tokens.is_expired_within(TOKEN_EXPIRY_BUFFER_SECS) {
                        ConnectionStatus::Expired
                    } else {
                        ConnectionStatus::Connected
                    }
                }
                _ => ConnectionStatus::NotConnected,
            };
            self.connection_status.set(status);
        }
    }

    #[keybinds]
    fn keys() {
        bind("escape", close);
        bind("1", tab_active);
        bind("2", tab_environments);
        bind("3", tab_accounts);
    }

    #[handler]
    async fn close(&self, mx: &ModalContext<()>) {
        mx.close(());
    }

    #[handler]
    async fn tab_active(&self) {
        self.navigate(Page::Active);
    }

    #[handler]
    async fn tab_environments(&self) {
        self.navigate(Page::Environments);
    }

    #[handler]
    async fn tab_accounts(&self) {
        self.navigate(Page::Accounts);
    }

    #[handler]
    async fn on_env_select(&self, gx: &GlobalContext) {
        let env_state = self.env_select.get();
        log::debug!(
            "[ClientManagement] Environment selected: {:?}",
            env_state.value()
        );
        self.update_connection_status(gx).await;
    }

    #[handler]
    async fn on_acc_select(&self, gx: &GlobalContext) {
        let acc_state = self.acc_select.get();
        log::debug!(
            "[ClientManagement] Account selected: {:?}",
            acc_state.value()
        );
        self.update_connection_status(gx).await;
    }

    async fn update_connection_status(&self, gx: &GlobalContext) {
        let env_state = self.env_select.get();
        let acc_state = self.acc_select.get();

        if let (Some(&env_id), Some(&acc_id)) = (env_state.value(), acc_state.value()) {
            let credentials = gx.data::<CredentialsProvider>();
            let status = match credentials.get_tokens(acc_id, env_id).await {
                Ok(Some(tokens)) => {
                    if tokens.is_expired_within(TOKEN_EXPIRY_BUFFER_SECS) {
                        ConnectionStatus::Expired
                    } else {
                        ConnectionStatus::Connected
                    }
                }
                _ => ConnectionStatus::NotConnected,
            };
            log::debug!(
                "[ClientManagement] Connection status: env_id={}, acc_id={}, status={:?}",
                env_id,
                acc_id,
                status
            );
            self.connection_status.set(status);
        } else {
            self.connection_status.set(ConnectionStatus::NotConnected);
        }
    }

    #[handler]
    async fn connect(&self, gx: &GlobalContext) {
        let env_state = self.env_select.get();
        let acc_state = self.acc_select.get();

        log::debug!(
            "[ClientManagement] Connect: env_select={:?}, acc_select={:?}",
            env_state.value(),
            acc_state.value()
        );

        let (env_id, acc_id) = match (env_state.value(), acc_state.value()) {
            (Some(&e), Some(&a)) => (e, a),
            _ => {
                gx.toast(Toast::error("Select both environment and account"));
                return;
            }
        };

        log::info!(
            "[ClientManagement] Connecting env_id={}, acc_id={}",
            env_id,
            acc_id
        );

        // Get env and account details from credentials
        let credentials = gx.data::<CredentialsProvider>();

        let env = credentials
            .list_environments()
            .await
            .ok()
            .and_then(|envs| envs.into_iter().find(|e| e.id == env_id));
        let acc = credentials
            .list_accounts()
            .await
            .ok()
            .and_then(|accs| accs.into_iter().find(|a| a.id == acc_id));

        let (env_url, env_name, client_id, acc_name, tenant_id) = match (env, acc) {
            (Some(e), Some(a)) => {
                let tenant = a.tenant_id.unwrap_or_default();
                (e.url.clone(), e.display_name.clone(), a.client_id.clone(), a.display_name.clone(), tenant)
            }
            _ => {
                gx.toast(Toast::error("Could not find environment or account"));
                return;
            }
        };

        // Open browser auth modal
        log::debug!(
            "[ClientManagement] Opening BrowserAuthModal for env_url={}, client_id={}, tenant_id={}",
            env_url,
            client_id,
            tenant_id
        );
        let token = gx
            .modal(BrowserAuthModal::for_environment(&env_url, &client_id, &tenant_id))
            .await;

        log::debug!(
            "[ClientManagement] BrowserAuthModal returned: has_token={}",
            token.is_some()
        );

        if let Some(access_token) = token {
            log::info!(
                "[ClientManagement] Auth successful, saving tokens for acc_id={}, env_id={}",
                acc_id,
                env_id
            );

            // Save tokens
            let cached = crate::credentials::CachedTokens {
                access_token: access_token.access_token,
                expires_at: access_token.expires_at,
                refresh_token: access_token.refresh_token.clone(),
            };

            log::debug!(
                "[ClientManagement] Saving tokens: expires_at={:?}, has_refresh={}",
                cached.expires_at,
                cached.refresh_token.is_some()
            );

            if let Err(e) = credentials.save_tokens(acc_id, env_id, &cached).await {
                log::error!("[ClientManagement] Failed to save tokens: {}", e);
                gx.toast(Toast::error(format!("Failed to save tokens: {}", e)));
                return;
            }

            log::debug!("[ClientManagement] Tokens saved successfully");

            // Set as active session
            if let Err(e) = credentials
                .set_active_session(Some(acc_id), Some(env_id))
                .await
            {
                log::error!("[ClientManagement] Failed to set session: {}", e);
                gx.toast(Toast::error(format!("Failed to set session: {}", e)));
                return;
            }

            log::info!(
                "[ClientManagement] Session set to acc_id={}, env_id={}",
                acc_id,
                env_id
            );

            self.connection_status.set(ConnectionStatus::Connected);
            
            // Publish session changed event
            gx.publish(SessionChanged {
                account_id: Some(acc_id),
                env_id: Some(env_id),
                account_name: Some(acc_name),
                environment_name: Some(env_name.clone()),
                environment_url: Some(env_url),
            });

            log::info!(
                "[ClientManagement] Published SessionChanged for {}",
                env_name
            );
            
            gx.toast(Toast::success("Connected successfully!"));
        } else {
            log::debug!("[ClientManagement] Auth cancelled or failed");
        }
    }

    // =========================================================================
    // Environments tab handlers
    // =========================================================================

    #[handler]
    async fn env_show_add(&self) {
        self.env_adding.set(true);
        self.env_add_url.set(String::new());
        self.env_add_name.set(String::new());
    }

    #[handler]
    async fn env_cancel_add(&self) {
        self.env_adding.set(false);
    }

    #[handler]
    async fn env_confirm_add(&self, gx: &GlobalContext) {
        let url = self.env_add_url.get();
        let name = self.env_add_name.get();

        if url.is_empty() {
            gx.toast(Toast::error("URL is required"));
            return;
        }

        let display_name = if name.is_empty() { url.clone() } else { name };

        let credentials = gx.data::<CredentialsProvider>();
        match credentials.create_environment(&url, &display_name).await {
            Ok(env) => {
                self.env_adding.set(false);
                self.reload_data(gx).await;
                gx.toast(Toast::success("Environment added"));
                gx.publish(EnvironmentAdded {
                    id: env.id,
                    url: env.url,
                    display_name: env.display_name,
                });
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to add environment: {}", e)));
            }
        }
    }

    #[handler]
    async fn env_delete(&self, gx: &GlobalContext) {
        let list_state = self.env_list.get();
        let Some(env_id) = list_state.last_activated else {
            gx.toast(Toast::error("Select an environment first"));
            return;
        };

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this environment?").title("Delete"))
            .await;
        if !confirmed {
            return;
        }

        let credentials = gx.data::<CredentialsProvider>();
        match credentials.delete_environment(env_id).await {
            Ok(_) => {
                self.reload_data(gx).await;
                gx.toast(Toast::success("Environment deleted"));
                gx.publish(EnvironmentRemoved { id: env_id });
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to delete: {}", e)));
            }
        }
    }

    // =========================================================================
    // Accounts tab handlers
    // =========================================================================

    #[handler]
    async fn acc_show_add(&self) {
        self.acc_adding.set(true);
        self.acc_add_client_id.set(String::new());
        self.acc_add_tenant_id.set(String::new());
        self.acc_add_name.set(String::new());
    }

    #[handler]
    async fn acc_cancel_add(&self) {
        self.acc_adding.set(false);
    }

    #[handler]
    async fn acc_confirm_add(&self, gx: &GlobalContext) {
        let client_id = self.acc_add_client_id.get();
        let tenant_id = self.acc_add_tenant_id.get();
        let name = self.acc_add_name.get();

        if client_id.is_empty() {
            gx.toast(Toast::error("Client ID is required"));
            return;
        }

        let display_name = if name.is_empty() {
            client_id.clone()
        } else {
            name
        };
        let tenant = if tenant_id.is_empty() {
            None
        } else {
            Some(tenant_id)
        };

        let credentials = gx.data::<CredentialsProvider>();
        let account = crate::credentials::Account {
            id: 0, // Will be assigned by DB
            display_name,
            auth_type: crate::credentials::AuthType::Browser,
            client_id,
            tenant_id: tenant,
            client_secret: None,
            username: None,
            password: None,
        };

        match credentials.create_account(&account).await {
            Ok(_) => {
                self.acc_adding.set(false);
                self.reload_data(gx).await;
                gx.toast(Toast::success("Account added"));
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to add account: {}", e)));
            }
        }
    }

    #[handler]
    async fn acc_delete(&self, gx: &GlobalContext) {
        let list_state = self.acc_list.get();
        let Some(acc_id) = list_state.last_activated else {
            gx.toast(Toast::error("Select an account first"));
            return;
        };

        let confirmed = gx
            .modal(ConfirmModal::with_message("Delete this account?").title("Delete"))
            .await;
        if !confirmed {
            return;
        }

        let credentials = gx.data::<CredentialsProvider>();
        match credentials.delete_account(acc_id).await {
            Ok(_) => {
                self.reload_data(gx).await;
                gx.toast(Toast::success("Account deleted"));
            }
            Err(e) => {
                gx.toast(Toast::error(format!("Failed to delete: {}", e)));
            }
        }
    }

    // =========================================================================
    // Layout and Pages (using page routing)
    // =========================================================================

    /// Shared layout wrapper with title and tab bar.
    fn layout(&self, content: Element) -> Element {
        let current = self.page();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill)
                style (bg: surface)
            {
                text (content: "Client Management") style (bold, fg: interact)

                row (gap: 2) {
                    button (label: "Active", hint: "1", id: "tab-active")
                        style (fg: if current == Page::Active { interact } else { muted })
                        on_activate: tab_active()
                    button (label: "Environments", hint: "2", id: "tab-environments")
                        style (fg: if current == Page::Environments { interact } else { muted })
                        on_activate: tab_environments()
                    button (label: "Accounts", hint: "3", id: "tab-accounts")
                        style (fg: if current == Page::Accounts { interact } else { muted })
                        on_activate: tab_accounts()
                }

                { content }
            }
        }
    }

    #[page(Active)]
    fn active_page(&self) -> Element {
        let status = self.connection_status.get();
        let (connect_label, status_text, status_color) = match status {
            ConnectionStatus::Connected => ("Re-authenticate", "Connected", "success"),
            ConnectionStatus::Expired => ("Re-authenticate", "Needs re-auth", "warning"),
            ConnectionStatus::NotConnected => ("Connect", "Not connected", "error"),
        };
        let status_indicator =
            Element::text("●").style(Style::new().foreground(Color::var(status_color)));

        page! {
            column (gap: 1, width: fill, height: fill) {
                select (state: self.env_select, id: "env-select", label: "Environment", placeholder: "Select environment...")
                    on_change: on_env_select()

                select (state: self.acc_select, id: "acc-select", label: "Account", placeholder: "Select account...")
                    on_change: on_acc_select()

                row (gap: 1) {
                    { status_indicator }
                    text (content: status_text)
                }

                // Spacer to push buttons to bottom
                row (height: fill)

                row (width: fill, justify: between) {
                    button (label: connect_label, id: "connect") on_activate: connect()
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }

    #[page(Environments)]
    fn environments_page(&self) -> Element {
        let is_adding = self.env_adding.get();

        if is_adding {
            return self.render_env_add_form();
        }

        page! {
            column (gap: 1, width: fill, height: fill) {
                list (state: self.env_list, id: "env-list", height: fill)
                    style (bg: background)

                row (width: fill, justify: between) {
                    row (gap: 1) {
                        button (label: "Add", id: "env-add") on_activate: env_show_add()
                        button (label: "Delete", id: "env-delete") on_activate: env_delete()
                    }
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }

    #[page(Accounts)]
    fn accounts_page(&self) -> Element {
        let is_adding = self.acc_adding.get();

        if is_adding {
            return self.render_acc_add_form();
        }

        page! {
            column (gap: 1, width: fill, height: fill) {
                list (state: self.acc_list, id: "acc-list", height: fill)
                    style (bg: background)

                row (width: fill, justify: between) {
                    row (gap: 1) {
                        button (label: "Add", id: "acc-add") on_activate: acc_show_add()
                        button (label: "Delete", id: "acc-delete") on_activate: acc_delete()
                    }
                    button (label: "Close", hint: "esc", id: "close") on_activate: close()
                }
            }
        }
    }

    // =========================================================================
    // Sub-forms (not using page routing - these are nested views)
    // =========================================================================

    fn render_env_add_form(&self) -> Element {
        page! {
            column (gap: 1, width: fill, height: fill) {
                text (content: "Add Environment") style (bold)

                input (state: self.env_add_url, id: "env-url", label: "URL", placeholder: "https://org.crm.dynamics.com")
                input (state: self.env_add_name, id: "env-name", label: "Name", placeholder: "My Environment")

                // Spacer to push buttons to bottom
                row (height: fill)

                row (width: fill, justify: between) {
                    button (label: "Cancel", id: "env-cancel") on_activate: env_cancel_add()
                    button (label: "Add", id: "env-confirm") on_activate: env_confirm_add()
                }
            }
        }
    }

    fn render_acc_add_form(&self) -> Element {
        page! {
            column (gap: 1, width: fill, height: fill) {
                text (content: "Add Account") style (bold)

                input (state: self.acc_add_client_id, id: "acc-client-id", label: "Client ID", placeholder: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
                input (state: self.acc_add_tenant_id, id: "acc-tenant-id", label: "Tenant ID", placeholder: "common (optional)")
                input (state: self.acc_add_name, id: "acc-name", label: "Name", placeholder: "My Account")

                // Spacer to push buttons to bottom
                row (height: fill)

                row (width: fill, justify: between) {
                    button (label: "Cancel", id: "acc-cancel") on_activate: acc_cancel_add()
                    button (label: "Add", id: "acc-confirm") on_activate: acc_confirm_add()
                }
            }
        }
    }
}
