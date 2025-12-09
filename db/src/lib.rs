mod chunks;
mod comics;
mod error;
mod metadata;
mod models;
mod schema;

use libsql::{Builder, Connection};
use std::path::Path;

pub use chunks::ChunkSearchResult;
pub use error::{DatabaseError, Result};
pub use models::{Chunks, Comics, Metadata, SectionType};

/// The dimension of the embedding vectors (must match F32_BLOB(1024) in schema) for qwen 0.6b
pub const EMBEDDING_DIM: usize = 1024;

pub struct Database {
  pub(crate) conn: Connection,
}

impl Database {
  pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();
    // if file exists - open.
    if std::fs::metadata(path).is_ok() {
      // check if initialization
      let db = Builder::new_local(path)
        .build()
        .await
        .map_err(|e| DatabaseError::LibSql(e))?;
      let conn = db
        .connect()
        .map_err(|e| DatabaseError::Connection(e.to_string()))?;
      let database = Database { conn };
      let initialized: Metadata = database.get_metadata("INITIALIZED").await?;
      if initialized.value == "true" {
        Ok(database)
      } else {
        Err(DatabaseError::InitializationError(
          "Database Schema Mismatch - File exists".to_string(),
        ))
      }
    } else {
      Self::init(path).await
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_wal_enabled() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_path = temp_dir.path().join("test.db");

    let db = Database::new(&test_path).await.unwrap();

    let mut rows = db.conn.query("PRAGMA journal_mode", ()).await.unwrap();
    let row = rows.next().await.unwrap().expect("expected row");
    let mode: String = row.get(0).unwrap();

    assert_eq!(mode, "wal");
    // temp_dir auto-cleans on drop
  }
}
