use thiserror::Error;

pub type Result<T> = std::result::Result<T, DatabaseError>;

#[derive(Debug, Error)]
pub enum DatabaseError {
  // ========================================================================
  // Connection & Initialization Errors
  // ========================================================================
  //
  #[error("Initialization error: {0}")]
  InitializationError(String),

  #[error("Connection error: {0}")]
  Connection(String),

  /// Database schema not initialized (tables don't exist)
  #[error("Database not initialized")]
  NotInitialized,

  // ========================================================================
  // Data Validation Errors
  // ========================================================================
  /// Invalid comic number (must be positive)
  #[error("Invalid comic number: {0}")]
  InvalidComicNumber(u64),

  /// Invalid embedding dimension
  #[error("Invalid embedding dimension: {0}")]
  InvalidEmbeddingDimension(String),

  /// Invalid chunk index (must be non-negative)
  #[error("Invalid chunk index: {0}")]
  InvalidChunkIndex(u64),

  /// Empty or invalid content
  #[error("Invalid content: {0}")]
  InvalidContent(String),

  // ========================================================================
  // Not Found Errors
  // ========================================================================
  /// Comic not found in database
  #[error("Comic not found: {0}")]
  ComicNotFound(u64),

  /// Chunk not found in database
  #[error("Chunk not found: {0}")]
  ChunkNotFound(u64),

  // ========================================================================
  // Conflict Errors
  // ========================================================================
  /// Comic already exists (duplicate primary key)
  #[error("Comic already exists: {0}")]
  ComicAlreadyExists(u64),

  /// Constraint violation (e.g., foreign key)
  #[error("Constraint violation: {0}")]
  ConstraintViolation(String),

  // ========================================================================
  // Query Errors
  // ========================================================================
  /// SQL query execution failed
  #[error("Prepared statement failed: {0}")]
  PreparedFailed(String),

  #[error("Query failed: {0}")]
  QueryFailed(String),

  /// Failed to parse row data
  #[error("Failed to parse row data: {0}")]
  RowParseFailed(String),

  /// Transaction failed
  #[error("Transaction failed: {0}")]
  TransactionFailed(String),

  /// Vector search failed
  #[error("Vector search failed: {0}")]
  VectorSearchFailed(String),

  #[error("Metadata not found for key {0}")]
  MetadataNotFound(String),

  #[error("Failed to parse metadata value: {0}")]
  MetaParseFailed(String),

  // ========================================================================
  // Serialization Errors
  // ========================================================================
  /// Failed to serialize/deserialize data
  #[error("Failed to serialize/deserialize data: {0}")]
  Serialization(String),

  /// Failed to convert section type
  #[error("Failed to convert section type: {0}")]
  InvalidSectionType(String),

  // ========================================================================
  // I/O Errors
  // ========================================================================
  /// File system error (can't open database file)
  #[error("File system error: {0}")]
  IoError(#[from] std::io::Error),

  // ========================================================================
  // Underlying Library Errors
  // ========================================================================
  /// libSQL-specific error
  #[error("libSQL error: {0}")]
  LibSql(#[from] libsql::Error),
}
