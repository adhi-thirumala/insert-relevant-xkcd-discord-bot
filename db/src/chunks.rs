use crate::{Database, Chunks, EMBEDDING_DIM};
use crate::error::{DatabaseError, Result};
use crate::models::SectionType;
use libsql::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSearchResult {
  pub chunk_id: u64,
  pub comic_number: u64,
  pub chunk_text: String,
  pub section_type: Option<String>,
  pub comic_title: String,
  pub xkcd_url: String,
  pub hover_text: Option<String>,
  pub distance: f32,
}

// Helper functions
fn validate_embedding(embedding: &[f32]) -> Result<()> {
  todo!()
}

fn vec_to_f32_blob(embedding: &[f32]) -> Vec<u8> {
  todo!()
}

fn f32_blob_to_vec(blob: &[u8]) -> Vec<f32> {
  todo!()
}

impl Database {
  pub async fn insert_chunk(&self, chunk: Chunks) -> Result<()> {
    todo!()
  }

  pub async fn insert_chunks_batch(&self, chunks: &[Chunks]) -> Result<()> {
    todo!()
  }

  pub async fn get_chunks_for_comic(&self, comic_number: u64) -> Result<Vec<Chunks>> {
    todo!()
  }

  pub async fn delete_chunks_for_comic(&self, comic_number: u64) -> Result<u64> {
    todo!()
  }

  pub async fn vector_search(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<ChunkSearchResult>> {
    todo!()
  }

  pub async fn vector_search_filtered(&self, query_embedding: &[f32], top_k: usize, comic_numbers: &[u64]) -> Result<Vec<ChunkSearchResult>> {
    todo!()
  }

  pub async fn count_chunks(&self) -> Result<u64> {
    todo!()
  }

  pub async fn avg_chunks_per_comic(&self) -> Result<f64> {
    todo!()
  }

  pub async fn count_chunks_for_comic(&self, comic_number: u64) -> Result<u64> {
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::{Chunks, Comics, SectionType};
  use crate::EMBEDDING_DIM;

  async fn setup() -> Database {
    Database::new(":memory:").await.unwrap()
  }

  fn make_comic(n: u64) -> Comics {
    Comics {
      comic_number: n,
      title: format!("C{}", n),
      url: format!("https://explainxkcd.com/{}", n),
      xkcd_url: format!("https://xkcd.com/{}", n),
      hover_text: Some(format!("H{}", n)),
      last_revision_id: 12345,
      last_revision_timestamp: "20250127000000".to_string(),
      scraped_at: "2025-01-27T00:00:00Z".to_string(),
      updated_at: "2025-01-27T00:00:00Z".to_string(),
    }
  }

  fn make_chunk(comic: u64, idx: u64) -> Chunks {
    Chunks {
      id: 0,
      comic_number: comic,
      chunk_text: format!("Chunk {}", idx),
      chunk_index: idx,
      section_type: Some(SectionType::Explanation),
      embedding: vec![0.5; EMBEDDING_DIM],
    }
  }

  #[tokio::test]
  async fn test_insert_chunk() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    assert!(db.insert_chunk(make_chunk(1, 0)).await.is_ok());
  }

  #[tokio::test]
  async fn test_insert_chunk_wrong_embedding_size() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut chunk = make_chunk(1, 0);
    chunk.embedding = vec![0.0; 100];
    assert!(db.insert_chunk(chunk).await.is_err());
  }

  #[tokio::test]
  async fn test_insert_chunk_nonexistent_comic_fails() {
    let db = setup().await;
    assert!(db.insert_chunk(make_chunk(999, 0)).await.is_err());
  }

  #[tokio::test]
  async fn test_insert_chunks_batch() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let chunks = vec![make_chunk(1, 0), make_chunk(1, 1)];
    assert!(db.insert_chunks_batch(&chunks).await.is_ok());
    assert_eq!(db.get_chunks_for_comic(1).await.unwrap().len(), 2);
  }

  #[tokio::test]
  async fn test_insert_chunks_batch_validates_all_embeddings() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut bad = make_chunk(1, 1);
    bad.embedding = vec![0.0; 50];
    let chunks = vec![make_chunk(1, 0), bad, make_chunk(1, 2)];
    assert!(db.insert_chunks_batch(&chunks).await.is_err());
    assert_eq!(db.get_chunks_for_comic(1).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_get_chunks_for_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(42)).await.unwrap();
    db.insert_chunk(make_chunk(42, 2)).await.unwrap();
    db.insert_chunk(make_chunk(42, 0)).await.unwrap();
    db.insert_chunk(make_chunk(42, 1)).await.unwrap();
    let chunks = db.get_chunks_for_comic(42).await.unwrap();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].chunk_index, 0);
    assert_eq!(chunks[1].chunk_index, 1);
    assert_eq!(chunks[2].chunk_index, 2);
  }

  #[tokio::test]
  async fn test_get_chunks_for_nonexistent_comic() {
    let db = setup().await;
    assert_eq!(db.get_chunks_for_comic(999).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_delete_chunks_for_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(10)).await.unwrap();
    for i in 0..3 {
      db.insert_chunk(make_chunk(10, i)).await.unwrap();
    }
    let deleted = db.delete_chunks_for_comic(10).await.unwrap();
    assert_eq!(deleted, 3);
    assert_eq!(db.get_chunks_for_comic(10).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_delete_chunks_returns_zero_if_none() {
    let db = setup().await;
    assert_eq!(db.delete_chunks_for_comic(999).await.unwrap(), 0);
  }

  #[tokio::test]
  async fn test_vector_search() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    db.insert_comic(make_comic(2)).await.unwrap();
    let mut c1 = make_chunk(1, 0);
    c1.embedding = vec![1.0; EMBEDDING_DIM];
    let mut c2 = make_chunk(2, 0);
    c2.embedding = vec![0.0; EMBEDDING_DIM];
    db.insert_chunk(c1).await.unwrap();
    db.insert_chunk(c2).await.unwrap();
    let query = vec![0.9; EMBEDDING_DIM];
    let results = db.vector_search(&query, 2).await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].comic_number, 1);
  }

  #[tokio::test]
  async fn test_vector_search_wrong_size() {
    let db = setup().await;
    let query = vec![0.5; 100];
    assert!(db.vector_search(&query, 10).await.is_err());
  }

  #[tokio::test]
  async fn test_vector_search_filtered() {
    let db = setup().await;
    for i in 1..=3 {
      db.insert_comic(make_comic(i)).await.unwrap();
      db.insert_chunk(make_chunk(i, 0)).await.unwrap();
    }
    let query = vec![0.5; EMBEDDING_DIM];
    let results = db.vector_search_filtered(&query, 10, &[1, 3]).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.comic_number == 1));
    assert!(!results.iter().any(|r| r.comic_number == 2));
  }

  #[tokio::test]
  async fn test_vector_search_filtered_empty() {
    let db = setup().await;
    let query = vec![0.5; EMBEDDING_DIM];
    assert_eq!(db.vector_search_filtered(&query, 10, &[]).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_count_chunks() {
    let db = setup().await;
    assert_eq!(db.count_chunks().await.unwrap(), 0);
    db.insert_comic(make_comic(1)).await.unwrap();
    db.insert_chunk(make_chunk(1, 0)).await.unwrap();
    db.insert_chunk(make_chunk(1, 1)).await.unwrap();
    assert_eq!(db.count_chunks().await.unwrap(), 2);
  }

  #[tokio::test]
  async fn test_avg_chunks_per_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    for i in 0..3 {
      db.insert_chunk(make_chunk(1, i)).await.unwrap();
    }
    db.insert_comic(make_comic(2)).await.unwrap();
    db.insert_chunk(make_chunk(2, 0)).await.unwrap();
    let avg = db.avg_chunks_per_comic().await.unwrap();
    assert_eq!(avg, 2.0);
  }

  #[tokio::test]
  async fn test_avg_chunks_empty() {
    let db = setup().await;
    assert_eq!(db.avg_chunks_per_comic().await.unwrap(), 0.0);
  }

  #[tokio::test]
  async fn test_count_chunks_for_comic() {
    let db = setup().await;
    db.insert_comic(make_comic(5)).await.unwrap();
    for i in 0..3 {
      db.insert_chunk(make_chunk(5, i)).await.unwrap();
    }
    assert_eq!(db.count_chunks_for_comic(5).await.unwrap(), 3);
    assert_eq!(db.count_chunks_for_comic(999).await.unwrap(), 0);
  }

  #[tokio::test]
  async fn test_embedding_roundtrip() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut chunk = make_chunk(1, 0);
    chunk.embedding = vec![0.123, 0.456, 0.789].into_iter().cycle().take(EMBEDDING_DIM).collect();
    db.insert_chunk(chunk.clone()).await.unwrap();
    let retrieved = db.get_chunks_for_comic(1).await.unwrap();
    for (o, r) in chunk.embedding.iter().zip(retrieved[0].embedding.iter()) {
      assert!((o - r).abs() < 0.0001);
    }
  }

  #[tokio::test]
  async fn test_section_type_roundtrip() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut chunk = make_chunk(1, 0);
    chunk.section_type = Some(SectionType::Trivia);
    db.insert_chunk(chunk).await.unwrap();
    let retrieved = db.get_chunks_for_comic(1).await.unwrap();
    assert!(matches!(retrieved[0].section_type, Some(SectionType::Trivia)));
  }

  #[tokio::test]
  async fn test_cascade_delete() {
    let db = setup().await;
    db.insert_comic(make_comic(10)).await.unwrap();
    db.insert_chunk(make_chunk(10, 0)).await.unwrap();
    db.insert_chunk(make_chunk(10, 1)).await.unwrap();
    assert_eq!(db.count_chunks_for_comic(10).await.unwrap(), 2);
    db.delete_comic(10).await.unwrap();
    assert_eq!(db.count_chunks_for_comic(10).await.unwrap(), 0);
  }
}
