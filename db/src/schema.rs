use libsql::{Builder, Connection};

use crate::error::DatabaseError;
use crate::models::Metadata;

/// Represents a database connection.
///
/// This struct contains a connection to a database.
///
///
pub struct Database {
  conn: Connection,
}

impl Database {
  pub async fn init(name: Option<&str>) -> Result<Self, DatabaseError> {
    // if file exists - return error
    //
    // check if file exists
    if std::fs::metadata(name.unwrap_or("xkcd_discord_bot_tables.db")).is_ok() {
      return Err(DatabaseError::InitializationError(
        "File already exists".to_string(),
      ));
    }
    let db = Builder::new_local(name.unwrap_or("xkcd_discord_bot_tables.db"))
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

  pub async fn create_tables(&self) -> Result<(), DatabaseError> {
    let query = include_str!("../migrations/001_schema.sql");
    self
      .conn
      .execute_batch(query)
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    Ok(())
  }
}
