use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Immutable job specification for transcoding a title track.
pub struct TranscodeJob<'a> {
    pub source_dvd: &'a Path,
    pub title_number: u32,
    pub output_path: &'a Path,
    pub display_title: &'a str,
    pub expected_duration_secs: Option<f64>,
}

/// Observer interface for decoupled transcode progress notifications.
pub trait ProgressObserver: Send + Sync {
    fn on_progress(&self, percent: f64, fps: &str, speed: &str);
    fn on_log(&self, line: &str);
    fn on_title_start(&self, _title_num: u32) {}
    fn on_title_complete(&self, _title_num: u32, _output_path: &Path) {}
}

/// Abstraction for media transcoding and stream copying engines.
pub trait Transcoder: Send + Sync {
    /// Transcodes an optical disc title track according to the specified job parameters.
    ///
    /// # Parameters
    /// - `job`: Specification of input source disc, title number, and destination output path.
    /// - `observer`: Optional progress and log stream callback handler.
    /// - `cancel_flag`: Optional cooperative cancellation token.
    ///
    /// # Errors
    /// Returns an error if transcoding fails, process terminates abnormally, or job is cancelled.
    ///
    /// # Panics
    /// Never panics under normal operation.
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()>;
}

/// Open/Closed Principle (OCP/SOLID): Trait contract for video codec strategy resolvers.
pub trait VideoCodecResolver: Send + Sync {
    fn resolve(&self, codec: &str) -> &'static str;
}

/// Default video codec resolver implementing software encoders (H.264, HEVC, AV1).
///
/// # Examples
///
/// ```
/// use dvd_ripper::domain::ports::transcoder::{DefaultVideoCodecResolver, VideoCodecResolver};
///
/// let resolver = DefaultVideoCodecResolver;
/// assert_eq!(resolver.resolve("hevc"), "libx265");
/// assert_eq!(resolver.resolve("av1"), "libsvtav1");
/// assert_eq!(resolver.resolve("h264"), "libx264");
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultVideoCodecResolver;

impl VideoCodecResolver for DefaultVideoCodecResolver {
    fn resolve(&self, codec: &str) -> &'static str {
        match codec.to_lowercase().as_str() {
            "hevc" | "h265" => "libx265",
            "av1" => "libsvtav1",
            _ => "libx264",
        }
    }
}

/// Typed, focused options configuring video, audio, and subtitle transcoding parameters.
///
/// # Examples
///
/// ```
/// use dvd_ripper::domain::ports::TranscodeOptions;
///
/// let options = TranscodeOptions {
///     codec: "hevc".to_string(),
///     preset: "medium".to_string(),
///     ..Default::default()
/// };
/// assert_eq!(options.codec, "hevc");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscodeOptions {
    pub transcode: bool,
    pub mkv: bool,
    pub codec: String,
    pub profile: String,
    pub preset: String,
    pub hwaccel: String,
    pub hwaccel_fallback: bool,
    pub deinterlace: bool,
    pub deinterlace_algo: Option<String>,
    pub denoise: bool,
    pub all_audio: bool,
    pub normalize_audio: bool,
    pub norm_target: Option<i32>,
    pub dual_audio: bool,
    pub audio_track: Option<u32>,
    pub audio_lang: Option<String>,
    pub auto_audio_pref: Option<String>,
    pub audio_title: Option<String>,
    pub audio_downmix: Option<String>,
    pub subtitles: bool,
    pub sub_lang: Option<String>,
    pub sub_format: Option<String>,
    pub sub_forced_only: bool,
    pub sub_default: bool,
    pub sub_burnin: bool,
    pub tonemap: Option<String>,
    pub ignore_errors: bool,
    #[serde(default)]
    pub start_chapter: Option<u32>,
    #[serde(default)]
    pub end_chapter: Option<u32>,
    #[serde(default)]
    pub split_chapters: bool,
    #[serde(default)]
    pub angle: Option<u32>,
    #[serde(default)]
    pub auto_crop: bool,
    #[serde(default)]
    pub detelecine: bool,
    #[serde(default)]
    pub crf: Option<u8>,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            transcode: true,
            mkv: false,
            codec: "h264".to_string(),
            profile: "standard".to_string(),
            preset: "slow".to_string(),
            hwaccel: "none".to_string(),
            hwaccel_fallback: true,
            deinterlace: false,
            deinterlace_algo: None,
            denoise: false,
            all_audio: false,
            normalize_audio: false,
            norm_target: None,
            dual_audio: false,
            audio_track: None,
            audio_lang: None,
            auto_audio_pref: None,
            audio_title: None,
            audio_downmix: None,
            subtitles: false,
            sub_lang: None,
            sub_format: None,
            sub_forced_only: false,
            sub_default: false,
            sub_burnin: false,
            tonemap: None,
            ignore_errors: false,
            start_chapter: None,
            end_chapter: None,
            split_chapters: false,
            angle: None,
            auto_crop: false,
            detelecine: false,
            crf: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_video_codec_resolver() {
        let resolver = DefaultVideoCodecResolver;
        assert_eq!(resolver.resolve("hevc"), "libx265");
        assert_eq!(resolver.resolve("h265"), "libx265");
        assert_eq!(resolver.resolve("HEVC"), "libx265");
        assert_eq!(resolver.resolve("H265"), "libx265");
        assert_eq!(resolver.resolve("av1"), "libsvtav1");
        assert_eq!(resolver.resolve("AV1"), "libsvtav1");
        assert_eq!(resolver.resolve("h264"), "libx264");
        assert_eq!(resolver.resolve("unknown"), "libx264");
        assert_eq!(resolver.resolve("copy"), "libx264");

        let cloned = resolver;
        assert_eq!(format!("{:?}", resolver), format!("{:?}", cloned));
    }

    #[test]
    fn test_transcode_job_and_options() {
        let path = Path::new("test.mp4");
        let src = Path::new("D:\\");
        let job = TranscodeJob {
            source_dvd: src,
            title_number: 1,
            output_path: path,
            display_title: "Test Title",
            expected_duration_secs: Some(3600.0),
        };
        assert_eq!(job.title_number, 1);
        assert_eq!(job.display_title, "Test Title");
        assert_eq!(job.expected_duration_secs, Some(3600.0));

        let opts = TranscodeOptions::default();
        let cloned_opts = opts.clone();
        assert_eq!(opts, cloned_opts);

        let json = serde_json::to_string(&opts).unwrap();
        let de: TranscodeOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, de);
    }
}
