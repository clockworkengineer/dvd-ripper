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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::domain::ports::testing::RecordingProgressObserver;

    struct DummyEngine {
        fail: bool,
        cancel: bool,
    }

    impl Transcoder for DummyEngine {
        fn transcode(
            &self,
            _job: &TranscodeJob,
            _observer: Option<&dyn ProgressObserver>,
            _cancel_flag: Option<Arc<AtomicBool>>,
        ) -> Result<()> {
            if self.cancel {
                return Err(anyhow!("operation cancelled by user"));
            }
            if self.fail {
                return Err(anyhow!("demux error: bad sector"));
            }
            Ok(())
        }
    }

    fn sample_job() -> TranscodeJob<'static> {
        TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: Path::new("out.mkv"),
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        }
    }

    #[test]
    fn test_fallback_empty_engines() {
        let fallback = AutoFallbackTranscoder::new(vec![]);
        let job = sample_job();
        let res = fallback.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("No transcoding engines registered"));
    }

    #[test]
    fn test_fallback_first_succeeds() {
        let e1 = Arc::new(DummyEngine { fail: false, cancel: false });
        let e2 = Arc::new(DummyEngine { fail: true, cancel: false });
        let fallback = AutoFallbackTranscoder::new(vec![("engine1", e1), ("engine2", e2)]);

        let job = sample_job();
        let obs = RecordingProgressObserver::new();
        assert!(fallback.transcode(&job, Some(&obs), None).is_ok());
        assert!(obs.logs().is_empty());
    }

    #[test]
    fn test_fallback_first_fails_second_succeeds() {
        let e1 = Arc::new(DummyEngine { fail: true, cancel: false });
        let e2 = Arc::new(DummyEngine { fail: false, cancel: false });
        let fallback = AutoFallbackTranscoder::new(vec![("engine1", e1), ("engine2", e2)]);

        let job = sample_job();
        let obs = RecordingProgressObserver::new();
        assert!(fallback.transcode(&job, Some(&obs), None).is_ok());
        let logs = obs.logs();
        assert_eq!(logs.len(), 2);
        assert!(logs[0].contains("Engine 'engine1' failed"));
        assert!(logs[1].contains("Falling back to engine 'engine2'"));
    }

    #[test]
    fn test_fallback_all_fail() {
        let e1 = Arc::new(DummyEngine { fail: true, cancel: false });
        let e2 = Arc::new(DummyEngine { fail: true, cancel: false });
        let fallback = AutoFallbackTranscoder::new(vec![("engine1", e1), ("engine2", e2)]);

        let job = sample_job();
        let obs = RecordingProgressObserver::new();
        let res = fallback.transcode(&job, Some(&obs), None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("bad sector"));
    }

    #[test]
    fn test_fallback_cancellation_pre_flag() {
        let e1 = Arc::new(DummyEngine { fail: false, cancel: false });
        let fallback = AutoFallbackTranscoder::new(vec![("engine1", e1)]);

        let job = sample_job();
        let flag = Arc::new(AtomicBool::new(true));
        let res = fallback.transcode(&job, None, Some(flag));
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled by user"));
    }

    #[test]
    fn test_fallback_cancellation_from_sub_engine_aborts_fallback() {
        let e1 = Arc::new(DummyEngine { fail: false, cancel: true });
        let e2 = Arc::new(DummyEngine { fail: false, cancel: false });
        let fallback = AutoFallbackTranscoder::new(vec![("engine1", e1), ("engine2", e2)]);

        let job = sample_job();
        let res = fallback.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled by user"));
    }
}


