use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunks {
  pub id: u64,
  pub comic_number: u64,
  pub chunk_text: String,
  pub chunk_index: u64,
  pub section_type: Option<SectionType>,
  pub embedding: Vec<f32>,
}
