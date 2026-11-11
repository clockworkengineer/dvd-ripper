//! Auto-Fallback Composite Transcoder adapter.
//!
//! Tries the primary transcoder (e.g. FFmpeg) first. If it fails with a copy protection,
//! demux, or bad-sector error, it automatically falls back to secondary engines (e.g.,
//! Native IFO Cell Extractor, HandBrakeCLI, or MakeMKV) to ensure rip completion.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, Transcoder,
};

/// Composite Transcoder that tries multiple engine implementations in priority order.
pub struct AutoFallbackTranscoder {
    engines: Vec<(&'static str, Arc<dyn Transcoder>)>,
}

impl AutoFallbackTranscoder {
    /// Creates a new `AutoFallbackTranscoder` with a list of named transcoders in fallback order.
    pub fn new(engines: Vec<(&'static str, Arc<dyn Transcoder>)>) -> Self {
        Self { engines }
    }
}

impl Transcoder for AutoFallbackTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        self.transcode_chapter_range(job, None, observer, cancel_flag)
    }

    fn transcode_chapter_range(
        &self,
        job: &TranscodeJob,
        chapter_range: Option<(u32, u32)>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        if self.engines.is_empty() {
            return Err(anyhow!("No transcoding engines registered in AutoFallbackTranscoder."));
        }

        let mut last_error = None;
        let total_engines = self.engines.len();

        for (idx, (name, engine)) in self.engines.iter().enumerate() {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                return Err(anyhow!("Transcoding cancelled by user."));
            }

            if idx > 0 {
                let msg = format!(
                    "[Auto Engine Fallback] Primary engine encountered an error. Falling back to engine '{}' ({}/{})...",
                    name,
                    idx + 1,
                    total_engines
                );
                if let Some(obs) = observer {
                    obs.on_log(&msg);
                } else {
                    println!("{}", msg);
                }
            }

            match engine.transcode_chapter_range(job, chapter_range, observer, cancel_flag.clone()) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("cancelled by user") {
                        return Err(err);
                    }
                    if let Some(obs) = observer {
                        obs.on_log(&format!(
                            "[Auto Engine Fallback] Engine '{}' failed: {}",
                            name, err_str
                        ));
                    }
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All transcoding engines failed.")))
    }
}

