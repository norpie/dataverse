//! Integration tests for password authentication flows.
//!
//! These tests require real Azure AD credentials and are ignored by default.
//! To run them, create a `.env` file in the dataverse-lib directory with:
//!
//! ```env
//! # For PasswordFlow (confidential client, v1.0)
//! DATAVERSE_CLIENT_ID=your-client-id
//! DATAVERSE_CLIENT_SECRET=your-client-secret
//! DATAVERSE_USERNAME=user@example.com
//! DATAVERSE_PASSWORD=your-password
//! DATAVERSE_URL=https://org.crm.dynamics.com
//!
//! # For PublicClientPasswordFlow (public client, v2.0)
//! DATAVERSE_TENANT_ID=your-tenant-id
//! ```
//!
//! Then run: `cargo test -p dataverse-lib -- --ignored`

use std::env;

use dataverse_lib::auth::{PasswordFlow, PublicClientPasswordFlow};

fn load_confidential_env() -> Option<(String, String, String, String, String)> {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").ok()?;
    let client_secret = env::var("DATAVERSE_CLIENT_SECRET").ok()?;
    let username = env::var("DATAVERSE_USERNAME").ok()?;
    let password = env::var("DATAVERSE_PASSWORD").ok()?;
    let url = env::var("DATAVERSE_URL").ok()?;

    Some((client_id, client_secret, username, password, url))
}

fn load_public_env() -> Option<(String, String, String, String, String)> {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").ok()?;
    let tenant_id = env::var("DATAVERSE_TENANT_ID").ok()?;
    let username = env::var("DATAVERSE_USERNAME").ok()?;
    let password = env::var("DATAVERSE_PASSWORD").ok()?;
    let url = env::var("DATAVERSE_URL").ok()?;

    Some((client_id, tenant_id, username, password, url))
}

// =============================================================================
// PasswordFlow (confidential client, v1.0) tests
// =============================================================================

mod confidential_client {
    use super::*;

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_authenticate() {
        let (client_id, client_secret, username, password, url) = load_confidential_env()
            .expect("Missing required environment variables. See module docs.");

        let flow = PasswordFlow::new(&client_id, &client_secret, &username, &password);

        let token = flow
            .authenticate(&url)
            .await
            .expect("Authentication failed");

        assert!(
            !token.access_token.is_empty(),
            "Access token should not be empty"
        );
        assert!(
            token.expires_at.is_some(),
            "Token should have expiration time"
        );

        println!("Successfully authenticated!");
        println!("Token expires at: {:?}", token.expires_at);
        println!("Has refresh token: {}", token.can_refresh());
    }

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_authenticate_and_refresh() {
        let (client_id, client_secret, username, password, url) = load_confidential_env()
            .expect("Missing required environment variables. See module docs.");

        let flow = PasswordFlow::new(&client_id, &client_secret, &username, &password);

        let token = flow
            .authenticate(&url)
            .await
            .expect("Authentication failed");

        let refresh_token = match &token.refresh_token {
            Some(rt) => rt.clone(),
            None => {
                println!("No refresh token received, skipping refresh test");
                return;
            }
        };

        let refreshed_token = flow
            .refresh(&url, &refresh_token)
            .await
            .expect("Token refresh failed");

        assert!(
            !refreshed_token.access_token.is_empty(),
            "Refreshed token should not be empty"
        );
        assert!(
            refreshed_token.expires_at.is_some(),
            "Refreshed token should have expiration time"
        );
        assert_ne!(
            token.access_token, refreshed_token.access_token,
            "Refreshed token should be different"
        );

        println!("Successfully refreshed token!");
        println!("New token expires at: {:?}", refreshed_token.expires_at);
    }

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_invalid_credentials() {
        let (client_id, client_secret, _username, _password, url) = load_confidential_env()
            .expect("Missing required environment variables. See module docs.");

        let flow = PasswordFlow::new(
            &client_id,
            &client_secret,
            "invalid@example.com",
            "wrongpassword",
        );

        let result = flow.authenticate(&url).await;

        assert!(result.is_err(), "Should fail with invalid credentials");
        println!("Got expected error: {}", result.unwrap_err());
    }
}

// =============================================================================
// PublicClientPasswordFlow (public client, v2.0) tests
// =============================================================================

mod public_client {
    use super::*;

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_authenticate() {
        let (client_id, tenant_id, username, password, url) =
            load_public_env().expect("Missing required environment variables. See module docs.");

        let flow = PublicClientPasswordFlow::new(&client_id, &tenant_id, &username, &password);

        let token = flow
            .authenticate(&url)
            .await
            .expect("Authentication failed");

        assert!(
            !token.access_token.is_empty(),
            "Access token should not be empty"
        );
        assert!(
            token.expires_at.is_some(),
            "Token should have expiration time"
        );

        println!("Successfully authenticated!");
        println!("Token expires at: {:?}", token.expires_at);
        println!("Has refresh token: {}", token.can_refresh());
    }

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_authenticate_and_refresh() {
        let (client_id, tenant_id, username, password, url) =
            load_public_env().expect("Missing required environment variables. See module docs.");

        let flow = PublicClientPasswordFlow::new(&client_id, &tenant_id, &username, &password);

        let token = flow
            .authenticate(&url)
            .await
            .expect("Authentication failed");

        let refresh_token = match &token.refresh_token {
            Some(rt) => rt.clone(),
            None => {
                println!("No refresh token received, skipping refresh test");
                return;
            }
        };

        let refreshed_token = flow
            .refresh(&url, &refresh_token)
            .await
            .expect("Token refresh failed");

        assert!(
            !refreshed_token.access_token.is_empty(),
            "Refreshed token should not be empty"
        );
        assert!(
            refreshed_token.expires_at.is_some(),
            "Refreshed token should have expiration time"
        );
        assert_ne!(
            token.access_token, refreshed_token.access_token,
            "Refreshed token should be different"
        );

        println!("Successfully refreshed token!");
        println!("New token expires at: {:?}", refreshed_token.expires_at);
    }

    #[tokio::test]
    #[ignore = "requires real credentials in .env file"]
    async fn test_invalid_credentials() {
        let (client_id, tenant_id, _username, _password, url) =
            load_public_env().expect("Missing required environment variables. See module docs.");

        let flow = PublicClientPasswordFlow::new(
            &client_id,
            &tenant_id,
            "invalid@example.com",
            "wrongpassword",
        );

        let result = flow.authenticate(&url).await;

        assert!(result.is_err(), "Should fail with invalid credentials");
        println!("Got expected error: {}", result.unwrap_err());
    }
}
