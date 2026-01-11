mod systems;

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Text;
use simplelog::{Config, LevelFilter, WriteLogger};

#[app]
struct DataverseTui {}

#[app_impl]
impl DataverseTui {
    fn element(&self) -> Element {
        page! {
            column (padding: 2, gap: 1) style (bg: background) {
                text (content: "Dataverse TUI") style (bold, fg: primary)
                text (content: "Press Ctrl+P to open launcher") style (fg: muted)
                text (content: "Press Ctrl+Q to quit") style (fg: muted)
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
