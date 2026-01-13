mod credentials;
mod modals;
mod paths;
mod settings;
mod systems;
mod widgets;

use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use simplelog::{Config, LevelFilter, WriteLogger};

use widgets::Spinner;

#[app(name = "Dataverse", singleton)]
struct DataverseTui {}

#[app(name = "Test")]
struct TestApp {
    instance_id: String,
}

#[app_impl]
impl TestApp {
    #[on_start]
    async fn on_start(&self) {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 100000;
        self.instance_id.set(format!("{:05}", id));
    }

    fn title(&self) -> String {
        let id = self.instance_id.get();
        if id.is_empty() {
            "Loading...".to_string()
        } else {
            format!("Instance {}", id)
        }
    }

    fn element(&self) -> Element {
        let id = self.instance_id.get();
        let label = format!("Instance ID: {}", id);
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Test App") style (bold, fg: accent)
                text (content: label) style (fg: text)
            }
        }
    }
}

#[app_impl]
impl DataverseTui {
    fn title(&self) -> String {
        "Home - really long for testing".to_string()
    }

    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Dataverse TUI") style (bold, fg: accent)
                text (content: "Press Ctrl+P to open launcher") style (fg: muted)
                text (content: "Press Ctrl+Q to quit") style (fg: muted)
                spinner (id: "main-spinner")
            }
        }
    }
}

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

    let settings = init_settings().await?;
    let credentials = init_credentials().await?;

    Runtime::new()?
        .data(settings)
        .data(credentials)
        .run(DataverseTui::default())
        .await?;

    Ok(())
}

fn init_directories() -> Result<(), std::io::Error> {
    if let Some(cache_dir) = paths::cache_dir() {
        std::fs::create_dir_all(&cache_dir)?;
    }
    if let Some(data_dir) = paths::data_dir() {
        std::fs::create_dir_all(&data_dir)?;
    }
    Ok(())
}

fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    paths::rotate_logs();
    let log_path = paths::log_file().unwrap_or_else(|| "latest.log".into());
    let log_file = File::create(&log_path)?;
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)?;
    Ok(())
}

async fn init_settings() -> Result<settings::SettingsProvider, settings::SettingsError> {
    let settings_path = paths::settings_db().unwrap_or_else(|| "settings.db".into());
    let backend = settings::SqliteBackend::new(&settings_path).await?;
    Ok(settings::SettingsProvider::new(backend))
}

async fn init_credentials() -> Result<credentials::CredentialsProvider, credentials::CredentialsError> {
    let creds_path = paths::credentials_db().unwrap_or_else(|| "credentials.db".into());
    let backend = credentials::SqliteCredentialsBackend::new(&creds_path).await?;
    Ok(credentials::CredentialsProvider::new(backend))
}
