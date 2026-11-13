//! Disc Rosetta Stone Registry Port.
//!
//! Provides deterministic disc fingerprinting and offline canonical title resolution,
//! mapping physical optical volume signatures directly to known movie/TV metadata,
//! authentic feature title indices (bypassing decoy traps), and episode chapter mappings.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Detailed episode mapping record for TV box set discs within the Rosetta Stone registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosettaEpisodeEntry {
    /// 1-based DVD title number containing this episode.
    pub title_number: u32,
    /// Chapter start index within the title (or None if title spans full episode).
    pub start_chapter: Option<u32>,
    /// Chapter end index within the title (or None if title spans full episode).
    pub end_chapter: Option<u32>,
    /// Season number (e.g. 1).
    pub season: u32,
    /// Episode index within season (e.g. 4).
    pub episode: u32,
    /// Canonical episode title (e.g. "Walkabout").
    pub episode_title: String,
}

/// Canonical metadata and optical structure definition for a recognized physical DVD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RosettaDiscEntry {
    /// Deterministic 64/128-bit disc fingerprint hash string.
    pub fingerprint: String,
    /// Canonical movie or series release title.
    pub canonical_title: String,
    /// Theatrical or broadcast release year.
    pub release_year: Option<u32>,
    /// Canonical IMDb ID (e.g. "tt0111161").
    pub imdb_id: Option<String>,
    /// Canonical TMDB ID (e.g. 278).
    pub tmdb_id: Option<u32>,
    /// Whether this media corresponds to a television series or anthology.
    pub is_tv_series: bool,
    /// Box set season number, if applicable.
    pub season: Option<u32>,
    /// Box set physical disc index within season, if applicable.
    pub disc_number: Option<u32>,
    /// Guaranteed authentic main feature title index (bypassing multi-hundred decoy playlists).
    pub main_feature_title: u32,
    /// Pre-mapped episode breakdown for TV series discs.
    pub episodes: Vec<RosettaEpisodeEntry>,
    /// DVD subtitle track indices containing foreign-language forced dialogue translations.
    pub forced_subtitle_tracks: Vec<u32>,
    /// Special notes, director cut details, or mastering notes.
    pub notes: Option<String>,
}

/// Domain contract for querying and contributing to the Disc Rosetta Stone registry.
pub trait RosettaStoneRegistry: Send + Sync {
    /// Looks up an optical disc's canonical profile by its deterministic fingerprint.
    fn lookup(&self, fingerprint: &str) -> Option<RosettaDiscEntry>;

    /// Registers or updates a canonical disc entry in the local persistent store.
    fn register(&self, entry: RosettaDiscEntry) -> Result<()>;

    /// Returns the total count of recognized disc profiles in this registry instance.
    fn entry_count(&self) -> usize;
}

