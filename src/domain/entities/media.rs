use serde::{Deserialize, Serialize};

/// Type of media represented on disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    TvSeries,
}

/// Unified Film / TV metadata entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FilmMetadata {
    pub title: String,
    pub year: Option<u32>,
    pub runtime_secs: Option<f64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub poster_bytes: Option<Vec<u8>>,
    pub is_series: bool,
    pub genre: Option<String>,
    pub director: Option<String>,
    pub actors: Option<String>,
    pub rating: Option<String>,
    pub total_seasons: Option<u32>,
}

/// Lightweight candidate result from online search queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchCandidate {
    pub title: String,
    pub year: Option<u32>,
    pub imdb_id: String,
    pub type_field: String,
    pub poster_url: Option<String>,
}

impl SearchCandidate {
    pub fn formatted_label(&self) -> String {
        let yr = self.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let type_desc = if !self.type_field.is_empty() {
            format!(" [{}]", self.type_field)
        } else {
            String::new()
        };
        format!("{}{}{}", self.title, yr, type_desc)
    }
}
