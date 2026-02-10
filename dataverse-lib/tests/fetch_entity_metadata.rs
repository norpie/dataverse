use std::env;

use dataverse_lib::DataverseClient;
use dataverse_lib::auth::{AutoRefreshTokenProvider, PasswordFlow};
use dataverse_lib::model::Entity;

fn client_from_env() -> DataverseClient {
    let _ = dotenvy::dotenv();

    let client_id = env::var("DATAVERSE_CLIENT_ID").expect("DATAVERSE_CLIENT_ID not set");
    let client_secret =
        env::var("DATAVERSE_CLIENT_SECRET").expect("DATAVERSE_CLIENT_SECRET not set");
    let username = env::var("DATAVERSE_USERNAME").expect("DATAVERSE_USERNAME not set");
    let password = env::var("DATAVERSE_PASSWORD").expect("DATAVERSE_PASSWORD not set");
    let url = env::var("DATAVERSE_URL").expect("DATAVERSE_URL not set");

    let flow = PasswordFlow::new(&client_id, &client_secret, &username, &password);
    let provider = AutoRefreshTokenProvider::new(flow);

    DataverseClient::builder()
        .url(url)
        .token_provider(provider)
        .build()
}

#[tokio::test]
async fn fetch_contact_entity_metadata() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init();

    let client = client_from_env();

    let who = client.connect().await.expect("connect failed");
    eprintln!("Connected as user: {}", who.user_id);

    let metadata = client
        .metadata()
        .entity(Entity::logical("contact"))
        .bypass_cache()
        .execute()
        .await
        .expect("fetch entity metadata failed");

    eprintln!("Entity: {}", metadata.logical_name());
    eprintln!("  Attributes: {}", metadata.attributes.len());
    eprintln!("  State attributes: {}", metadata.state_attributes.len());
    eprintln!("  Status attributes: {}", metadata.status_attributes.len());
    eprintln!(
        "  Picklist attributes: {}",
        metadata.picklist_attributes.len()
    );
    eprintln!(
        "  Multi-select picklist attributes: {}",
        metadata.multi_select_picklist_attributes.len()
    );

    assert_eq!(metadata.logical_name(), "contact");
    assert!(
        !metadata.attributes.is_empty(),
        "expected at least one attribute"
    );
    assert!(
        !metadata.state_attributes.is_empty(),
        "expected at least one state attribute"
    );
    assert!(
        !metadata.status_attributes.is_empty(),
        "expected at least one status attribute"
    );
}
