//! Domain ports for media stream inspection and frame capture.

use crate::domain::entities::disc::{DvdTitleInfo, TitleStreamDetails};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Abstraction for extracting visual video frame thumbnails.
pub trait ThumbnailExtractor: Send + Sync {
    /// Extracts a JPEG thumbnail from the specified DVD title at the given offset.
    ///
    /// # Errors
    /// Returns an error if thumbnail generation fails or output path is invalid.
    fn extract_thumbnail(
        &self,
        dvd_path: &Path,
        title_num: u32,
        timestamp_secs: f64,
        output_path: &Path,
    ) -> Result<PathBuf>;
}

/// Abstraction for probing titles and stream metadata from media files or optical discs.
pub trait StreamProber: Send + Sync {
    /// Probes available title tracks and durations from the specified source disc or file.
    fn probe_titles(&self, dvd_path: &Path) -> Vec<DvdTitleInfo>;

    /// Probes detailed chapter markers, audio streams, subtitle streams, and angles for a specific title.
    fn probe_stream_details(&self, _dvd_path: &Path, title_num: u32) -> TitleStreamDetails {
        TitleStreamDetails {
            title_num,
            ..Default::default()
        }
    }
}

/// Interface Segregation (ISP/SOLID): Abstraction for active video complexity, crop, and forced-subtitle analysis.
pub trait VideoAnalyzer: Send + Sync {
    /// Predicts the optimal Constant Rate Factor (CRF) for a target SSIM perceptual threshold.
    fn predict_optimal_crf(
        &self,
        dvd_path: &Path,
        title_num: u32,
        codec: &str,
        target_ssim: f64,
    ) -> (u8, f64);

    /// Detects automatic letterbox/pillarbox crop dimensions (`w:h:x:y`) if present.
    fn detect_auto_crop(&self, dvd_path: &Path, title_num: u32) -> Option<String>;

    /// Scans subtitle tracks to identify a foreign-audio forced subtitle stream index.
    fn detect_forced_subtitle(
        &self,
        dvd_path: &Path,
        title_num: u32,
        preferred_lang: &str,
    ) -> Option<u32>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockThumbnailExtractor;

    impl ThumbnailExtractor for MockThumbnailExtractor {
        fn extract_thumbnail(
            &self,
            _dvd_path: &Path,
            _title_num: u32,
            _timestamp_secs: f64,
            output_path: &Path,
        ) -> Result<PathBuf> {
            Ok(output_path.to_path_buf())
        }
    }

    struct MockStreamProber;

    impl StreamProber for MockStreamProber {
        fn probe_titles(&self, _dvd_path: &Path) -> Vec<DvdTitleInfo> {
            vec![DvdTitleInfo {
                title_num: 1,
                duration_secs: 5400.0,
            }]
        }
    }

    #[test]
    fn test_mock_thumbnail_and_prober_contract() {
        let extractor = MockThumbnailExtractor;
        let out = extractor
            .extract_thumbnail(Path::new("D:\\"), 1, 10.0, Path::new("thumb.jpg"))
            .unwrap();
        assert_eq!(out, PathBuf::from("thumb.jpg"));

        let prober = MockStreamProber;
        let titles = prober.probe_titles(Path::new("D:\\"));
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].title_num, 1);
        assert_eq!(titles[0].duration_secs, 5400.0);
    }
}
