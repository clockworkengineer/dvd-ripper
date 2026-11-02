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
