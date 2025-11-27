use libsql::{de, params};

use crate::error::{DatabaseError, Result};
use crate::{Database, Metadata};

impl Database {
  pub async fn get_metadata(&self, key: &str) -> Result<Metadata> {
    let mut stmt = self
      .conn
      .prepare("SELECT * FROM metadata WHERE key = ?")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    let row = stmt
      .query_row(params!(key))
      .await
      .map_err(|_| DatabaseError::MetadataNotFound(key.to_string()))?;

    de::from_row::<Metadata>(&row).map_err(|e| DatabaseError::Serialization(e.to_string()))
  }
}
