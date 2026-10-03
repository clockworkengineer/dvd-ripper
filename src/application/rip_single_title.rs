use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use anyhow::Result;

use crate::domain::entities::job::RipRecord;
use crate::domain::entities::media::FilmMetadata;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::{ProgressObserver, TranscodeJob, Transcoder};

/// Request parameter object for executing a single movie or episode ripping job.
#[derive(Debug, Clone)]
pub struct RipSingleTitleRequest {
    pub source_dvd: PathBuf,
    pub title_number: u32,
    pub output_path: PathBuf,
    pub display_title: String,
    pub expected_duration_secs: Option<f64>,
    pub metadata: Option<FilmMetadata>,
    pub min_free_gb: u64,
}

/// Use case that orchestrates the single-title ripping workflow:
/// 1. Verifies free disk space threshold.
/// 2. Dispatches RipStarted notification.
/// 3. Executes transcoding with live progress observer.
/// 4. Generates sidecar .nfo file and saves artwork if metadata exists.
/// 5. Records event in persistent history.
/// 6. Dispatches RipSuccess or RipFailed notification.
pub struct RipSingleTitleUseCase {
    transcoder: Arc<dyn Transcoder>,
    notifier: Arc<dyn Notifier>,
    disk_guard: Arc<dyn DiskSpaceGuard>,
    history_repo: Arc<dyn HistoryRepository>,
}

impl RipSingleTitleUseCase {
    pub fn new(
        transcoder: Arc<dyn Transcoder>,
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
        history_repo: Arc<dyn HistoryRepository>,
    ) -> Self {
        Self {
            transcoder,
            notifier,
            disk_guard,
            history_repo,
        }
    }

    pub fn execute(
        &self,
        request: &RipSingleTitleRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let parent_dir = request.output_path.parent().unwrap_or(&request.output_path);

        // 1. Check disk space guard
        self.disk_guard.check_free_space(parent_dir, request.min_free_gb)?;

        // 2. Notify start
        let drive_str = request.source_dvd.to_string_lossy();
        let _ = self.notifier.send(&NotificationEvent::RipStarted {
            title: &request.display_title,
            drive: &drive_str,
        });

        // 3. Assemble transcode job
        let job = TranscodeJob {
            source_dvd: &request.source_dvd,
            title_number: request.title_number,
            output_path: &request.output_path,
            display_title: &request.display_title,
            expected_duration_secs: request.expected_duration_secs,
        };

        // 4. Execute transcoding
        let start_time = std::time::Instant::now();
        let transcode_res = self.transcoder.transcode(&job, observer, cancel_flag);

        match transcode_res {
            Ok(()) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let out_str = request.output_path.to_string_lossy();

                // Generate NFO sidecar if metadata exists
                if let Some(ref meta) = request.metadata {
                    let _ = crate::infrastructure::storage::nfo_writer::generate_nfo_file(
                        &request.output_path,
                        meta,
                        if meta.is_series { "tvshow" } else { "movie" },
                    );
                }

                // Append record to history
                let record = RipRecord::new(
                    &request.display_title,
                    if request.metadata.as_ref().map_or(false, |m| m.is_series) { "tv" } else { "movie" },
                    &out_str,
                    "Completed",
                );
                let _ = self.history_repo.append_record(&record, None);

                // Notify success
                let _ = self.notifier.send(&NotificationEvent::RipSuccess {
                    title: &request.display_title,
                    output_file: &out_str,
                    duration_secs,
                });

                Ok(())
            }
            Err(e) => {
                let err_str = e.to_string();
                let out_str = request.output_path.to_string_lossy();

                let record = RipRecord::new(
                    &request.display_title,
                    "unknown",
                    &out_str,
                    "Failed",
                );
                let _ = self.history_repo.append_record(&record, None);

                let _ = self.notifier.send(&NotificationEvent::RipFailed {
                    title: &request.display_title,
                    error_message: &err_str,
                });

                Err(e)
            }
        }
    }
}
