use crate::domain::entities::media::{FilmMetadata, SearchCandidate};
use crate::domain::ports::metadata::MetadataProvider;
use anyhow::Result;

/// Concrete OMDb REST API metadata provider implementing the domain MetadataProvider port.
#[derive(Debug, Default, Clone)]
pub struct OmdbMetadataProvider {
    pub custom_api_key: Option<String>,
}

impl OmdbMetadataProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            custom_api_key: api_key,
        }
    }
}

impl MetadataProvider for OmdbMetadataProvider {
    fn search(&self, query: &str) -> Result<Vec<SearchCandidate>> {
        let candidates = crate::imdb::fetch_search_candidates(query);
        Ok(candidates
            .into_iter()
            .map(|c| SearchCandidate {
                imdb_id: c.imdb_id,
                title: c.title,
                year: c.year,
                type_field: c.type_field,
                poster_url: c.poster_url,
            })
            .collect())
    }

    fn get_details_by_id(&self, id: &str) -> Result<Option<FilmMetadata>> {
        let res = crate::imdb::lookup_omdb_by_id(id).map(|m| FilmMetadata {
            title: m.title,
            year: m.year,
            runtime_secs: m.runtime_secs,
            poster_url: m.poster_url,
            plot: m.plot,
            genre: m.genre,
            director: m.director,
            actors: m.actors,
            rating: m.rating,
            is_series: m.is_series,
            total_seasons: m.total_seasons,
            poster_bytes: m.poster_bytes,
        });
        Ok(res)
    }

    fn get_details_by_title_and_year(
        &self,
        title: &str,
        year: Option<u32>,
    ) -> Result<Option<FilmMetadata>> {
        let q = if let Some(y) = year {
            format!("{} {}", title, y)
        } else {
            title.to_string()
        };
        match crate::imdb::lookup_film_details(&q) {
            Ok(m) => Ok(Some(FilmMetadata {
                title: m.title,
                year: m.year,
                runtime_secs: m.runtime_secs,
                poster_url: m.poster_url,
                plot: m.plot,
                genre: m.genre,
                director: m.director,
                actors: m.actors,
                rating: m.rating,
                is_series: m.is_series,
                total_seasons: m.total_seasons,
                poster_bytes: m.poster_bytes,
            })),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_omdb_metadata_provider_lifecycle() {
        let provider = OmdbMetadataProvider::new(Some("custom_key".to_string()));
        assert_eq!(provider.custom_api_key.as_deref(), Some("custom_key"));

        let def = OmdbMetadataProvider::default();
        assert!(def.custom_api_key.is_none());

        let cloned = provider.clone();
        assert_eq!(format!("{:?}", provider), format!("{:?}", cloned));
    }

    #[test]
    fn test_omdb_metadata_provider_methods() {
        let provider = OmdbMetadataProvider::new(None);

        // Search returns candidate vector
        let _ = provider.search("Aliens");

        // Lookup by id
        let _ = provider.get_details_by_id("tt0090605");

        // Lookup by title and year
        let _ = provider.get_details_by_title_and_year("Aliens", Some(1986));
        let _ = provider.get_details_by_title_and_year("NonExistentXYZ123456789", None);
    }
}
