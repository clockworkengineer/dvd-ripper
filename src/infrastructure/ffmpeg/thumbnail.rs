use crate::domain::entities::disc::DvdTitleInfo;
use crate::domain::ports::media_probe::{StreamProber, ThumbnailExtractor};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Concrete ThumbnailExtractor and StreamProber implementation using FFmpeg CLI.
#[derive(Debug, Clone)]
pub struct FfmpegMediaProber {
    ffmpeg_path: String,
}

impl FfmpegMediaProber {
    pub fn new(ffmpeg_path: impl Into<String>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
        }
    }
}

impl ThumbnailExtractor for FfmpegMediaProber {
    fn extract_thumbnail(
        &self,
        dvd_path: &Path,
        title_num: u32,
        timestamp_secs: f64,
        output_path: &Path,
    ) -> Result<PathBuf> {
        crate::ffmpeg::generate_title_thumbnail(
            &self.ffmpeg_path,
            dvd_path,
            title_num,
            timestamp_secs,
            output_path,
        )
    }
}

impl StreamProber for FfmpegMediaProber {
    fn probe_titles(&self, dvd_path: &Path) -> Vec<DvdTitleInfo> {
        let titles = crate::ffmpeg::probe_dvd_titles(&self.ffmpeg_path, dvd_path, None);
        titles
            .into_iter()
            .map(|t| DvdTitleInfo {
                title_num: t.title_num,
                duration_secs: t.duration_secs,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_media_prober_construction() {
        let prober = FfmpegMediaProber::new("ffmpeg");
        assert_eq!(prober.ffmpeg_path, "ffmpeg");

        let cloned = prober.clone();
        assert_eq!(format!("{:?}", prober), format!("{:?}", cloned));

        // probe_titles with nonexistent path returns empty vec
        let titles = prober.probe_titles(Path::new("Z:\\DefinitelyNonExistent"));
        assert!(titles.is_empty());

        // extract_thumbnail with nonexistent path fails gracefully
        let res = prober.extract_thumbnail(
            Path::new("Z:\\DefinitelyNonExistent"),
            1,
            10.0,
            Path::new("dummy_out.jpg"),
        );
        assert!(res.is_err());
    }
}
