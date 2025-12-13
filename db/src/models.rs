use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Represents a full comic that has been scraped.
///
/// This struct contains all the information about a comic, including its number, title, URL, and other metadata.
///
///
/// # Example
/// ```
/// use db::Comics;
/// let comics = Comics {
///    comic_number: 1,
///    title: "Title".to_string(),
///    url: "https://example.com".to_string(),
///    xkcd_url: "https://xkcd.com".to_string(),
///    hover_text: Some("Hover Text".to_string()),
///    last_revision_id: 1,
///    last_revision_timestamp: "2023-01-01T00:00:00Z".to_string(),
///    scraped_at: "2023-01-01T00:00:00Z".to_string(),
///    updated_at: "2023-01-01T00:00:00Z".to_string(),
///};
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comics {
  pub comic_number: u64,
  pub title: String,
  pub url: String,      //explainxkcd.com url
  pub xkcd_url: String, //xkcd.com url
  pub hover_text: Option<String>,
  pub last_revision_id: u64,
  pub last_revision_timestamp: String,
  pub scraped_at: String,
  pub updated_at: String,
}

/// Represents the type of section in a comic.
///
/// This enum defines the different types of sections that can be found in a comic. Supports direct conversion to and from string via the `Display` and `FromStr` traits.
///
#[derive(Debug, Clone, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SectionType {
  TitleHover,
  Explanation,
  Transcript,
  Trivia,
  Other,
}

/// Represents a chunk of text from a comic.
///
/// This struct contains all the information about a chunk of text, including its ID, comic number, text, index, section type, and embedding.
///
///
/// # Example
/// ```
/// use db::{Chunks, SectionType};
/// let chunks = Chunks {
///    id: Some(1),
///    comic_number: 1,
///    chunk_text: "Chunk Text".to_string(),
///    chunk_index: 1,
///    section_type: Some(SectionType::TitleHover),
///    embedding: vec![0.0, 0.0, 0.0],
///};
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunks {
  pub id: Option<u64>,
  pub comic_number: u64,
  pub chunk_text: String,
  pub chunk_index: u64,
  pub section_type: Option<SectionType>,
  pub embedding: Vec<f32>,
}

/// Represents metadata about the database.
///
/// This struct contains all the information about the database, including its ID, key, and value.
///
///
/// # Example
/// ```
/// use db::Metadata;
/// let metadata = Metadata {
///    key: "key".to_string(),
///    value: "value".to_string(),
///};
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
  pub key: String,
  pub value: String,
}
