//! Browser authentication example (Authorization Code + PKCE).
//!
//! Run with: cargo run --example browser_auth
//!
//! Requires .env file with:
//! - DATAVERSE_CLIENT_ID
//! - DATAVERSE_TENANT_ID
//! - DATAVERSE_URL

use std::env;

use dataverse_lib::auth::BrowserFlow;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").expect("DATAVERSE_CLIENT_ID not set");
    let tenant_id = env::var("DATAVERSE_TENANT_ID").expect("DATAVERSE_TENANT_ID not set");
    let url = env::var("DATAVERSE_URL").expect("DATAVERSE_URL not set");

    let flow = BrowserFlow::new(&client_id, &tenant_id).redirect_port(8400);

    println!("Starting browser authentication...\n");

    let pending = flow.start(&url).await?;

    println!("Redirect URI: {}", pending.redirect_uri);
    println!("Opening browser...\n");

    if let Err(e) = pending.open_browser() {
        eprintln!("Failed to open browser: {}", e);
        println!("Please open this URL manually:");
        println!("{}\n", pending.auth_url);
    }

    println!("Waiting for callback...");

    let token = pending.wait().await?;

    println!("\nAuthentication successful!");
    println!("Token expires at: {:?}", token.expires_at);
    println!("Has refresh token: {}", token.can_refresh());

    Ok(())
}
