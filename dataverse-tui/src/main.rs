mod apps;
mod client_manager;
mod credentials;
mod file_io;
mod formatting;
mod lua;
mod migrations;
mod modals;
mod paths;
mod settings;
mod systems;
mod widgets;

use apps::migration::repository::MigrationRepository;

use std::fs::File;

use rafter::prelude::*;
use simplelog::{Config, LevelFilter, WriteLogger};

use apps::Welcome;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    init_directories()?;
    init_logging()?;

    let (settings, credentials, migrations) = tokio::try_join!(
        async {
            init_settings()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        },
        async {
            init_credentials()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        },
        async {
            init_migrations()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        },
    )?;

    Runtime::new()?
        .data(settings)
        .data(credentials)
        .data(migrations)
        .run(Welcome::default())
        .await?;

    Ok(())
}

fn init_directories() -> Result<(), std::io::Error> {
    if let Some(logs_dir) = paths::logs_dir() {
        std::fs::create_dir_all(&logs_dir)?;
    }
    if let Some(data_dir) = paths::data_dir() {
        std::fs::create_dir_all(&data_dir)?;
    }
    if let Some(cache_dir) = paths::cache_dir() {
        std::fs::create_dir_all(&cache_dir)?;
    }
    Ok(())
}

fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    paths::rotate_logs();
    let log_path = paths::log_file().unwrap_or_else(|| "latest.log".into());
    let log_file = File::create(&log_path)?;
    WriteLogger::init(LevelFilter::Warn, Config::default(), log_file)?;
    Ok(())
}

async fn init_settings() -> Result<settings::Settings, settings::SettingsError> {
    let settings_path = paths::settings_db().unwrap_or_else(|| "settings.db".into());
    let backend = settings::SqliteBackend::new(&settings_path).await?;
    let backend = std::sync::Arc::new(backend);
    settings::Settings::load(backend).await
}

async fn init_credentials()
-> Result<credentials::CredentialsProvider, credentials::CredentialsError> {
    let creds_path = paths::credentials_db().unwrap_or_else(|| "credentials.db".into());
    let backend = credentials::SqliteCredentialsBackend::new(&creds_path).await?;
    Ok(credentials::CredentialsProvider::new(backend))
}

async fn init_migrations()
-> Result<MigrationRepository, apps::migration::repository::RepositoryError> {
    let migrations_path = paths::migrations_db().unwrap_or_else(|| "migrations.db".into());
    MigrationRepository::new(&migrations_path).await
}
