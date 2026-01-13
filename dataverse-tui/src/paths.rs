//! Platform-specific directory paths.
//!
//! Uses XDG on Linux, standard locations on macOS/Windows.

use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "norpie";
const APPLICATION: &str = "dataverse";

/// Get project directories, or None if home directory cannot be determined.
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
}

/// Get the data directory for persistent application data.
///
/// - Linux: `$XDG_DATA_HOME/dataverse` or `~/.local/share/dataverse`
/// - macOS: `~/Library/Application Support/dev.norpie.dataverse`
/// - Windows: `C:\Users\<User>\AppData\Roaming\norpie\dataverse\data`
pub fn data_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
}

/// Get the cache directory for temporary/regenerable data.
///
/// - Linux: `$XDG_CACHE_HOME/dataverse` or `~/.cache/dataverse`
/// - macOS: `~/Library/Caches/dev.norpie.dataverse`
/// - Windows: `C:\Users\<User>\AppData\Local\norpie\dataverse\cache`
pub fn cache_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.cache_dir().to_path_buf())
}

/// Get the config directory for configuration files.
///
/// - Linux: `$XDG_CONFIG_HOME/dataverse` or `~/.config/dataverse`
/// - macOS: `~/Library/Application Support/dev.norpie.dataverse`
/// - Windows: `C:\Users\<User>\AppData\Roaming\norpie\dataverse\config`
pub fn config_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

/// Get the path to the settings database.
pub fn settings_db() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("settings.db"))
}

/// Get the path to the credentials database.
pub fn credentials_db() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("credentials.db"))
}

/// Get the path to the latest log file.
pub fn log_file() -> Option<PathBuf> {
    cache_dir().map(|dir| dir.join("latest.log"))
}

/// Maximum number of old log files to keep.
const MAX_OLD_LOGS: usize = 25;

/// Rotate logs: rename latest.log to timestamped name, clean up old logs.
///
/// Call this at startup before creating the new log file.
pub fn rotate_logs() {
    let Some(cache) = cache_dir() else { return };
    let latest = cache.join("latest.log");

    // Rename existing latest.log to timestamped name
    if latest.exists() {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let archived = cache.join(format!("{}.log", timestamp));
        let _ = fs::rename(&latest, &archived);
    }

    // Clean up old logs
    cleanup_old_logs(&cache);
}

/// Remove old log files, keeping only the most recent MAX_OLD_LOGS.
fn cleanup_old_logs(cache_dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(cache_dir) else { return };

    // Collect log files (excluding latest.log)
    let mut logs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".log") && name != "latest.log"
        })
        .collect();

    // Sort by modification time (oldest first)
    logs.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

    // Remove oldest logs beyond limit
    if logs.len() > MAX_OLD_LOGS {
        for entry in logs.iter().take(logs.len() - MAX_OLD_LOGS) {
            let _ = fs::remove_file(entry.path());
        }
    }
}
