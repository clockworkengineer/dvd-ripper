//! Use case for executing 1:1 byte-for-byte archival ISO optical disc dumps.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::domain::entities::error::DvdError;
use crate::domain::entities::job::RipRecord;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::ProgressObserver;

/// Request parameter object for executing a 1:1 ISO disc dump.
#[derive(Debug, Clone)]
pub struct DumpIsoRequest {
    pub source_dvd: PathBuf,
    pub output_path: PathBuf,
    pub volume_label: Option<String>,
    pub display_title: String,
    pub min_free_gb: u64,
}

/// Use case that orchestrates sector-by-sector archival ISO imaging:
/// 1. Verifies free disk space threshold on target partition.
/// 2. Dispatches RipStarted notification.
/// 3. Streams live sector copying progress to observers.
/// 4. Handles cooperative cancellation and records "Cancelled" or "Completed" in history.
/// 5. Dispatches RipSuccess, RipCancelled, or RipFailed notifications.
pub struct DumpIsoUseCase {
    notifier: Arc<dyn Notifier>,
    disk_guard: Arc<dyn DiskSpaceGuard>,
    history_repo: Arc<dyn HistoryRepository>,
}

impl DumpIsoUseCase {
    pub fn new(
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
        history_repo: Arc<dyn HistoryRepository>,
    ) -> Self {
        Self {
            notifier,
            disk_guard,
            history_repo,
        }
    }

    pub fn execute(
        &self,
        request: &DumpIsoRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<u64> {
        let parent_dir = request.output_path.parent().unwrap_or(&request.output_path);

        // 1. Check disk space guard
        self.disk_guard
            .check_free_space(parent_dir, request.min_free_gb)?;

        // 2. Notify start
        let drive_str = request.source_dvd.to_string_lossy();
        let _ = self.notifier.send(&NotificationEvent::RipStarted {
            title: &request.display_title,
            drive: &drive_str,
        });

        let start_time = std::time::Instant::now();

        // 3. Setup progress channel if observer provided
        let (tx, rx) = std::sync::mpsc::channel();

        let dump_res = if let Some(obs) = observer {
            let src_str = request.source_dvd.to_string_lossy().to_string();
            let out_path = request.output_path.clone();
            let label = request.volume_label.clone();
            let cancel_clone = cancel_flag.clone();

            let worker = std::thread::spawn(move || {
                crate::iso::dump_disc_to_iso(
                    &src_str,
                    &out_path,
                    label.as_deref(),
                    cancel_clone.as_ref(),
                    Some(&tx),
                )
            });

            while let Ok(event) = rx.recv() {
                match event {
                    crate::ffmpeg::ProgressEvent::Progress {
                        percent,
                        fps,
                        speed,
                    } => {
                        obs.on_progress(percent, &fps, &speed);
                    }
                    crate::ffmpeg::ProgressEvent::Log(line) => {
                        obs.on_log(&line);
                    }
                    _ => {}
                }
            }

            match worker.join() {
                Ok(res) => res,
                Err(_) => Err(anyhow::anyhow!("ISO dump worker thread panicked")),
            }
        } else {
            let src_str = request.source_dvd.to_string_lossy();
            crate::iso::dump_disc_to_iso(
                &src_str,
                &request.output_path,
                request.volume_label.as_deref(),
                cancel_flag.as_ref(),
                None,
            )
        };

        let out_str = request.output_path.to_string_lossy();

        match dump_res {
            Ok(bytes_written) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let record = RipRecord::with_details(
                    &request.display_title,
                    "ISO Image",
                    &out_str,
                    "Completed",
                    request.volume_label.clone(),
                    None,
                    Some(duration_secs),
                    Some(bytes_written),
                    Some("copy".to_string()),
                    None,
                    None,
                );
                let _ = self.history_repo.append_record(&record, None);

                let _ = self.notifier.send(&NotificationEvent::RipSuccess {
                    title: &request.display_title,
                    output_file: &out_str,
                    duration_secs,
                });

                Ok(bytes_written)
            }
            Err(e) => {
                let is_cancelled = cancel_flag
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::SeqCst))
                    || e.to_string().to_lowercase().contains("cancel");

                if is_cancelled {
                    let record =
                        RipRecord::new(&request.display_title, "ISO Image", &out_str, "Cancelled");
                    let _ = self.history_repo.append_record(&record, None);

                    let _ = self.notifier.send(&NotificationEvent::RipCancelled {
                        title: &request.display_title,
                    });

                    Err(DvdError::Cancelled.into())
                } else {
                    let err_str = e.to_string();
                    let record =
                        RipRecord::new(&request.display_title, "ISO Image", &out_str, "Failed");
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
}
