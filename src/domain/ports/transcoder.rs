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
        }
    }
}
