use anyhow::Result;
use crate::domain::entities::media::FilmMetadata;

/// On-disk cache mapping disc volume fingerprints to verified FilmMetadata records.
#[derive(Debug, Default, Clone)]
pub struct DiskFingerprintCache;

impl DiskFingerprintCache {
    pub fn new() -> Self {
        Self
    }

    pub fn lookup(&self, fingerprint: &str) -> Option<FilmMetadata> {
        crate::imdb::lookup_fingerprint_cache(fingerprint).map(|m| FilmMetadata {
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
        })
    }

    pub fn save(&self, fingerprint: &str, metadata: &FilmMetadata) -> Result<()> {
        let legacy_meta = crate::imdb::FilmMetadata {
            title: metadata.title.clone(),
            year: metadata.year,
            runtime_secs: metadata.runtime_secs,
            poster_url: metadata.poster_url.clone(),
            plot: metadata.plot.clone(),
            genre: metadata.genre.clone(),
            director: metadata.director.clone(),
            actors: metadata.actors.clone(),
            rating: metadata.rating.clone(),
            is_series: metadata.is_series,
            total_seasons: metadata.total_seasons,
            poster_bytes: metadata.poster_bytes.clone(),
        };
        crate::imdb::save_fingerprint_cache(fingerprint, &legacy_meta);
        Ok(())
    }
}
