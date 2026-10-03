use anyhow::Result;
use crate::domain::entities::media::{FilmMetadata, SearchCandidate};

/// Abstraction for online and offline film/TV metadata lookups.
pub trait MetadataProvider: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<SearchCandidate>>;
    fn get_details_by_id(&self, id: &str) -> Result<Option<FilmMetadata>>;
    fn get_details_by_title_and_year(&self, title: &str, year: Option<u32>) -> Result<Option<FilmMetadata>>;
}
