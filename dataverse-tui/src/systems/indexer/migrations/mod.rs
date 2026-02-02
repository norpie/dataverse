//! Indexer database migrations.

use include_dir::Dir;
use include_dir::include_dir;

use crate::migrations::Migration;
use crate::migrations::MigrationError;

static DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/systems/indexer/migrations");

/// Load all indexer migrations.
pub fn load() -> Result<Vec<Migration>, MigrationError> {
    crate::migrations::load_from_dir(&DIR)
}
