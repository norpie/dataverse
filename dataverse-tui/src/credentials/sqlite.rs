//! SQLite credentials backend.

use std::path::Path;

use async_sqlite::Client;
use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;

use super::models::Account;
use super::models::ActiveSession;
use super::models::AuthType;
use super::models::CachedTokens;
use super::models::Environment;
use super::CredentialsBackend;
use super::CredentialsError;

/// SQLite-backed credentials storage.
pub struct SqliteCredentialsBackend {
    client: Client,
}

impl SqliteCredentialsBackend {
    /// Create a new SQLite credentials backend at the given path.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, CredentialsError> {
        let client = async_sqlite::ClientBuilder::new()
            .path(path)
            .open()
            .await?;

        // Initialize schema
        client
            .conn(|conn| {
                conn.execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS environments (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        url TEXT NOT NULL UNIQUE,
                        display_name TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS accounts (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        display_name TEXT NOT NULL,
                        auth_type TEXT NOT NULL,
                        client_id TEXT NOT NULL,
                        tenant_id TEXT,
                        client_secret TEXT,
                        username TEXT,
                        password TEXT
                    );

                    CREATE TABLE IF NOT EXISTS tokens (
                        account_id INTEGER NOT NULL,
                        environment_id INTEGER NOT NULL,
                        access_token TEXT NOT NULL,
                        expires_at TEXT,
                        refresh_token TEXT,
                        PRIMARY KEY (account_id, environment_id),
                        FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                        FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS active_session (
                        id INTEGER PRIMARY KEY CHECK (id = 1),
                        account_id INTEGER,
                        environment_id INTEGER,
                        FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL,
                        FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE SET NULL
                    );

                    -- Ensure active_session has exactly one row
                    INSERT OR IGNORE INTO active_session (id, account_id, environment_id)
                    VALUES (1, NULL, NULL);
                    ",
                )
            })
            .await?;

        Ok(Self { client })
    }
}

#[async_trait]
impl CredentialsBackend for SqliteCredentialsBackend {
    // =========================================================================
    // Environments
    // =========================================================================

    async fn create_environment(
        &self,
        url: &str,
        display_name: &str,
    ) -> Result<Environment, CredentialsError> {
        let url = url.to_string();
        let display_name = display_name.to_string();
        let url_clone = url.clone();
        let display_name_clone = display_name.clone();

        let id = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO environments (url, display_name) VALUES (?, ?)",
                    rusqlite::params![&url_clone, &display_name_clone],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await?;

        Ok(Environment {
            id,
            url,
            display_name,
        })
    }

    async fn get_environment(&self, id: i64) -> Result<Option<Environment>, CredentialsError> {
        self.client
            .conn(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT id, url, display_name FROM environments WHERE id = ?")?;
                let mut rows = stmt.query([id])?;
                match rows.next()? {
                    Some(row) => Ok(Some(Environment {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        display_name: row.get(2)?,
                    })),
                    None => Ok(None),
                }
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn get_environment_by_url(
        &self,
        url: &str,
    ) -> Result<Option<Environment>, CredentialsError> {
        let url = url.to_string();
        self.client
            .conn(move |conn| {
                let mut stmt =
                    conn.prepare("SELECT id, url, display_name FROM environments WHERE url = ?")?;
                let mut rows = stmt.query([&url])?;
                match rows.next()? {
                    Some(row) => Ok(Some(Environment {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        display_name: row.get(2)?,
                    })),
                    None => Ok(None),
                }
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn list_environments(&self) -> Result<Vec<Environment>, CredentialsError> {
        self.client
            .conn(|conn| {
                let mut stmt =
                    conn.prepare("SELECT id, url, display_name FROM environments ORDER BY id")?;
                let rows = stmt.query_map([], |row| {
                    Ok(Environment {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        display_name: row.get(2)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn update_environment(
        &self,
        id: i64,
        url: &str,
        display_name: &str,
    ) -> Result<(), CredentialsError> {
        let url = url.to_string();
        let display_name = display_name.to_string();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE environments SET url = ?, display_name = ? WHERE id = ?",
                    rusqlite::params![&url, &display_name, id],
                )
            })
            .await?;

        Ok(())
    }

    async fn delete_environment(&self, id: i64) -> Result<(), CredentialsError> {
        self.client
            .conn(move |conn| conn.execute("DELETE FROM environments WHERE id = ?", [id]))
            .await?;

        Ok(())
    }

    // =========================================================================
    // Accounts
    // =========================================================================

    async fn create_account(&self, account: &Account) -> Result<Account, CredentialsError> {
        let display_name = account.display_name.clone();
        let auth_type = account.auth_type.as_str().to_string();
        let client_id = account.client_id.clone();
        let tenant_id = account.tenant_id.clone();
        let client_secret = account.client_secret.clone();
        let username = account.username.clone();
        let password = account.password.clone();

        let id = self
            .client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO accounts (display_name, auth_type, client_id, tenant_id, client_secret, username, password)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        &display_name,
                        &auth_type,
                        &client_id,
                        &tenant_id,
                        &client_secret,
                        &username,
                        &password
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .await?;

        let mut created = account.clone();
        created.id = id;
        Ok(created)
    }

    async fn get_account(&self, id: i64) -> Result<Option<Account>, CredentialsError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, display_name, auth_type, client_id, tenant_id, client_secret, username, password
                     FROM accounts WHERE id = ?",
                )?;
                let mut rows = stmt.query([id])?;
                match rows.next()? {
                    Some(row) => {
                        let auth_type_str: String = row.get(2)?;
                        let auth_type = AuthType::from_str(&auth_type_str)
                            .ok_or_else(|| rusqlite::Error::InvalidQuery)?;
                        Ok(Some(Account {
                            id: row.get(0)?,
                            display_name: row.get(1)?,
                            auth_type,
                            client_id: row.get(3)?,
                            tenant_id: row.get(4)?,
                            client_secret: row.get(5)?,
                            username: row.get(6)?,
                            password: row.get(7)?,
                        }))
                    }
                    None => Ok(None),
                }
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn list_accounts(&self) -> Result<Vec<Account>, CredentialsError> {
        self.client
            .conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, display_name, auth_type, client_id, tenant_id, client_secret, username, password
                     FROM accounts ORDER BY id",
                )?;
                let rows = stmt.query_map([], |row| {
                    let auth_type_str: String = row.get(2)?;
                    let auth_type = AuthType::from_str(&auth_type_str)
                        .ok_or_else(|| rusqlite::Error::InvalidQuery)?;
                    Ok(Account {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        auth_type,
                        client_id: row.get(3)?,
                        tenant_id: row.get(4)?,
                        client_secret: row.get(5)?,
                        username: row.get(6)?,
                        password: row.get(7)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn update_account(&self, account: &Account) -> Result<(), CredentialsError> {
        let id = account.id;
        let display_name = account.display_name.clone();
        let auth_type = account.auth_type.as_str().to_string();
        let client_id = account.client_id.clone();
        let tenant_id = account.tenant_id.clone();
        let client_secret = account.client_secret.clone();
        let username = account.username.clone();
        let password = account.password.clone();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE accounts SET display_name = ?, auth_type = ?, client_id = ?, tenant_id = ?, client_secret = ?, username = ?, password = ?
                     WHERE id = ?",
                    rusqlite::params![
                        &display_name,
                        &auth_type,
                        &client_id,
                        &tenant_id,
                        &client_secret,
                        &username,
                        &password,
                        id
                    ],
                )
            })
            .await?;

        Ok(())
    }

    async fn delete_account(&self, id: i64) -> Result<(), CredentialsError> {
        self.client
            .conn(move |conn| conn.execute("DELETE FROM accounts WHERE id = ?", [id]))
            .await?;

        Ok(())
    }

    // =========================================================================
    // Tokens
    // =========================================================================

    async fn get_tokens(
        &self,
        account_id: i64,
        env_id: i64,
    ) -> Result<Option<CachedTokens>, CredentialsError> {
        self.client
            .conn(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT access_token, expires_at, refresh_token FROM tokens
                     WHERE account_id = ? AND environment_id = ?",
                )?;
                let mut rows = stmt.query(rusqlite::params![account_id, env_id])?;
                match rows.next()? {
                    Some(row) => {
                        let expires_at_str: Option<String> = row.get(1)?;
                        let expires_at = expires_at_str
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc));
                        Ok(Some(CachedTokens {
                            access_token: row.get(0)?,
                            expires_at,
                            refresh_token: row.get(2)?,
                        }))
                    }
                    None => Ok(None),
                }
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn save_tokens(
        &self,
        account_id: i64,
        env_id: i64,
        tokens: &CachedTokens,
    ) -> Result<(), CredentialsError> {
        let access_token = tokens.access_token.clone();
        let expires_at = tokens.expires_at.map(|dt| dt.to_rfc3339());
        let refresh_token = tokens.refresh_token.clone();

        self.client
            .conn(move |conn| {
                conn.execute(
                    "INSERT INTO tokens (account_id, environment_id, access_token, expires_at, refresh_token)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT(account_id, environment_id) DO UPDATE SET
                         access_token = excluded.access_token,
                         expires_at = excluded.expires_at,
                         refresh_token = excluded.refresh_token",
                    rusqlite::params![account_id, env_id, &access_token, &expires_at, &refresh_token],
                )
            })
            .await?;

        Ok(())
    }

    async fn clear_tokens(&self, account_id: i64, env_id: i64) -> Result<(), CredentialsError> {
        self.client
            .conn(move |conn| {
                conn.execute(
                    "DELETE FROM tokens WHERE account_id = ? AND environment_id = ?",
                    rusqlite::params![account_id, env_id],
                )
            })
            .await?;

        Ok(())
    }

    // =========================================================================
    // Active Session
    // =========================================================================

    async fn get_active_session(&self) -> Result<ActiveSession, CredentialsError> {
        self.client
            .conn(|conn| {
                let mut stmt =
                    conn.prepare("SELECT account_id, environment_id FROM active_session WHERE id = 1")?;
                let mut rows = stmt.query([])?;
                match rows.next()? {
                    Some(row) => Ok(ActiveSession {
                        account_id: row.get(0)?,
                        environment_id: row.get(1)?,
                    }),
                    None => Ok(ActiveSession::default()),
                }
            })
            .await
            .map_err(CredentialsError::from)
    }

    async fn set_active_session(
        &self,
        account_id: Option<i64>,
        env_id: Option<i64>,
    ) -> Result<(), CredentialsError> {
        self.client
            .conn(move |conn| {
                conn.execute(
                    "UPDATE active_session SET account_id = ?, environment_id = ? WHERE id = 1",
                    rusqlite::params![account_id, env_id],
                )
            })
            .await?;

        Ok(())
    }
}
