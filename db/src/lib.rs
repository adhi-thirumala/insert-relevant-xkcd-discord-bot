mod comics;
mod error;
mod metadata;
mod models;
mod schema;

use libsql::{Builder, Connection};
use std::path::Path;

pub use error::{DatabaseError, Result};
pub use models::{Chunks, Comics, Metadata, SectionType};

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
        Self::init(path).await
      }
    } else {
      Self::init(path).await
    }
  }
}

#[cfg(test)]
mod tests {}
