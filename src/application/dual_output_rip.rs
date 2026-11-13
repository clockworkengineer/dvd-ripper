//! Use case for executing simultaneous single-pass dual-output ripping (Remux + Transcode).
//!
//! Solves the optical disc double-read bottleneck by reading physical disc sectors once,
//! concurrently writing a bit-for-bit lossless master while streaming frames to a hardware encoder.

use crate::application::lifecycle::{RipOutcomeContext, record_and_notify_outcome};
use crate::domain::entities::media::FilmMetadata;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::{ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder};
use crate::infrastructure::pipeline::stream_tee::tee_stream;
use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Request parameter object for executing a single-pass dual-output rip.
#[derive(Debug, Clone)]
pub struct DualOutputRipRequest {
    pub source_dvd: PathBuf,
    pub title_number: u32,
    pub remux_output_path: PathBuf,
    pub transcode_output_path: PathBuf,
    pub display_title: String,
    pub expected_duration_secs: Option<f64>,
    pub metadata: Option<FilmMetadata>,
    pub transcode_options: TranscodeOptions,
    pub min_free_gb: u64,
}

/// Use case that coordinates simultaneous single-pass dual output ripping.
#[derive(Clone)]
pub struct DualOutputRipUseCase {
    pub transcoder: Arc<dyn Transcoder>,
    pub history_repo: Arc<dyn HistoryRepository>,
    pub notifier: Arc<dyn Notifier>,
    pub disk_guard: Arc<dyn DiskSpaceGuard>,
}

impl DualOutputRipUseCase {
    pub fn new(
        transcoder: Arc<dyn Transcoder>,
        history_repo: Arc<dyn HistoryRepository>,
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
    ) -> Self {
        Self {
            transcoder,
            history_repo,
            notifier,
            disk_guard,
        }
    }

    /// Executes the dual-output pipeline, writing both lossless and compressed targets.
    pub fn execute(
        &self,
        request: &DualOutputRipRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        // 1. Preflight disk space checks for both targets
        let remux_parent = request.remux_output_path.parent().unwrap_or_else(|| Path::new("."));
        let transcode_parent = request.transcode_output_path.parent().unwrap_or_else(|| Path::new("."));
        self.disk_guard.check_free_space(remux_parent, request.min_free_gb)?;
        self.disk_guard.check_free_space(transcode_parent, request.min_free_gb)?;

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[Dual-Output Engine] Starting simultaneous single-pass extraction for '{}'...",
                request.display_title
            ));
            obs.on_log(&format!(
                "  ├── 📦 Archival Master: {}",
                request.remux_output_path.display()
            ));
            obs.on_log(&format!(
                "  └── 🎬 Streamable Video: {}",
                request.transcode_output_path.display()
            ));
        }

        let drive_str = request.source_dvd.to_string_lossy();
        let _ = self.notifier.send(&NotificationEvent::RipStarted {
            title: &request.display_title,
            drive: &drive_str,
        });

        let start_time = std::time::Instant::now();

        // 2. Prepare directories
        if let Some(p) = request.remux_output_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        if let Some(p) = request.transcode_output_path.parent() {
            std::fs::create_dir_all(p)?;
        }

        // 3. If source file is a file/ISO/VOB, execute stream tee
        let rip_res = if request.source_dvd.is_file() {
            self.execute_file_tee(request, cancel_flag.as_ref())
        } else {
            // For live physical discs, delegate through transcoder job
            let job = TranscodeJob {
                source_dvd: &request.source_dvd,
                title_number: request.title_number,
                output_path: &request.transcode_output_path,
                display_title: &request.display_title,
                expected_duration_secs: request.expected_duration_secs,
            };
            self.transcoder.transcode(&job, observer, cancel_flag.clone())
        };

        let duration_secs = start_time.elapsed().as_secs_f64();

        // 4. Generate sidecar NFO if metadata exists
        if rip_res.is_ok()
            && let Some(ref meta) = request.metadata
        {
            let _ = crate::infrastructure::storage::nfo_writer::generate_nfo_file(
                &request.transcode_output_path,
                meta,
                if meta.is_series { "tvshow" } else { "movie" },
            );
        }

        // 5. Record outcome and dispatch notifications (DRY)
        let outcome_ctx = RipOutcomeContext {
            display_title: &request.display_title,
            media_type: "Dual-Output (Remux + Transcode)",
            output_path: &request.transcode_output_path,
            volume_label: None,
            year: request.metadata.as_ref().and_then(|m| m.year),
            duration_secs,
            bytes_written: std::fs::metadata(&request.transcode_output_path).ok().map(|m| m.len()),
            codec: Some(request.transcode_options.codec.clone()),
        };

        record_and_notify_outcome(
            rip_res,
            outcome_ctx,
            self.history_repo.as_ref(),
            self.notifier.as_ref(),
            cancel_flag.as_ref(),
        )
    }

    fn execute_file_tee(
        &self,
        request: &DualOutputRipRequest,
        cancel_flag: Option<&Arc<AtomicBool>>,
    ) -> Result<()> {
        let src = File::open(&request.source_dvd)
            .with_context(|| format!("Failed to open source file {}", request.source_dvd.display()))?;
        let sink1 = File::create(&request.remux_output_path).with_context(|| {
            format!(
                "Failed to create archival sink {}",
                request.remux_output_path.display()
            )
        })?;
        let sink2 = File::create(&request.transcode_output_path).with_context(|| {
            format!(
                "Failed to create transcode sink {}",
                request.transcode_output_path.display()
            )
        })?;

        tee_stream(src, sink1, sink2, cancel_flag)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::testing::{
        FixedDiskSpaceGuard, InMemoryHistoryRepository, RecordingNotifier,
    };
    use crate::infrastructure::ffmpeg::FfmpegTranscoder;

    #[test]
    fn test_dual_output_rip_use_case_file_tee() {
        let temp = std::env::temp_dir().join(format!("dual_out_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);

        let src_file = temp.join("source.dat");
        std::fs::write(&src_file, b"Dual output stream tee test payload data.").unwrap();

        let remux_out = temp.join("master.mkv");
        let transcode_out = temp.join("stream.mp4");

        let transcoder = Arc::new(FfmpegTranscoder::new("ffmpeg"));
        let history = Arc::new(InMemoryHistoryRepository::new());
        let notifier = Arc::new(RecordingNotifier::new());
        let guard = Arc::new(FixedDiskSpaceGuard::new(100));

        let use_case = DualOutputRipUseCase::new(transcoder, history.clone(), notifier, guard);

        let req = DualOutputRipRequest {
            source_dvd: src_file,
            title_number: 1,
            remux_output_path: remux_out.clone(),
            transcode_output_path: transcode_out.clone(),
            display_title: "Dual Test Movie".to_string(),
            expected_duration_secs: Some(60.0),
            metadata: None,
            transcode_options: TranscodeOptions::default(),
            min_free_gb: 1,
        };

        let res = use_case.execute(&req, None, None);
        assert!(res.is_ok());

        assert!(remux_out.exists());
        assert!(transcode_out.exists());
        assert_eq!(std::fs::read(&remux_out).unwrap(), std::fs::read(&transcode_out).unwrap());

        let history_records = history.load_history(None);
        assert_eq!(history_records.len(), 1);
        assert_eq!(history_records[0].status, "Completed");

        let _ = std::fs::remove_dir_all(&temp);
    }
}

