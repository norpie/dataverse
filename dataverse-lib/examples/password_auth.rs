//! Password authentication example (confidential client, v1.0).
//!
//! Run with: cargo run --example password_auth
//!
//! Requires .env file with:
//! - DATAVERSE_CLIENT_ID
//! - DATAVERSE_CLIENT_SECRET
//! - DATAVERSE_USERNAME
//! - DATAVERSE_PASSWORD
//! - DATAVERSE_URL

use std::env;

use dataverse_lib::auth::PasswordFlow;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").expect("DATAVERSE_CLIENT_ID not set");
    let client_secret = env::var("DATAVERSE_CLIENT_SECRET").expect("DATAVERSE_CLIENT_SECRET not set");
    let username = env::var("DATAVERSE_USERNAME").expect("DATAVERSE_USERNAME not set");
    let password = env::var("DATAVERSE_PASSWORD").expect("DATAVERSE_PASSWORD not set");
    let url = env::var("DATAVERSE_URL").expect("DATAVERSE_URL not set");

    let flow = PasswordFlow::new(&client_id, &client_secret, &username, &password);

    println!("Authenticating...\n");

    let token = flow.authenticate(&url).await?;

    println!("Authentication successful!");
    println!("Token expires at: {:?}", token.expires_at);
    println!("Has refresh token: {}", token.can_refresh());

    if let Some(refresh_token) = &token.refresh_token {
        println!("\nRefreshing token...");
        let refreshed = flow.refresh(&url, refresh_token).await?;
        println!("Token refreshed!");
        println!("New token expires at: {:?}", refreshed.expires_at);
    }

    Ok(())
}
