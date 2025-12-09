use chrono::{DateTime, Utc};
use futures::{StreamExt, TryFutureExt, TryStreamExt};
use libsql::{Rows, de, params};

use crate::error::Result;
use crate::models::Comics;
use crate::{Database, DatabaseError, chunks};

async fn into_comic_vec(rows: Rows) -> Result<Vec<Comics>> {
  rows
    .into_stream()
    .map(|res| res.map_err(|e| DatabaseError::QueryFailed(e.to_string())))
    .and_then(|row| async move {
      de::from_row::<Comics>(&row).map_err(|e| DatabaseError::Serialization(e.to_string()))
    })
    .try_collect()
    .map_err(|e| DatabaseError::Serialization(e.to_string()))
    .await
}

impl Database {
  /// Insert a new comic into the database.
  ///
  /// # Errors
  /// Returns an error if:
  /// - A comic with the same `comic_number` already exists in the database
  /// - The database connection fails
  ///
  /// # Required fields
  /// All fields in the `Comics` struct are required. Ensure that:
  /// - `comic_number` is unique
  /// - `title` is non-empty
  /// - Timestamps are in the correct format
  pub async fn insert_comic(&self, comic: Comics) -> Result<()> {
    let stmt = self
      .conn
      .prepare(
        "INSERT INTO xkcd_comics (
          comic_number,
          title,
          url,
          xkcd_url,
          hover_text,
          last_revision_id,
          last_revision_timestamp,
          scraped_at,
          updated_at
          ) VALUES (
          ?,
          ?,
          ?,
          ?,
          ?,
          ?,
          ?,
          ?,
          ?
          )",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    stmt
      .execute(params![
        comic.comic_number,
        comic.title,
        comic.url,
        comic.xkcd_url,
        comic.hover_text,
        comic.last_revision_id,
        comic.last_revision_timestamp,
        comic.scraped_at,
        comic.updated_at,
      ])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    Ok(())
  }

  /// Get a comic by its number
  pub async fn get_comic_by_number(&self, comic_number: u64) -> Result<Option<Comics>> {
    let mut stmt = self
      .conn
      .prepare("SELECT * FROM xkcd_comics WHERE comic_number = ?")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    match stmt.query_row(params![comic_number]).await {
      Ok(row) => Ok(Some(
        de::from_row::<Comics>(&row).map_err(|e| DatabaseError::Serialization(e.to_string()))?,
      )),
      Err(libsql::Error::QueryReturnedNoRows) => Ok(None),
      Err(e) => Err(DatabaseError::QueryFailed(e.to_string())),
    }
  }

  /// Check if a comic exists
  pub async fn comic_exists(&self, comic_number: u64) -> Result<bool> {
    self
      .get_comic_by_number(comic_number)
      .await
      .map(|comic| comic.is_some())
  }

  /// Update comic metadata when wiki is updated
  /// Returns an error if the comic does not exist
  pub async fn update_comic(
    &self,
    comic_number: u64,
    last_revision_id: u64,
    last_revision_timestamp: String,
    updated_at: String,
  ) -> Result<()> {
    let stmt = self
      .conn
      .prepare(
        "UPDATE xkcd_comics SET last_revision_id = ?, last_revision_timestamp = ?, updated_at = ? WHERE comic_number = ?"
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    let rows_affected = stmt
      .execute(params![
        last_revision_id,
        last_revision_timestamp,
        updated_at,
        comic_number
      ])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    if rows_affected == 0 {
      Err(DatabaseError::InvalidComicNumber(comic_number))
    } else {
      Ok(())
    }
  }

  /// Delete a comic (cascades to chunks via foreign key).
  ///
  /// Returns an error if the comic doesn't exist.
  pub async fn delete_comic(&self, comic_number: u64) -> Result<()> {
    let stmt = self
      .conn
      .prepare("DELETE FROM xkcd_comics WHERE comic_number = ?")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    let rows_affected = stmt
      .execute(params![comic_number])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    if rows_affected == 0 {
      Err(DatabaseError::InvalidComicNumber(comic_number))
    } else {
      Ok(())
    }
  }

  /// Get the highest comic number in database
  pub async fn get_max_comic_number(&self) -> Result<u64> {
    let mut stmt = self
      .conn
      .prepare("SELECT MAX(comic_number) FROM xkcd_comics")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    let row = stmt
      .query_row(params![])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    match row.get(0) {
      Ok(Some(max_comic_number)) => Ok(max_comic_number),
      Ok(None) => Err(DatabaseError::NoComicsFound),
      Err(e) => Err(DatabaseError::RowParseFailed(e.to_string())),
    }
  }

  /// Get comics that haven't been updated recently (for update checks)
  pub async fn get_comics_needing_update(&self, older_than: DateTime<Utc>) -> Result<Vec<Comics>> {
    let stmt = self
      .conn
      .prepare("SELECT * FROM xkcd_comics WHERE updated_at < ?")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    let rows = stmt
      .query(params![older_than.to_rfc3339()])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    into_comic_vec(rows).await
  }

  /// Get a batch of comics by their numbers.
  ///
  /// Returns only the comics that exist in the database. Non-existent comic
  /// numbers are silently skipped.
  ///
  /// The returned vector contains the found comics in ascending order of comic number,
  /// regardless of the order of the input slice. Duplicate comic numbers in the input
  /// are returned only once.
  ///
  /// # Parameters
  /// - `comic_numbers`: Slice of comic numbers to retrieve
  ///
  /// # Returns
  /// A vector of comics that were found. May be shorter than the input slice
  /// if some comics don't exist. Returns an empty vector if no comics are found.
  pub async fn get_comics_batch(&self, comic_numbers: Vec<u64>) -> Result<Vec<Comics>> {
    let stmt = self
      .conn
      .prepare("SELECT * FROM xkcd_comics WHERE comic_number IN (SELECT value FROM json_each(?))")
      .await?;

    let rows = stmt
      .query(params![chunks::vec_to_json_string(comic_numbers)])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    into_comic_vec(rows).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::Comics;

  async fn setup() -> Database {
    Database::new(":memory:").await.unwrap()
  }

  fn make_comic(number: u64) -> Comics {
    Comics {
      comic_number: number,
      title: format!("Comic {}", number),
      url: format!("https://explainxkcd.com/{}", number),
      xkcd_url: format!("https://xkcd.com/{}", number),
      hover_text: Some(format!("Hover text {}", number)),
      last_revision_id: 12345,
      last_revision_timestamp: "20250127000000".to_string(),
      scraped_at: "2025-01-27T00:00:00Z".to_string(),
      updated_at: "2025-01-27T00:00:00Z".to_string(),
    }
  }

  #[tokio::test]
  async fn test_insert_comic() {
    let db = setup().await;
    let comic = make_comic(1);
    let result = db.insert_comic(comic.clone()).await;
    assert!(result.is_ok());
  }

  #[tokio::test]
  async fn test_insert_duplicate_comic_fails() {
    let db = setup().await;
    let comic = make_comic(1);
    db.insert_comic(comic.clone()).await.unwrap();
    let result = db.insert_comic(comic).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_get_comic_exists() {
    let db = setup().await;
    let comic = make_comic(42);
    db.insert_comic(comic.clone()).await.unwrap();
    let result = db.get_comic_by_number(42).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().comic_number, 42);
  }

  #[tokio::test]
  async fn test_get_comic_not_found() {
    let db = setup().await;
    let result = db.get_comic_by_number(999).await.unwrap();
    assert!(result.is_none());
  }

  #[tokio::test]
  async fn test_comic_exists() {
    let db = setup().await;
    db.insert_comic(make_comic(100)).await.unwrap();
    assert!(db.comic_exists(100).await.unwrap());
    assert!(!db.comic_exists(101).await.unwrap());
  }

  #[tokio::test]
  async fn test_update_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(50)).await.unwrap();
    let result = db
      .update_comic(
        50,
        99999,
        "20250128000000".to_string(),
        "2025-01-28T00:00:00Z".to_string(),
      )
      .await;
    assert!(result.is_ok());
    let updated = db.get_comic_by_number(50).await.unwrap().unwrap();
    assert_eq!(updated.last_revision_id, 99999);
  }

  #[tokio::test]
  async fn test_update_nonexistent_comic_fails() {
    let db = setup().await;
    let result = db
      .update_comic(
        999,
        12345,
        "20250127000000".to_string(),
        "2025-01-27T00:00:00Z".to_string(),
      )
      .await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_delete_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(10)).await.unwrap();
    assert!(db.comic_exists(10).await.unwrap());
    db.delete_comic(10).await.unwrap();
    assert!(!db.comic_exists(10).await.unwrap());
  }

  #[tokio::test]
  async fn test_delete_nonexistent_comic_fails() {
    let db = setup().await;
    let result = db.delete_comic(999).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_get_max_comic_number_empty_db() {
    let db = setup().await;
    assert!(
      db.get_max_comic_number()
        .await
        .is_err_and(|e| matches!(e, DatabaseError::NoComicsFound))
    );
  }

  #[tokio::test]
  async fn test_get_max_comic_number() {
    let db = setup().await;
    db.insert_comic(make_comic(5)).await.unwrap();
    db.insert_comic(make_comic(100)).await.unwrap();
    db.insert_comic(make_comic(42)).await.unwrap();
    assert_eq!(db.get_max_comic_number().await.unwrap(), 100);
  }

  #[tokio::test]
  async fn test_get_comics_needing_update() {
    let db = setup().await;
    let mut old = make_comic(1);
    old.updated_at = "2020-01-01T00:00:00Z".to_string();
    db.insert_comic(old).await.unwrap();
    db.insert_comic(make_comic(2)).await.unwrap();
    let cutoff = "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    let old_comics = db.get_comics_needing_update(cutoff).await.unwrap();
    assert_eq!(old_comics.len(), 1);
    assert_eq!(old_comics[0].comic_number, 1);
  }

  #[tokio::test]
  async fn test_get_comics_batch() {
    let db = setup().await;
    for i in 1..=5 {
      db.insert_comic(make_comic(i)).await.unwrap();
    }
    let batch = db.get_comics_batch([2, 4, 1].to_vec()).await.unwrap();
    assert_eq!(batch.len(), 3);
  }

  #[tokio::test]
  async fn test_get_comics_batch_empty() {
    let db = setup().await;
    assert_eq!(db.get_comics_batch(Vec::new()).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_get_comics_batch_some_missing() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    db.insert_comic(make_comic(3)).await.unwrap();
    let batch = db.get_comics_batch([1, 2, 3].to_vec()).await.unwrap();
    assert_eq!(batch.len(), 2);
  }
}
