//! Data models for credentials storage.

use chrono::DateTime;
use chrono::Utc;

/// Authentication type for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    /// Browser-based OAuth2 with PKCE.
    Browser,
    /// Device code flow for headless authentication.
    DeviceCode,
    /// Password flow with confidential client (v1.0, requires client_secret).
    Password,
    /// Password flow with public client (v2.0).
    PublicPassword,
}

impl AuthType {
    /// Convert to string for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::Browser => "browser",
            AuthType::DeviceCode => "device_code",
            AuthType::Password => "password",
            AuthType::PublicPassword => "public_password",
        }
    }

    /// Parse from database string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "browser" => Some(AuthType::Browser),
            "device_code" => Some(AuthType::DeviceCode),
            "password" => Some(AuthType::Password),
            "public_password" => Some(AuthType::PublicPassword),
            _ => None,
        }
    }

    /// Whether this auth type uses v2.0 endpoints for refresh.
    pub fn uses_v2(&self) -> bool {
        match self {
            AuthType::Browser | AuthType::DeviceCode | AuthType::PublicPassword => true,
            AuthType::Password => false,
        }
    }

    /// Whether this auth type requires a client secret for refresh.
    pub fn requires_client_secret(&self) -> bool {
        matches!(self, AuthType::Password)
    }
}

/// A Dataverse environment (instance).
#[derive(Debug, Clone)]
pub struct Environment {
    pub id: i64,
    pub url: String,
    pub display_name: String,
}

/// An authentication account.
#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub display_name: String,
    pub auth_type: AuthType,
    pub client_id: String,
    /// Tenant ID. None for v1 password flow (uses "common").
    pub tenant_id: Option<String>,
    /// Client secret. Only for confidential client (Password auth type).
    pub client_secret: Option<String>,
    /// Username. Only for password flows.
    pub username: Option<String>,
    /// Password. Only for password flows.
    pub password: Option<String>,
}

impl Account {
    /// Create a new account for insertion (id will be assigned by database).
    pub fn new(
        display_name: String,
        auth_type: AuthType,
        client_id: String,
        tenant_id: Option<String>,
        client_secret: Option<String>,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            id: 0, // Will be set by database
            display_name,
            auth_type,
            client_id,
            tenant_id,
            client_secret,
            username,
            password,
        }
    }
}

/// Cached OAuth tokens for an account-environment pair.
#[derive(Debug, Clone)]
pub struct CachedTokens {
    pub access_token: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub refresh_token: Option<String>,
}

impl CachedTokens {
    /// Check if the token is expired or will expire within the given duration.
    pub fn is_expired_within(&self, buffer_secs: i64) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let buffer = chrono::Duration::seconds(buffer_secs);
                Utc::now() + buffer >= expires_at
            }
            None => false, // No expiry means it's valid
        }
    }

    /// Check if refresh is possible.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// The currently active session.
#[derive(Debug, Clone, Default)]
pub struct ActiveSession {
    pub account_id: Option<i64>,
    pub environment_id: Option<i64>,
}

impl ActiveSession {
    /// Check if a session is active.
    pub fn is_active(&self) -> bool {
        self.account_id.is_some() && self.environment_id.is_some()
    }
}
