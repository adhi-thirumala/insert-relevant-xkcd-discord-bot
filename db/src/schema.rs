use libsql::Builder;

use crate::{
  Database,
  error::{DatabaseError, Result},
};
use std::path::Path;

/// Represents a database connection.
///
/// This struct contains a connection to a database.
///
///
impl Database {
  pub(crate) async fn init(path: impl AsRef<Path>) -> Result<Self> {
    let path = path.as_ref();

    // check if file exists
    if std::fs::metadata(path).is_ok() {
      return Err(DatabaseError::InitializationError(
        "File already exists".to_string(),
      ));
    }
    let db = Builder::new_local(path)
      .build()
      .await
      .map_err(|e| DatabaseError::LibSql(e))?;

    let database = Self {
      conn: db
        .connect()
        .map_err(|e| DatabaseError::Connection(e.to_string()))?,
    };
    database.create_tables().await?;
    Ok(database)
  }
  async fn create_tables(&self) -> Result<()> {
    let query = include_str!("../migrations/001_schema.sql");
    self
      .conn
      .execute_batch(query)
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    Ok(())
  }
}
