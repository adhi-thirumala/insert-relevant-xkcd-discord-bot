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
    let row = stmt.query_row(params!(key)).await.map_err(|e| match e {
      libsql::Error::QueryReturnedNoRows => DatabaseError::MetadataNotFound(key.to_string()),
      _ => DatabaseError::QueryFailed(e.to_string()),
    })?;

    de::from_row::<Metadata>(&row).map_err(|e| DatabaseError::Serialization(e.to_string()))
  }

  pub async fn set_metadata(&self, key: &str, value: String) -> Result<()> {
    let stmt = self
      .conn
      .prepare(
        "INSERT INTO metadata (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    stmt
      .execute(params!(key, value))
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    Ok(())
  }
}
