//! Local and Embedded Disc Rosetta Stone Registry Adapter.
//!
//! Provides fast offline canonical metadata resolution for recognized DVDs,
//! including pre-identified main feature titles (bypassing decoy traps)
//! and television episode breakdowns.

use crate::domain::ports::rosetta::{RosettaDiscEntry, RosettaEpisodeEntry, RosettaStoneRegistry};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Computes a deterministic 128-bit hexadecimal fingerprint from primary volume
/// descriptor bytes and IFO file headers.
#[must_use]
pub fn compute_optical_rosetta_hash(pvd_data: &[u8], ifo_header: &[u8]) -> String {
    // 128-bit FNV-1a hash implementation
    let mut h0: u64 = 0x6a09e667f3bcc908;
    let mut h1: u64 = 0xbb67ae8584caa73b;

    for (i, &byte) in pvd_data.iter().chain(ifo_header.iter()).enumerate() {
        if i % 2 == 0 {
            h0 ^= byte as u64;
            h0 = h0.wrapping_mul(0x100000001b3);
        } else {
            h1 ^= byte as u64;
            h1 = h1.wrapping_mul(0x100000001b3);
        }
    }

    format!("{:016x}{:016x}", h0, h1)
}

/// Persistent and embedded implementation of [`RosettaStoneRegistry`].
pub struct LocalRosettaRegistry {
    storage_path: Option<PathBuf>,
    entries: Arc<RwLock<HashMap<String, RosettaDiscEntry>>>,
}

impl LocalRosettaRegistry {
    /// Creates a new in-memory registry seeded with known reference titles.
    #[must_use]
    pub fn new_in_memory() -> Self {
        let registry = Self {
            storage_path: None,
            entries: Arc::new(RwLock::new(HashMap::new())),
        };
        registry.seed_known_reference_discs();
        registry
    }

    /// Creates a registry persisting custom and learned disc profiles to a JSON file.
    #[must_use]
    pub fn with_storage(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let registry = Self {
            storage_path: Some(path.clone()),
            entries: Arc::new(RwLock::new(HashMap::new())),
        };
        registry.seed_known_reference_discs();
        registry.load_from_disk(&path);
        registry
    }

    fn seed_known_reference_discs(&self) {
        let mut map = self.entries.write().unwrap();

        // Reference Seed 1: The Matrix (1999) - Iconic single-feature DVD
        map.insert(
            "matr1x1999dvd0001".to_string(),
            RosettaDiscEntry {
                fingerprint: "matr1x1999dvd0001".to_string(),
                canonical_title: "The Matrix".to_string(),
                release_year: Some(1999),
                imdb_id: Some("tt0133093".to_string()),
                tmdb_id: Some(603),
                is_tv_series: false,
                season: None,
                disc_number: None,
                main_feature_title: 1,
                episodes: Vec::new(),
                forced_subtitle_tracks: vec![1],
                notes: Some("Master reference: Wachowskis original master".to_string()),
            },
        );

        // Reference Seed 2: Pulp Fiction (1994)
        map.insert(
            "pulpf1ct1994dvd01".to_string(),
            RosettaDiscEntry {
                fingerprint: "pulpf1ct1994dvd01".to_string(),
                canonical_title: "Pulp Fiction".to_string(),
                release_year: Some(1994),
                imdb_id: Some("tt0110912".to_string()),
                tmdb_id: Some(680),
                is_tv_series: false,
                season: None,
                disc_number: None,
                main_feature_title: 1,
                episodes: Vec::new(),
                forced_subtitle_tracks: Vec::new(),
                notes: Some("Quentin Tarantino Palme d'Or master".to_string()),
            },
        );

        // Reference Seed 3: Firefly Disc 1 (TV Box Set)
        map.insert(
            "f1refly2002d1dvd0".to_string(),
            RosettaDiscEntry {
                fingerprint: "f1refly2002d1dvd0".to_string(),
                canonical_title: "Firefly".to_string(),
                release_year: Some(2002),
                imdb_id: Some("tt0303461".to_string()),
                tmdb_id: Some(1437),
                is_tv_series: true,
                season: Some(1),
                disc_number: Some(1),
                main_feature_title: 1,
                episodes: vec![
                    RosettaEpisodeEntry {
                        title_number: 1,
                        start_chapter: None,
                        end_chapter: None,
                        season: 1,
                        episode: 1,
                        episode_title: "Serenity".to_string(),
                    },
                    RosettaEpisodeEntry {
                        title_number: 2,
                        start_chapter: None,
                        end_chapter: None,
                        season: 1,
                        episode: 2,
                        episode_title: "The Train Job".to_string(),
                    },
                    RosettaEpisodeEntry {
                        title_number: 3,
                        start_chapter: None,
                        end_chapter: None,
                        season: 1,
                        episode: 3,
                        episode_title: "Bushwhacked".to_string(),
                    },
                ],
                forced_subtitle_tracks: Vec::new(),
                notes: Some("Episodes in broadcast/intended order".to_string()),
            },
        );
    }

    fn load_from_disk(&self, path: &Path) {
        if path.exists()
            && let Ok(data) = std::fs::read_to_string(path)
            && let Ok(saved) = serde_json::from_str::<HashMap<String, RosettaDiscEntry>>(&data)
        {
            let mut map = self.entries.write().unwrap();
            for (k, v) in saved {
                map.insert(k, v);
            }
        }
    }

    fn save_to_disk(&self) -> Result<()> {
        if let Some(ref path) = self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let map = self.entries.read().unwrap();
            let json = serde_json::to_string_pretty(&*map)?;
            std::fs::write(path, json)?;
        }
        Ok(())
    }
}

impl RosettaStoneRegistry for LocalRosettaRegistry {
    fn lookup(&self, fingerprint: &str) -> Option<RosettaDiscEntry> {
        let map = self.entries.read().unwrap();
        map.get(fingerprint).cloned()
    }

    fn register(&self, entry: RosettaDiscEntry) -> Result<()> {
        {
            let mut map = self.entries.write().unwrap();
            map.insert(entry.fingerprint.clone(), entry);
        }
        self.save_to_disk()
    }

    fn entry_count(&self) -> usize {
        let map = self.entries.read().unwrap();
        map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_optical_rosetta_hash() {
        let pvd = b"CD001\x01\x00THE_MATRIX_DVD_1999";
        let ifo = b"DVDVIDEO-VMG\x00\x01\x02\x03";
        let hash = compute_optical_rosetta_hash(pvd, ifo);
        assert_eq!(hash.len(), 32);

        // Deterministic check
        let hash2 = compute_optical_rosetta_hash(pvd, ifo);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_in_memory_seeded_lookup() {
        let registry = LocalRosettaRegistry::new_in_memory();
        assert!(registry.entry_count() >= 3);

        let matrix = registry.lookup("matr1x1999dvd0001").expect("Should find Matrix");
        assert_eq!(matrix.canonical_title, "The Matrix");
        assert_eq!(matrix.release_year, Some(1999));
        assert_eq!(matrix.main_feature_title, 1);
        assert_eq!(matrix.imdb_id.as_deref(), Some("tt0133093"));

        let firefly = registry.lookup("f1refly2002d1dvd0").expect("Should find Firefly");
        assert!(firefly.is_tv_series);
        assert_eq!(firefly.episodes.len(), 3);
        assert_eq!(firefly.episodes[0].episode_title, "Serenity");
    }

    #[test]
    fn test_register_and_persist_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_rosetta_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("rosetta.json");

        let custom_entry = RosettaDiscEntry {
            fingerprint: "custom_hash_12345".to_string(),
            canonical_title: "Interstellar".to_string(),
            release_year: Some(2014),
            imdb_id: Some("tt0816692".to_string()),
            tmdb_id: Some(157336),
            is_tv_series: false,
            season: None,
            disc_number: None,
            main_feature_title: 1,
            episodes: Vec::new(),
            forced_subtitle_tracks: Vec::new(),
            notes: Some("IMAX 70mm cut".to_string()),
        };

        {
            let registry = LocalRosettaRegistry::with_storage(&file_path);
            registry.register(custom_entry.clone()).unwrap();
            assert_eq!(
                registry.lookup("custom_hash_12345").unwrap().canonical_title,
                "Interstellar"
            );
        }

        // Reopen from disk to verify persistence
        {
            let registry = LocalRosettaRegistry::with_storage(&file_path);
            let restored = registry.lookup("custom_hash_12345").expect("Should reload");
            assert_eq!(restored.canonical_title, "Interstellar");
            assert_eq!(restored.release_year, Some(2014));
        }
    }
}
