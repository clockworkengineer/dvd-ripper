use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use anyhow::Result;

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
