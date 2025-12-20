//! Explorer Example
//!
//! A multi-view file explorer demo showcasing rafter's capabilities:
//! - Multiple views with view-scoped keybinds
//! - Modal dialogs (confirm, input)
//! - State management with automatic reactivity
//! - Keyboard navigation and vim-style keybinds
//! - Focus system with Tab navigation
//! - Toast notifications
//! - Theme-aware styling
//! - Configurable keybinds

mod modals;
mod views;

use std::fmt;
use std::fs::File;

use log::LevelFilter;
use rafter::color::Color;
use rafter::prelude::*;
use rafter::theme::{DefaultTheme, Theme};
use simplelog::{Config, WriteLogger};

use modals::{ConfirmModal, RenameModal};
use views::{DetailView, ListView};

// ============================================================================
// View Enum
// ============================================================================

#[derive(Debug, Clone, Default)]
pub enum View {
    #[default]
    List,
    Detail {
        index: usize,
    },
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            View::List => write!(f, "List"),
            View::Detail { .. } => write!(f, "Detail"),
        }
    }
}

// ============================================================================
// File Entry (simulated)
// ============================================================================

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

impl FileEntry {
    pub fn new(name: impl Into<String>, size: u64, is_dir: bool) -> Self {
        Self {
            name: name.into(),
            size,
            is_dir,
        }
    }

    pub fn size_display(&self) -> String {
        if self.is_dir {
            "<DIR>".to_string()
        } else if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        }
    }
}

impl ListItem for FileEntry {
    const HEIGHT: u16 = 1;

    fn render(&self, focused: bool, selected: bool) -> Node {
        let prefix = if selected { "[x] " } else { "[ ] " };
        let icon = if self.is_dir { "📁" } else { "📄" };
        let display_name = format!("{}{} {}", prefix, icon, self.name);
        let size_display = self.size_display();

        if focused {
            // Focused row - highlighted background, flex: 1 fills available width
            if self.is_dir {
                view! {
                    row (flex: 1, justify: space_between, bg: surface) {
                        text (bold, fg: secondary) { display_name }
                        text (fg: muted) { size_display }
                    }
                }
            } else {
                view! {
                    row (flex: 1, justify: space_between, bg: surface) {
                        text (bold, fg: primary) { display_name }
                        text (fg: muted) { size_display }
                    }
                }
            }
        } else {
            // Non-focused row
            if self.is_dir {
                view! {
                    row (flex: 1, justify: space_between) {
                        text (fg: secondary) { display_name }
                        text (fg: muted) { size_display }
                    }
                }
            } else {
                view! {
                    row (flex: 1, justify: space_between) {
                        text { display_name }
                        text (fg: muted) { size_display }
                    }
                }
            }
        }
    }
}

// ============================================================================
// Custom Theme
// ============================================================================

#[derive(Debug, Clone)]
struct ExplorerTheme {
    inner: DefaultTheme,
}

impl ExplorerTheme {
    fn new() -> Self {
        Self {
            inner: DefaultTheme {
                primary: Color::rgb(80, 180, 220),     // Cyan
                secondary: Color::rgb(180, 140, 255),  // Purple
                background: Color::rgb(20, 22, 30),    // Dark blue-gray
                surface: Color::rgb(35, 38, 50),       // Slightly lighter
                text: Color::rgb(220, 225, 235),       // Off-white
                text_muted: Color::rgb(120, 130, 150), // Gray
                error: Color::rgb(255, 90, 90),        // Red
                success: Color::rgb(90, 210, 90),      // Green
                warning: Color::rgb(255, 190, 50),     // Yellow/Orange
                info: Color::rgb(100, 160, 255),       // Blue
            },
        }
    }
}

impl Theme for ExplorerTheme {
    fn resolve(&self, name: &str) -> Option<Color> {
        self.inner.resolve(name)
    }

    fn color_names(&self) -> Vec<&'static str> {
        self.inner.color_names()
    }

    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Explorer App
// ============================================================================

#[app]
pub struct Explorer {
    /// Current view
    view: View,
    /// List of files
    files: List<FileEntry>,
    /// Status message
    status: String,
}

#[app_impl]
impl Explorer {
    async fn on_start(&self, cx: &AppContext) {
        // Initialize with sample files - enough to require scrolling
        let mut files = vec![
            FileEntry::new("Documents", 0, true),
            FileEntry::new("Pictures", 0, true),
            FileEntry::new("Downloads", 0, true),
            FileEntry::new("Music", 0, true),
            FileEntry::new("Videos", 0, true),
            FileEntry::new("Desktop", 0, true),
            FileEntry::new("Projects", 0, true),
            FileEntry::new("Archives", 0, true),
        ];

        // Add many files to demonstrate scrolling
        for i in 1..=50 {
            files.push(FileEntry::new(format!("file_{:03}.txt", i), i as u64 * 100, false));
        }

        self.files.set_items(files);
        // Enable multi-selection mode (Space to toggle, Ctrl+click, Shift+range)
        self.files.set_selection_mode(SelectionMode::Multiple);
        // Set initial cursor position
        self.files.set_cursor(0);
        self.status.set("Ready".to_string());
        cx.toast("Explorer loaded");
    }

    fn current_view(&self) -> Option<String> {
        Some(self.view.get().to_string())
    }

    // -------------------------------------------------------------------------
    // Global Keybinds (always active)
    // -------------------------------------------------------------------------

    #[keybinds]
    fn global_keys() -> Keybinds {
        keybinds! {
            "q" => quit,
            "?" => show_help,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    #[handler]
    async fn show_help(&self, cx: &AppContext) {
        cx.toast("q:quit  j/k:navigate  enter:open  backspace:back  d:delete  r:rename");
    }

    // -------------------------------------------------------------------------
    // List View Keybinds
    // -------------------------------------------------------------------------

    #[keybinds(view = List)]
    fn list_keys() -> Keybinds {
        keybinds! {
            // Note: j/k/g/G/enter are handled by List component internally
            // Only "l" needs a keybind for vim-style open
            "l" => open_selected,
            "d" => delete_selected,
            "r" => rename_selected,
            "n" => new_file,
        }
    }

    #[handler]
    async fn open_selected(&self, cx: &AppContext) {
        // Use activated index from list event, or fall back to cursor (for keybind)
        let selected = cx.list_activated_index().or_else(|| self.files.cursor());
        if let Some(selected) = selected {
            if let Some(file) = self.files.get(selected) {
                if file.is_dir {
                    cx.toast(format!("Opening folder: {}", file.name));
                    // In a real app, we'd load the directory contents
                } else {
                    // Navigate to detail view
                    self.view.set(View::Detail { index: selected });
                    self.status.set(format!("Viewing: {}", file.name));
                }
            }
        }
    }

    #[handler]
    async fn delete_selected(&self, cx: &AppContext) {
        if let Some(selected) = self.files.cursor() {
            if let Some(file) = self.files.get(selected) {
                let confirmed = cx
                    .modal(ConfirmModal::new(format!(
                        "Delete '{}'?",
                        file.name
                    )))
                    .await;

                if confirmed {
                    self.files.remove(selected);
                    cx.toast("File deleted");
                    self.status.set("Deleted".to_string());
                }
            }
        }
    }

    #[handler]
    async fn rename_selected(&self, cx: &AppContext) {
        if let Some(selected) = self.files.cursor() {
            if let Some(file) = self.files.get(selected) {
                if let Some(new_name) = cx.modal(RenameModal::new(file.name.clone())).await {
                    self.files.update(|f| {
                        if let Some(entry) = f.get_mut(selected) {
                            entry.name = new_name.clone();
                        }
                    });
                    cx.toast(format!("Renamed to '{}'", new_name));
                    self.status.set("Renamed".to_string());
                }
            }
        }
    }

    #[handler]
    async fn new_file(&self, cx: &AppContext) {
        if let Some(name) = cx.modal(RenameModal::with_title("New File", "")).await {
            if !name.is_empty() {
                self.files.push(FileEntry::new(name.clone(), 0, false));
                cx.toast(format!("Created '{}'", name));
                self.status.set("Created".to_string());
            }
        }
    }

    // -------------------------------------------------------------------------
    // Detail View Keybinds
    // -------------------------------------------------------------------------

    #[keybinds(view = Detail)]
    fn detail_keys() -> Keybinds {
        keybinds! {
            "backspace" | "h" | "escape" => back_to_list,
            "d" => delete_current,
            "r" => rename_current,
        }
    }

    #[handler]
    async fn back_to_list(&self) {
        self.view.set(View::List);
        self.status.set("Ready".to_string());
    }

    #[handler]
    async fn delete_current(&self, cx: &AppContext) {
        if let View::Detail { index } = self.view.get() {
            if let Some(file) = self.files.get(index) {
                let confirmed = cx
                    .modal(ConfirmModal::new(format!("Delete '{}'?", file.name)))
                    .await;

                if confirmed {
                    self.files.remove(index);
                    self.view.set(View::List);
                    cx.toast("File deleted");
                    self.status.set("Deleted".to_string());
                }
            }
        }
    }

    #[handler]
    async fn rename_current(&self, cx: &AppContext) {
        if let View::Detail { index } = self.view.get() {
            if let Some(file) = self.files.get(index) {
                if let Some(new_name) = cx.modal(RenameModal::new(file.name.clone())).await {
                    self.files.update(|f| {
                        if let Some(entry) = f.get_mut(index) {
                            entry.name = new_name.clone();
                        }
                    });
                    cx.toast(format!("Renamed to '{}'", new_name));
                    self.status.set("Renamed".to_string());
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // View Rendering
    // -------------------------------------------------------------------------

    fn view(&self) -> Node {
        let current_view = self.view.get();
        let status = self.status.get();

        match current_view {
            View::List => ListView::render(&self.files, &status),
            View::Detail { index } => {
                if let Some(file) = self.files.get(index) {
                    DetailView::render(&file, &status)
                } else {
                    ListView::render(&self.files, &status)
                }
            }
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("explorer.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new()
        .theme(ExplorerTheme::new())
        .start_with::<Explorer>()
        .await
    {
        eprintln!("Error: {}", e);
    }
}
