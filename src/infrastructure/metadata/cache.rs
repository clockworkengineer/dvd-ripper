use crate::domain::entities::media::FilmMetadata;
use anyhow::Result;
use serde::{Deserialize, Serialize};
/**
 * @file cache.rs
 * @brief On-disk disc fingerprint and media metadata caching for instant recognition.
 */
use std::path::PathBuf;

/// On-disk cache entry mapping a disc volume fingerprint to verified media metadata and ripping settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedDiscEntry {
    pub fingerprint: String,
    pub volume_label: Option<String>,
    pub metadata: FilmMetadata,
    pub title_number: Option<u32>,
    pub is_tv: bool,
    pub season: Option<u32>,
    pub cached_at: String,
}

impl CachedDiscEntry {
    pub fn to_imdb_metadata(&self) -> crate::imdb::FilmMetadata {
        crate::imdb::FilmMetadata {
            title: self.metadata.title.clone(),
            year: self.metadata.year,
            runtime_secs: self.metadata.runtime_secs,
            poster_url: self.metadata.poster_url.clone(),
            plot: self.metadata.plot.clone(),
            genre: self.metadata.genre.clone(),
            director: self.metadata.director.clone(),
            actors: self.metadata.actors.clone(),
            rating: self.metadata.rating.clone(),
            is_series: self.is_tv,
            total_seasons: self.metadata.total_seasons,
            poster_bytes: self.metadata.poster_bytes.clone(),
        }
    }
}

/// On-disk cache mapping disc volume fingerprints to verified FilmMetadata records and settings.
#[derive(Debug, Default, Clone)]
pub struct DiskFingerprintCache;

impl DiskFingerprintCache {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the absolute path to the persistent disc cache file.
    pub fn cache_file_path() -> PathBuf {
        let path = crate::utils::get_app_file_path("disc_cache.json");
        let _ = crate::utils::ensure_parent_dir(&path);
        path
    }

    /// Looks up verified FilmMetadata for a given disc fingerprint.
    pub fn lookup(&self, fingerprint: &str) -> Option<FilmMetadata> {
        self.lookup_entry(fingerprint).map(|e| e.metadata)
    }

    /// Looks up full CachedDiscEntry including title number and TV series settings.
    pub fn lookup_entry(&self, fingerprint: &str) -> Option<CachedDiscEntry> {
        let path = Self::cache_file_path();
        if let Some(map) = crate::utils::load_json_file::<
            std::collections::HashMap<String, CachedDiscEntry>,
        >(&path)
            && let Some(entry) = map.get(fingerprint)
        {
            return Some(entry.clone());
        }

        // Fallback to legacy fingerprints.json if present
        if let Some(m) = crate::imdb::lookup_fingerprint_cache(fingerprint) {
            return Some(CachedDiscEntry {
                fingerprint: fingerprint.to_string(),
                volume_label: None,
                metadata: FilmMetadata {
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
                },
                title_number: None,
                is_tv: m.is_series,
                season: None,
                cached_at: chrono::Local::now().to_rfc3339(),
            });
        }

        None
    }

    /// Saves or updates film metadata for the specified disc fingerprint.
    pub fn save(&self, fingerprint: &str, metadata: &FilmMetadata) -> Result<()> {
        let entry = CachedDiscEntry {
            fingerprint: fingerprint.to_string(),
            volume_label: None,
            metadata: metadata.clone(),
            title_number: None,
            is_tv: metadata.is_series,
            season: None,
            cached_at: chrono::Local::now().to_rfc3339(),
        };
        self.save_entry(&entry)
    }

    /// Saves full CachedDiscEntry to persistent cache file.
    pub fn save_entry(&self, entry: &CachedDiscEntry) -> Result<()> {
        let path = Self::cache_file_path();
        let mut map: std::collections::HashMap<String, CachedDiscEntry> =
            crate::utils::load_json_file(&path).unwrap_or_default();
        map.insert(entry.fingerprint.clone(), entry.clone());
        crate::utils::save_json_file(&path, &map)?;

        // Also mirror to legacy cache so existing callers of lookup_fingerprint_cache work
        let legacy_meta = crate::imdb::FilmMetadata {
            title: entry.metadata.title.clone(),
            year: entry.metadata.year,
            runtime_secs: entry.metadata.runtime_secs,
            poster_url: entry.metadata.poster_url.clone(),
            plot: entry.metadata.plot.clone(),
            genre: entry.metadata.genre.clone(),
            director: entry.metadata.director.clone(),
            actors: entry.metadata.actors.clone(),
            rating: entry.metadata.rating.clone(),
            is_series: entry.is_tv,
            total_seasons: entry.metadata.total_seasons,
            poster_bytes: entry.metadata.poster_bytes.clone(),
        };
        crate::imdb::save_fingerprint_cache(&entry.fingerprint, &legacy_meta);

        Ok(())
    }

    /// Lists all cached disc entries sorted by most recently updated first.
    pub fn list_all_entries(&self) -> Vec<CachedDiscEntry> {
        let path = Self::cache_file_path();
        let map: std::collections::HashMap<String, CachedDiscEntry> =
            crate::utils::load_json_file(&path).unwrap_or_default();
        let mut entries: Vec<_> = map.into_values().collect();
        entries.sort_by(|a, b| b.cached_at.cmp(&a.cached_at));
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disc_fingerprint_cache_roundtrip() {
        let cache = DiskFingerprintCache::new();
        let dummy_fp = "disc_test_fingerprint_12345";
        let entry = CachedDiscEntry {
            fingerprint: dummy_fp.to_string(),
            volume_label: Some("ALIENS_SE".to_string()),
            metadata: FilmMetadata {
                title: "Aliens".to_string(),
                year: Some(1986),
                runtime_secs: Some(8220.0),
                genre: Some("Action, Sci-Fi".to_string()),
                ..Default::default()
            },
            title_number: Some(1),
            is_tv: false,
            season: None,
            cached_at: chrono::Local::now().to_rfc3339(),
        };

        assert!(cache.save_entry(&entry).is_ok());

        let loaded = cache.lookup_entry(dummy_fp);
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.metadata.title, "Aliens");
        assert_eq!(loaded.metadata.year, Some(1986));
        assert_eq!(loaded.title_number, Some(1));
        assert_eq!(loaded.volume_label.as_deref(), Some("ALIENS_SE"));
    }
}
