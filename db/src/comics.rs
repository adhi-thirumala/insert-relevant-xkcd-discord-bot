use crate::error::{DatabaseError, Result};
use crate::models::Comics;
use crate::{Chunks, Database};
use libsql::params;

impl Database {
  /// Insert a new comic into the database
  pub async fn insert_comic(&self, comic: Comics) -> Result<()> {
    todo!()
  }

  /// Get a comic by its number
  pub async fn get_comic_by_number(&self, comic_number: u64) -> Result<Option<Comics>> {
    todo!()
  }

  /// Get a comic by its chunk
  pub async fn get_comic_by_chunk(&self, chunk: &Chunks) -> Result<Option<Comics>> {
    todo!()
  }

  /// Check if a comic exists
  pub async fn comic_exists(&self, comic_number: u64) -> Result<bool> {
    todo!()
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
    todo!()
  }

  /// Delete a comic (cascades to chunks via foreign key)
  pub async fn delete_comic(&self, comic_number: u64) -> Result<()> {
    todo!()
  }

  /// Get the highest comic number in database
  pub async fn get_max_comic_number(&self) -> Result<Option<u64>> {
    todo!()
  }

  /// Count total comics
  pub async fn count_comics(&self) -> Result<u64> {
    todo!()
  }

  /// Get all comic numbers (for update checking)
  pub async fn get_all_comic_numbers(&self) -> Result<Vec<u64>> {
    todo!()
  }

  /// Get comics that haven't been updated recently (for update checks)
  pub async fn get_comics_needing_update(&self, older_than_days: u64) -> Result<Vec<Comics>> {
    todo!()
  }

  /// Get a batch of comics by their numbers
  pub async fn get_comics_batch(&self, comic_numbers: &[u64]) -> Result<Vec<Comics>> {
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::Comics;

  async fn setup() -> Database {
    let db = Database::new(":memory:").await.unwrap();
    db
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
    assert!(db.get_max_comic_number().await.unwrap().is_none());
  }

  #[tokio::test]
  async fn test_get_max_comic_number() {
    let db = setup().await;
    db.insert_comic(make_comic(5)).await.unwrap();
    db.insert_comic(make_comic(100)).await.unwrap();
    db.insert_comic(make_comic(42)).await.unwrap();
    assert_eq!(db.get_max_comic_number().await.unwrap(), Some(100));
  }

  #[tokio::test]
  async fn test_count_comics() {
    let db = setup().await;
    assert_eq!(db.count_comics().await.unwrap(), 0);
    db.insert_comic(make_comic(1)).await.unwrap();
    db.insert_comic(make_comic(2)).await.unwrap();
    assert_eq!(db.count_comics().await.unwrap(), 2);
  }

  #[tokio::test]
  async fn test_get_all_comic_numbers() {
    let db = setup().await;
    db.insert_comic(make_comic(50)).await.unwrap();
    db.insert_comic(make_comic(10)).await.unwrap();
    db.insert_comic(make_comic(30)).await.unwrap();
    let numbers = db.get_all_comic_numbers().await.unwrap();
    assert_eq!(numbers, vec![10, 30, 50]);
  }

  #[tokio::test]
  async fn test_get_comics_needing_update() {
    let db = setup().await;
    let mut old = make_comic(1);
    old.updated_at = "2020-01-01T00:00:00Z".to_string();
    db.insert_comic(old).await.unwrap();
    db.insert_comic(make_comic(2)).await.unwrap();
    let old_comics = db.get_comics_needing_update(30).await.unwrap();
    assert_eq!(old_comics.len(), 1);
    assert_eq!(old_comics[0].comic_number, 1);
  }

  #[tokio::test]
  async fn test_get_comics_batch() {
    let db = setup().await;
    for i in 1..=5 {
      db.insert_comic(make_comic(i)).await.unwrap();
    }
    let batch = db.get_comics_batch(&[2, 4, 1]).await.unwrap();
    assert_eq!(batch.len(), 3);
  }

  #[tokio::test]
  async fn test_get_comics_batch_empty() {
    let db = setup().await;
    assert_eq!(db.get_comics_batch(&[]).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_get_comics_batch_some_missing() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    db.insert_comic(make_comic(3)).await.unwrap();
    let batch = db.get_comics_batch(&[1, 2, 3]).await.unwrap();
    assert_eq!(batch.len(), 2);
  }
}
