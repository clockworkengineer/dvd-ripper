//! Domain ports for media stream inspection and frame capture.

use crate::domain::entities::disc::DvdTitleInfo;
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
