//! Explorer Example
//!
//! A multi-page file explorer demo showcasing rafter's capabilities:
//! - Multiple pages with page-scoped keybinds
//! - Modal dialogs (confirm, input)
//! - State management with automatic reactivity
//! - Keyboard navigation and vim-style keybinds
//! - Focus system with Tab navigation
//! - Toast notifications
//! - Theme-aware styling
//! - Configurable keybinds

mod modals;
mod pages;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;

use log::LevelFilter;
use rafter::styling::color::Color;
use rafter::prelude::*;
use rafter::styling::theme::{DefaultTheme, Theme};
use simplelog::{Config, WriteLogger};

use modals::{ConfirmModal, RenameModal};
use pages::{DetailView, ListView};

// ============================================================================
// Virtual File System
// ============================================================================

/// A simple virtual file system for the explorer demo.
#[derive(Debug, Clone)]
struct VirtualFS {
    /// Map from path to list of entries in that directory
    directories: HashMap<String, Vec<FileEntry>>,
}

impl Default for VirtualFS {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualFS {
    fn new() -> Self {
        let mut directories = HashMap::new();

        // Root directory
        let root = vec![
            FileEntry::new("Documents", 0, true),
            FileEntry::new("Pictures", 0, true),
            FileEntry::new("Downloads", 0, true),
            FileEntry::new("Music", 0, true),
            FileEntry::new("Videos", 0, true),
            FileEntry::new("Desktop", 0, true),
            FileEntry::new("Projects", 0, true),
            FileEntry::new("Archives", 0, true),
            FileEntry::new("perftest", 0, true),
            FileEntry::new("readme.txt", 1024, false),
            FileEntry::new("config.json", 256, false),
        ];
        directories.insert("/".to_string(), root);

        // Documents directory
        let documents: Vec<FileEntry> = (1..=20)
            .map(|i| FileEntry::new(format!("document_{:02}.txt", i), i as u64 * 100, false))
            .collect();
        directories.insert("/Documents".to_string(), documents);

        // Pictures directory
        let pictures: Vec<FileEntry> = (1..=15)
            .map(|i| FileEntry::new(format!("photo_{:02}.jpg", i), i as u64 * 50000, false))
            .collect();
        directories.insert("/Pictures".to_string(), pictures);

        // Downloads directory
        let downloads = vec![
            FileEntry::new("installer.exe", 52428800, false),
            FileEntry::new("archive.zip", 10485760, false),
            FileEntry::new("movie.mp4", 734003200, false),
        ];
        directories.insert("/Downloads".to_string(), downloads);

        // Empty directories
        for name in ["Music", "Videos", "Desktop", "Archives"] {
            directories.insert(format!("/{}", name), vec![]);
        }

        // Projects directory with subdirectories
        let projects = vec![
            FileEntry::new("rust-project", 0, true),
            FileEntry::new("web-app", 0, true),
            FileEntry::new("notes.md", 2048, false),
        ];
        directories.insert("/Projects".to_string(), projects);

        directories.insert(
            "/Projects/rust-project".to_string(),
            vec![
                FileEntry::new("src", 0, true),
                FileEntry::new("Cargo.toml", 512, false),
                FileEntry::new("README.md", 1024, false),
            ],
        );

        directories.insert(
            "/Projects/rust-project/src".to_string(),
            vec![
                FileEntry::new("main.rs", 2048, false),
                FileEntry::new("lib.rs", 4096, false),
            ],
        );

        directories.insert(
            "/Projects/web-app".to_string(),
            vec![
                FileEntry::new("index.html", 1024, false),
                FileEntry::new("style.css", 512, false),
                FileEntry::new("app.js", 2048, false),
            ],
        );

        // Performance test directory with 100,000 items
        let perftest: Vec<FileEntry> = (1..=100_000)
            .map(|i| FileEntry::new(format!("file_{:06}.dat", i), i as u64 * 10, false))
            .collect();
        directories.insert("/perftest".to_string(), perftest);

        Self { directories }
    }

    /// Get entries for a path.
    fn get(&self, path: &str) -> Vec<FileEntry> {
        self.directories.get(path).cloned().unwrap_or_default()
    }

    /// Check if a path exists as a directory.
    fn is_dir(&self, path: &str) -> bool {
        self.directories.contains_key(path)
    }
}

// ============================================================================
// Page Enum
// ============================================================================

#[derive(Debug, Clone, Default)]
pub enum Page {
    #[default]
    List,
    Detail {
        index: usize,
    },
}

impl fmt::Display for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Page::List => write!(f, "List"),
            Page::Detail { .. } => write!(f, "Detail"),
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
        // Use composable helpers: checkbox + custom icons + default colors
        let checkbox = Self::selection_indicator(selected);
        let icon = if self.is_dir { "📁" } else { "📄" };
        let display_name = format!("{}{} {}", checkbox, icon, self.name);
        let size_display = self.size_display();

        // Build custom layout with icons and two-column display
        let content = page! {
            row (justify: space_between) {
                // Apply bold styling if focused, use directory color if is_dir
                if focused {
                    text (bold) { display_name }
                } else if self.is_dir {
                    text (fg: secondary) { display_name }
                } else {
                    text { display_name }
                }
                text (fg: muted) { size_display }
            }
        };

        // Apply default focus/selection colors
        Self::apply_default_style(content, focused, selected)
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
                validation_error: Color::rgb(255, 90, 90),
                validation_error_border: Color::rgb(255, 90, 90),
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
    /// Current page
    page: Page,
    /// List of files
    files: List<FileEntry>,
    /// Current path in the virtual filesystem
    path: String,
    /// Path history stack for back navigation
    path_stack: Vec<String>,
    /// Virtual filesystem
    #[state(skip)]
    fs: VirtualFS,
    /// Status message
    status: String,
}

#[app_impl]
impl Explorer {
    async fn on_start(&self, cx: &AppContext) {
        // Load root directory
        let files = self.fs.get("/");
        self.files.set_items(files);
        // Enable multi-selection mode (Space to toggle, Ctrl+click, Shift+range)
        self.files.set_selection_mode(SelectionMode::Multiple);
        // Set initial cursor position
        self.files.set_cursor(0);
        self.path.set("/".to_string());
        self.status.set("Ready".to_string());
        cx.toast("Explorer loaded");
    }

    fn current_page(&self) -> Option<String> {
        Some(self.page.get().to_string())
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
    // List Page Keybinds
    // -------------------------------------------------------------------------

    #[keybinds(page = List)]
    fn list_keys() -> Keybinds {
        keybinds! {
            // Note: j/k/g/G/enter are handled by List widget internally
            // Only "l" needs a keybind for vim-style open
            "l" => open_selected,
            "backspace" | "h" => back_directory,
            "d" => delete_selected,
            "r" => rename_selected,
            "n" => new_file,
        }
    }

    #[handler]
    async fn open_selected(&self, cx: &AppContext) {
        // Use activated index from list event, or fall back to cursor (for keybind)
        let selected = cx.activated_index().or_else(|| self.files.cursor());
        if let Some(selected) = selected
            && let Some(file) = self.files.get(selected)
        {
            if file.is_dir {
                // Navigate into directory
                let current_path = self.path.get();
                let new_path = if current_path == "/" {
                    format!("/{}", file.name)
                } else {
                    format!("{}/{}", current_path, file.name)
                };

                if self.fs.is_dir(&new_path) {
                    // Push current path to stack for back navigation
                    self.path_stack.update(|stack| stack.push(current_path));
                    // Load new directory
                    let entries = self.fs.get(&new_path);
                    self.files.set_items(entries);
                    self.files.set_cursor(0);
                    self.path.set(new_path.clone());
                    self.status.set(format!("Opened: {}", file.name));
                }
            } else {
                // Navigate to detail page
                self.page.set(Page::Detail { index: selected });
                self.status.set(format!("Viewing: {}", file.name));
            }
        }
    }

    #[handler]
    async fn back_directory(&self, cx: &AppContext) {
        let stack = self.path_stack.get();
        if let Some(parent_path) = stack.last().cloned() {
            // Pop from stack
            self.path_stack.update(|s| {
                s.pop();
            });
            // Load parent directory
            let entries = self.fs.get(&parent_path);
            self.files.set_items(entries);
            self.files.set_cursor(0);
            self.path.set(parent_path);
            self.status.set("Back".to_string());
        } else {
            cx.toast("Already at root");
        }
    }

    #[handler]
    async fn delete_selected(&self, cx: &AppContext) {
        if let Some(selected) = self.files.cursor() {
            if let Some(file) = self.files.get(selected) {
                let confirmed = cx
                    .modal(ConfirmModal::new(format!("Delete '{}'?", file.name)))
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
    // Detail Page Keybinds
    // -------------------------------------------------------------------------

    #[keybinds(page = Detail)]
    fn detail_keys() -> Keybinds {
        keybinds! {
            "backspace" | "h" | "escape" => back_to_list,
            "d" => delete_current,
            "r" => rename_current,
        }
    }

    #[handler]
    async fn back_to_list(&self) {
        self.page.set(Page::List);
        self.status.set("Ready".to_string());
    }

    #[handler]
    async fn delete_current(&self, cx: &AppContext) {
        if let Page::Detail { index } = self.page.get() {
            if let Some(file) = self.files.get(index) {
                let confirmed = cx
                    .modal(ConfirmModal::new(format!("Delete '{}'?", file.name)))
                    .await;

                if confirmed {
                    self.files.remove(index);
                    self.page.set(Page::List);
                    cx.toast("File deleted");
                    self.status.set("Deleted".to_string());
                }
            }
        }
    }

    #[handler]
    async fn rename_current(&self, cx: &AppContext) {
        if let Page::Detail { index } = self.page.get() {
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
    // Page Rendering
    // -------------------------------------------------------------------------

    fn page(&self) -> Node {
        let current_page = self.page.get();
        let status = self.status.get();
        let path = self.path.get();

        match current_page {
            Page::List => ListView::render(&self.files, &path, &status),
            Page::Detail { index } => {
                if let Some(file) = self.files.get(index) {
                    DetailView::render(&file, &status)
                } else {
                    ListView::render(&self.files, &path, &status)
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
