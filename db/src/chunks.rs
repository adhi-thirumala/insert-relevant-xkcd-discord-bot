use crate::error::{DatabaseError, Result};
use crate::models::SectionType;
use crate::{Chunks, Database, EMBEDDING_DIM};
use libsql::params;
use serde::{Deserialize, Serialize};
use serde_json::to_string;

/// Result of a vector similarity search operation.
///
/// Contains the chunk data along with metadata from the associated comic.
/// Results are ordered by similarity (most similar first) via the vector index.
/// Returned by vector search operations such as [`Database::vector_search`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSearchResult {
  /// The unique identifier of the chunk.
  pub chunk_id: u64,
  /// The XKCD comic number associated with this chunk.
  pub comic_number: u64,
  /// The text content of the chunk.
  pub chunk_text: String,
  /// The section type of the chunk (e.g., "transcript", "explanation", etc.), if available.
  pub section_type: Option<String>,
  /// The title of the associated XKCD comic.
  pub comic_title: String,
  /// The URL to the XKCD comic.
  pub xkcd_url: String,
  /// The hover text (alt text) of the comic, if available.
  pub hover_text: Option<String>,
}

// Helper functions
fn validate_embedding(embedding: &[f32]) -> Result<()> {
  if embedding.len() != EMBEDDING_DIM {
    return Err(DatabaseError::InvalidEmbeddingDimension(format!(
      "Expected {} dimensions, got {}",
      EMBEDDING_DIM,
      embedding.len()
    )));
  }
  Ok(())
}

pub(crate) fn vec_to_json_string(embedding: Vec<impl Serialize>) -> String {
  to_string(&embedding).expect("Failed to serialize embedding (should not fail)")
}

fn f32_blob_to_vec(blob: &[u8]) -> Vec<f32> {
  blob
    .chunks_exact(4)
    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
    .collect()
}

impl Database {
  /// Insert a single chunk into the database.
  ///
  /// # Errors
  /// Returns an error if:
  /// - The embedding dimension doesn't match EMBEDDING_DIM (768)
  /// - The comic_number doesn't exist (foreign key constraint)
  /// - The database operation fails
  pub async fn insert_chunk(&self, chunk: Chunks) -> Result<u64> {
    validate_embedding(&chunk.embedding)?;

    let stmt = self
      .conn
      .prepare(
        "INSERT INTO xkcd_chunks (
           comic_number,
           chunk_text,
           chunk_index,
           section_type,
           embedding
          ) VALUES (
          ?,
          ?,
          ?,
          ?,
          vector32(?)
          )",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    stmt
      .execute(params![
        // no comic id - its autoincrement on add
        chunk.comic_number,
        chunk.chunk_text,
        chunk.chunk_index,
        chunk.section_type.map(|s| s.to_string()),
        vec_to_json_string(chunk.embedding),
      ])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    Ok(self.conn.last_insert_rowid() as u64)
  }

  /// Insert multiple chunks into the database in a batch.
  ///
  /// All chunks are validated before insertion. If any chunk fails validation,
  /// none of the chunks will be inserted (atomic operation).
  ///
  /// # Errors
  /// Returns an error if:
  /// - Any chunk's embedding dimension doesn't match EMBEDDING_DIM (768)
  /// - Any chunk's comic_number doesn't exist (foreign key constraint)
  /// - The database operation fails
  pub async fn insert_chunks_batch(&self, chunks: Vec<Chunks>) -> Result<()> {
    for chunk in &chunks {
      validate_embedding(&chunk.embedding)?;
    }
    let tx = self
      .conn
      .transaction()
      .await
      .map_err(|e| DatabaseError::TransactionFailed(e.to_string()))?;

    let stmt = tx
      .prepare(
        "INSERT INTO xkcd_chunks (
       comic_number,
       chunk_text,
       chunk_index,
       section_type,
       embedding
      ) VALUES (
      ?,
      ?,
      ?,
      ?,
      vector32(?)
      )",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    for chunk in chunks {
      stmt
        .execute(params![
          chunk.comic_number,
          chunk.chunk_text,
          chunk.chunk_index,
          chunk.section_type.map(|s| s.to_string()),
          vec_to_json_string(chunk.embedding),
        ])
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

      stmt.reset();
    }

    tx.commit()
      .await
      .map_err(|e| DatabaseError::TransactionFailed(e.to_string()))?;
    Ok(())
  }

  pub async fn get_chunks_for_comic(&self, comic_number: u64) -> Result<Vec<Chunks>> {
    let stmt = self
      .conn
      .prepare(
        "SELECT id, comic_number, chunk_text, chunk_index, section_type, embedding
         FROM xkcd_chunks
         WHERE comic_number = ?
         ORDER BY chunk_index ASC",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    let mut rows = stmt
      .query(params![comic_number])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    let mut chunks = Vec::new();
    while let Some(row) = rows
      .next()
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?
    {
      let id: u64 = row
        .get(0)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let comic_number: u64 = row
        .get(1)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let chunk_text: String = row
        .get(2)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let chunk_index: u64 = row
        .get(3)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let section_type_str: Option<String> = row
        .get(4)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let embedding_blob: Vec<u8> = row
        .get(5)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;

      let section_type = section_type_str
        .map(|s| s.parse::<SectionType>())
        .transpose()
        .map_err(|e| DatabaseError::Serialization(format!("Invalid section_type: {}", e)))?;

      let embedding = f32_blob_to_vec(&embedding_blob);

      chunks.push(Chunks {
        id: Some(id),
        comic_number,
        chunk_text,
        chunk_index,
        section_type,
        embedding,
      });
    }

    Ok(chunks)
  }

  /// Delete all chunks associated with a comic.
  ///
  /// Returns the number of chunks that were deleted. Returns 0 if the comic
  /// has no chunks or doesn't exist.
  pub async fn delete_chunks_for_comic(&self, comic_number: u64) -> Result<u64> {
    let stmt = self
      .conn
      .prepare("DELETE FROM xkcd_chunks WHERE comic_number = ?")
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;

    let rows_affected = stmt
      .execute(params![comic_number])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    Ok(rows_affected as u64)
  }

  pub async fn vector_search(
    &self,
    query_embedding: Vec<f32>,
    top_k: usize,
  ) -> Result<Vec<ChunkSearchResult>> {
    validate_embedding(&query_embedding)?;

    let query_vec_json = vec_to_json_string(query_embedding);
    let stmt = self
      .conn
      .prepare(
        "SELECT
          xc.id,
          xc.comic_number,
          xc.chunk_text,
          xc.section_type,
          c.title,
          c.xkcd_url,
          c.hover_text
        FROM vector_top_k('chunks_vec_idx', vector32(?), ?) v
        JOIN xkcd_chunks xc ON xc.rowid = v.id
        JOIN xkcd_comics c ON c.comic_number = xc.comic_number",
      )
      .await
      .map_err(|e| DatabaseError::PreparedFailed(e.to_string()))?;
    let mut rows = stmt
      .query(params![query_vec_json, top_k as u64])
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    let mut results = Vec::new();
    while let Some(row) = rows
      .next()
      .await
      .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?
    {
      let chunk_id: u64 = row
        .get(0)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let comic_number: u64 = row
        .get(1)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let chunk_text: String = row
        .get(2)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let section_type: Option<String> = row
        .get(3)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let comic_title: String = row
        .get(4)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let xkcd_url: String = row
        .get(5)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;
      let hover_text: Option<String> = row
        .get(6)
        .map_err(|e| DatabaseError::Serialization(e.to_string()))?;

      results.push(ChunkSearchResult {
        chunk_id,
        comic_number,
        chunk_text,
        section_type,
        comic_title,
        xkcd_url,
        hover_text,
      });
    }

    Ok(results)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::EMBEDDING_DIM;
  use crate::models::{Chunks, Comics, SectionType};

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
      id: Some(0),
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
    assert!(db.insert_chunks_batch(chunks).await.is_ok());
    assert_eq!(db.get_chunks_for_comic(1).await.unwrap().len(), 2);
  }

  #[tokio::test]
  async fn test_insert_chunks_batch_validates_all_embeddings() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut bad = make_chunk(1, 1);
    bad.embedding = vec![0.0; 50];
    let chunks = vec![make_chunk(1, 0), bad, make_chunk(1, 2)];
    assert!(db.insert_chunks_batch(chunks).await.is_err());
    assert_eq!(db.get_chunks_for_comic(1).await.unwrap().len(), 0);
  }

  #[tokio::test]
  async fn test_insert_chunks_batch_rollback_on_foreign_key_violation() {
    let db = setup().await;

    // Insert one comic
    db.insert_comic(make_comic(1)).await.unwrap();

    // Create batch with valid and invalid chunks
    let chunks = vec![
      make_chunk(1, 0),   // Valid - comic 1 exists
      make_chunk(999, 0), // Invalid - comic 999 doesn't exist
    ];

    // Batch insert should fail due to foreign key violation
    let result = db.insert_chunks_batch(chunks).await;
    assert!(result.is_err());

    // Verify rollback: comic 1 should have ZERO chunks
    // (the first chunk should have been rolled back)
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
    let results = db.vector_search(query, 2).await.unwrap();
    assert_eq!(results.len(), 2);
    // vector_top_k returns top K results, ordering depends on index implementation
    let comic_numbers: Vec<u64> = results.iter().map(|r| r.comic_number).collect();
    assert!(comic_numbers.contains(&1));
    assert!(comic_numbers.contains(&2));
  }

  #[tokio::test]
  async fn test_vector_search_invalid_embedding_dimension() {
    let db = setup().await;
    let query = vec![0.5; 100];
    assert!(db.vector_search(query, 10).await.is_err());
  }
  #[tokio::test]
  async fn test_embedding_roundtrip() {
    let db = setup().await;
    db.insert_comic(make_comic(1)).await.unwrap();
    let mut chunk = make_chunk(1, 0);
    chunk.embedding = vec![0.123, 0.456, 0.789]
      .into_iter()
      .cycle()
      .take(EMBEDDING_DIM)
      .collect();
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
    assert!(matches!(
      retrieved[0].section_type,
      Some(SectionType::Trivia)
    ));
  }
}
