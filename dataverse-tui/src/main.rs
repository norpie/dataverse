mod modals;
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
    let log_file = File::create("dataverse-tui.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(DataverseTui::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}
