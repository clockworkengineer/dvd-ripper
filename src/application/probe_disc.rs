use crate::domain::entities::disc::DiscInfo;
use crate::domain::entities::media::FilmMetadata;
use crate::domain::heuristics::cluster_detector::rank_best_title_from_slice;
use crate::domain::ports::metadata::MetadataProvider;
use crate::domain::ports::optical_drive::OpticalDrive;
use anyhow::Result;
use std::sync::Arc;

/// Result of probing a disc, combining disc hardware details, title tracks, and metadata.
#[derive(Debug, Clone)]
pub struct DiscProbeResult {
    pub disc_info: DiscInfo,
    pub primary_title_num: u32,
    pub primary_duration_secs: Option<f64>,
    pub metadata: Option<FilmMetadata>,
}

/// Use case that probes optical disc layout, analyzes copy protection, ranks titles, and fetches metadata.
pub struct ProbeDiscUseCase {
    metadata_provider: Option<Arc<dyn MetadataProvider>>,
}

impl ProbeDiscUseCase {
    pub fn new(metadata_provider: Option<Arc<dyn MetadataProvider>>) -> Self {
        Self { metadata_provider }
    }

    pub fn execute(&self, drive: &dyn OpticalDrive) -> Result<DiscProbeResult> {
        let drive_path = drive.get_path();
        let volume_label = drive.read_volume_label();
        let protection = drive.inspect_protection();
        let titles = drive.probe_titles();

        let fingerprint = crate::dvd::compute_disc_fingerprint(&drive_path.to_string_lossy());

        // Attempt metadata lookup by volume label or cache
        let metadata = if let Some(ref provider) = self.metadata_provider {
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
        };

        let expected_runtime = metadata.as_ref().and_then(|m| m.runtime_secs);
        let (primary_title_num, primary_duration_secs) =
            rank_best_title_from_slice(&titles, expected_runtime);

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
        })
    }
}
