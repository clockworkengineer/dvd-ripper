use crate::domain::entities::disc::{DvdTitleInfo, TitleStreamDetails};
use crate::domain::ports::media_probe::{StreamProber, ThumbnailExtractor, VideoAnalyzer};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Concrete ThumbnailExtractor, StreamProber, and VideoAnalyzer implementation using FFmpeg CLI.
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

    fn probe_stream_details(&self, dvd_path: &Path, title_num: u32) -> TitleStreamDetails {
        crate::ffmpeg::probe_title_stream_details(&self.ffmpeg_path, dvd_path, title_num)
    }
}

impl VideoAnalyzer for FfmpegMediaProber {
    fn predict_optimal_crf(
        &self,
        dvd_path: &Path,
        title_num: u32,
        codec: &str,
        target_ssim: f64,
    ) -> (u8, f64) {
        let res = crate::ffmpeg::predict_optimal_crf(
            &self.ffmpeg_path,
            dvd_path,
            title_num,
            codec,
            target_ssim,
        );
        (res.tested_crf, res.mean_ssim)
    }

    fn detect_auto_crop(&self, dvd_path: &Path, title_num: u32) -> Option<String> {
        crate::ffmpeg::detect_crop_dimensions(&self.ffmpeg_path, dvd_path, title_num, Some(120.0))
    }

    fn detect_forced_subtitle(
        &self,
        dvd_path: &Path,
        title_num: u32,
        preferred_lang: &str,
    ) -> Option<u32> {
        let details = self.probe_stream_details(dvd_path, title_num);
        let candidates: Vec<crate::domain::SubtitleCandidateMetrics> = details
            .subtitle_streams
            .iter()
            .map(|s| crate::domain::SubtitleCandidateMetrics {
                stream_index: s.stream_index,
                language: s.language.clone(),
                forced_flag: s.forced,
                cue_count: if s.forced { 10 } else { 0 },
                total_active_secs: 0.0,
                title_duration_secs: details.duration_secs,
            })
            .collect();
        crate::domain::detect_foreign_audio_forced_subtitle(&candidates, preferred_lang)
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
