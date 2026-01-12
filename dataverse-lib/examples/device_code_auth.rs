//! Device code authentication example.
//!
//! Run with: cargo run --example device_code_auth
//!
//! Requires .env file with:
//! - DATAVERSE_CLIENT_ID
//! - DATAVERSE_TENANT_ID
//! - DATAVERSE_URL

use std::env;
use std::io::Write;

use dataverse_lib::auth::DeviceCodeFlow;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").expect("DATAVERSE_CLIENT_ID not set");
    let tenant_id = env::var("DATAVERSE_TENANT_ID").expect("DATAVERSE_TENANT_ID not set");
    let url = env::var("DATAVERSE_URL").expect("DATAVERSE_URL not set");

    let flow = DeviceCodeFlow::new(&client_id, &tenant_id);

    println!("Starting device code authentication...\n");

    let pending = flow.start(&url).await?;

    println!("========================================");
    println!("Code: {}", pending.info.user_code);
    println!("URL:  {}", pending.info.verification_url);
    println!("========================================\n");

    print!("Press Enter to open browser (or open the URL manually)...");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if let Err(e) = pending.info.open_browser() {
        eprintln!("Failed to open browser: {}", e);
        println!("Please open the URL manually.");
    }

    println!("\nWaiting for authentication...");

    let token = pending.wait().await?;

    println!("\nAuthentication successful!");
    println!("Token expires at: {:?}", token.expires_at);
    println!("Has refresh token: {}", token.can_refresh());

    Ok(())
}
