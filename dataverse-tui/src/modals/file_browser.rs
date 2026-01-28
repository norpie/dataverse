//! File browser modal for selecting files from the filesystem.

use std::fs;
use std::path::{Path, PathBuf};

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Button, Input, List, ListItem, ListState, Select, SelectState, Text};

/// Result returned when user confirms the file browser modal.
#[derive(Clone, Debug)]
pub struct SaveFileResult {
    pub path: PathBuf,
    pub file_type: String,
}

/// A filesystem entry (file or directory).
#[derive(Clone, Debug)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

impl ListItem for FsEntry {
    type Key = String;

    fn key(&self) -> String {
        self.path.to_string_lossy().to_string()
    }

    fn render(&self) -> Element {
        let prefix = if self.is_dir { "> " } else { "  " };
        Element::text(&format!("{}{}", prefix, self.name))
    }
}

/// Modal for browsing and selecting files.
#[modal(size = Md)]
pub struct FileBrowserModal {
    current_dir: PathBuf,
    list: ListState<FsEntry>,
    #[state(skip)]
    file_types: Vec<String>,
    #[state(skip)]
    require_existing: bool,
    filename: String,
    file_type: SelectState<String>,
}

impl FileBrowserModal {
    /// Create a new file browser modal with accepted file types.
    pub fn new(initial_dir: impl AsRef<Path>, file_types: Vec<String>) -> Self {
        let current_dir = initial_dir.as_ref().to_path_buf();
        let entries = Self::read_dir(&current_dir, &file_types);

        // Build select options from file types
        let select_options: Vec<(String, &str)> = file_types
            .iter()
            .map(|ft| {
                let label = match ft.as_str() {
                    "csv" => "CSV (.csv)",
                    "xlsx" => "Excel (.xlsx)",
                    "json" => "JSON (.json)",
                    "xml" => "XML (.xml)",
                    _ => ft.as_str(),
                };
                (ft.clone(), label)
            })
            .collect();

        let default_type = file_types.first().cloned().unwrap_or_default();
        let file_type_state = SelectState::new(select_options).with_value(default_type);

        Self {
            current_dir: State::new(current_dir),
            list: State::new(ListState::new(entries)),
            file_types,
            require_existing: false,
            filename: State::new(String::new()),
            file_type: State::new(file_type_state),
            ..Default::default()
        }
    }

    /// Require that the selected file must exist (for opening existing files).
    pub fn require_existing(mut self) -> Self {
        self.require_existing = true;
        self
    }

    /// Set an initial filename (without extension - extension added from default file type).
    pub fn with_filename(self, filename: impl Into<String>) -> Self {
        let name = filename.into();
        let default_type = self.file_types.first().cloned().unwrap_or_default();
        let full_name = if default_type.is_empty() {
            name
        } else {
            format!("{}.{}", name, default_type)
        };
        self.filename.set(full_name);
        self
    }

    /// Replace file extension in filename if it matches a known file type.
    fn replace_extension(&self, new_ext: &str) {
        let filename = self.filename.get();
        if filename.is_empty() {
            return;
        }

        // Find if current filename has a known extension
        for ft in &self.file_types {
            let suffix = format!(".{}", ft);
            if filename.to_lowercase().ends_with(&suffix.to_lowercase()) {
                // Replace the extension
                let stem = &filename[..filename.len() - suffix.len()];
                self.filename.set(format!("{}.{}", stem, new_ext));
                return;
            }
        }

        // No known extension found, append new one
        self.filename.set(format!("{}.{}", filename, new_ext));
    }

    /// Read directory contents and return sorted entries.
    fn read_dir(dir: &Path, file_types: &[String]) -> Vec<FsEntry> {
        let mut entries = Vec::new();

        // Add parent directory entry if not at root
        if let Some(parent) = dir.parent() {
            entries.push(FsEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        // Read directory contents
        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files
                if name.starts_with('.') {
                    continue;
                }

                let is_dir = path.is_dir();

                // Apply filter for files only
                if !is_dir && !file_types.is_empty() {
                    let extension = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase());

                    let matches = extension
                        .map(|ext| file_types.iter().any(|f| f.to_lowercase() == ext))
                        .unwrap_or(false);

                    if !matches {
                        continue;
                    }
                }

                entries.push(FsEntry { name, path, is_dir });
            }
        }

        // Sort: ".." first, then directories, then files (all alphabetically)
        entries.sort_by(|a, b| {
            match (&a.name, &b.name, a.is_dir, b.is_dir) {
                (name_a, _, _, _) if name_a == ".." => std::cmp::Ordering::Less,
                (_, name_b, _, _) if name_b == ".." => std::cmp::Ordering::Greater,
                (_, _, true, false) => std::cmp::Ordering::Less,
                (_, _, false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        entries
    }

    /// Refresh the current directory listing.
    fn refresh(&self) {
        let current_dir = self.current_dir.get();
        let entries = Self::read_dir(&current_dir, &self.file_types);
        self.list.set(ListState::new(entries));
    }

    /// Navigate to a directory.
    fn navigate_to(&self, path: &Path) {
        self.current_dir.set(path.to_path_buf());
        self.refresh();
    }
}

#[modal_impl(Result = Option<SaveFileResult>)]
impl FileBrowserModal {
    fn default_result(&self) -> Option<SaveFileResult> {
        None
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel_modal);
        bind("backspace", navigate_up);
    }

    #[handler]
    async fn cancel_modal(&self, mx: &ModalContext<Option<SaveFileResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn confirm_path(&self, mx: &ModalContext<Option<SaveFileResult>>) {
        let filename = self.filename.get();
        let current_dir = self.current_dir.get();
        let file_type = self.file_type.with_ref(|s| s.value().cloned())
            .unwrap_or_else(|| self.file_types.first().cloned().unwrap_or_default());

        if filename.is_empty() {
            return; // Don't close without a filename
        }

        let path = current_dir.join(&filename);

        // If require_existing is true, validate that the file exists
        if self.require_existing && !path.exists() {
            return; // Don't close if file doesn't exist
        }

        mx.close(Some(SaveFileResult { path, file_type }));
    }

    #[handler]
    async fn on_file_type_change(&self) {
        let new_type = self.file_type.with_ref(|s| s.value().cloned());
        if let Some(ext) = new_type {
            self.replace_extension(&ext);
        }
    }

    #[handler]
    async fn navigate_up(&self) {
        let current_dir = self.current_dir.get();
        if let Some(parent) = current_dir.parent() {
            self.navigate_to(parent);
        }
    }

    #[handler]
    async fn on_list_activate(&self) {
        let focused_key = self.list.with_ref(|s| s.focused_key.clone());

        if let Some(key) = focused_key {
            let entry = self.list.with_ref(|s| {
                s.items.iter().find(|e| e.key() == key).cloned()
            });

            if let Some(entry) = entry {
                if entry.is_dir {
                    self.navigate_to(&entry.path);
                } else {
                    // Set full filename (with extension)
                    self.filename.set(entry.name.clone());

                    // Update file type dropdown if extension matches an available type
                    if let Some(ext) = entry.path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if self.file_types.iter().any(|ft| ft.to_lowercase() == ext_lower) {
                            self.file_type.update(|s| {
                                s.selection.selected.clear();
                                s.selection.selected.insert(ext_lower);
                            });
                        }
                    }
                }
            }
        }
    }

    fn element(&self) -> Element {
        let current_dir = self.current_dir.get();
        let dir_display = current_dir.to_string_lossy().to_string();
        let title = if self.require_existing { "Open File" } else { "Save File" };
        let confirm_label = if self.require_existing { "Open" } else { "Save" };

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                // Header
                column {
                    text (content: title) style (bold, fg: interact)
                    text (content: dir_display) style (fg: muted)
                }

                // Content
                column (height: fill, gap: 1) {
                    box_ (height: fill, width: fill) style (bg: background) {
                        list (state: self.list, id: "file-list", height: fill, width: fill)
                            on_activate: on_list_activate()
                    }
                    row (gap: 1, width: fill) {
                        input (state: self.filename, id: "filename-input", label: "Filename", width: fill)
                        select (state: self.file_type, id: "file-type", label: "Type", width: 20)
                            on_change: on_file_type_change()
                    }
                }

                // Footer
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel") on_activate: cancel_modal()
                    button (label: confirm_label, id: "confirm") on_activate: confirm_path()
                }
            }
        }
    }
}
