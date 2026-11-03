//! Use case for probing optical disc layout, copy protection, titles, and metadata.

use crate::domain::entities::disc::{DiscInfo, TvEpisodeInfo};
use crate::domain::entities::media::FilmMetadata;
use crate::domain::heuristics::cluster_detector::rank_best_title_from_slice;
use crate::domain::heuristics::tv_detector::detect_tv_episodes_from_titles;
use crate::domain::ports::metadata::MetadataProvider;
use crate::domain::ports::optical_drive::OpticalDrive;
use anyhow::Result;
use std::sync::Arc;

/// Result of probing a disc, combining disc hardware details, title tracks, TV episodes, and metadata.
#[derive(Debug, Clone)]
pub struct DiscProbeResult {
    pub disc_info: DiscInfo,
    pub primary_title_num: u32,
    pub primary_duration_secs: Option<f64>,
    pub metadata: Option<FilmMetadata>,
    pub tv_episodes: Vec<TvEpisodeInfo>,
}

/// Use case that probes optical disc layout, analyzes copy protection, detects TV episodes, and fetches metadata.
pub struct ProbeDiscUseCase {
    metadata_provider: Option<Arc<dyn MetadataProvider>>,
}

impl ProbeDiscUseCase {
    /// Creates a new [`ProbeDiscUseCase`] with an optional online metadata provider.
    ///
    /// # Panics
    /// Never panics.
    pub fn new(metadata_provider: Option<Arc<dyn MetadataProvider>>) -> Self {
        Self { metadata_provider }
    }

    /// Probes the specified optical drive device, extracting title hierarchy, copy protection,
    /// and correlating online film or TV episode metadata.
    ///
    /// # Parameters
    /// - `drive`: Abstract optical drive port representing the physical or virtual drive.
    ///
    /// # Errors
    /// Returns an error if optical drive querying fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn execute(&self, drive: &dyn OpticalDrive) -> Result<DiscProbeResult> {
        let drive_path = drive.get_path();
        let volume_label = drive.read_volume_label();
        let protection = drive.inspect_protection();
        let titles = drive.probe_titles();

        let fingerprint = crate::dvd::compute_disc_fingerprint(&drive_path.to_string_lossy());

        // 1. Check local persistent fingerprint cache for instant offline match
        let cached_meta = crate::imdb::lookup_fingerprint_cache(&fingerprint);

        // 2. Fall back to online metadata provider lookup if not in cache
        let metadata = cached_meta.or_else(|| {
            if let Some(ref provider) = self.metadata_provider {
                if let Some(ref label) = volume_label {
                    let clean_title = crate::utils::normalize_volume_label_title(label);
                    provider
                        .get_details_by_title_and_year(&clean_title, None)
                        .ok()
                        .flatten()
                } else {
                    None
                }
            } else {
                None
            }
        });

        // 3. Rank best theatrical title using duration cluster detector
        let expected_runtime = metadata.as_ref().and_then(|m| m.runtime_secs);
        let (primary_title_num, primary_duration_secs) =
            rank_best_title_from_slice(&titles, expected_runtime);

        // 4. Detect TV episodes using TV detector heuristic
        let show_title = metadata
            .as_ref()
            .map(|m| m.title.as_str())
            .unwrap_or("TV Show");
        let tv_episodes = detect_tv_episodes_from_titles(&titles, show_title, 1, 1, None);

        let disc_info = DiscInfo {
            device_path: drive_path.to_path_buf(),
            volume_label,
            fingerprint,
            titles,
            protection,
        };

        Ok(DiscProbeResult {
            disc_info,
            primary_title_num,
            primary_duration_secs,
            metadata,
            tv_episodes,
        })
    }
}
