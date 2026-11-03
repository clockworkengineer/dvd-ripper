use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::domain::entities::error::DvdError;
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

impl RipSingleTitleRequest {
    /// Creates a new fluent builder for constructing a [`RipSingleTitleRequest`].
    ///
    /// # Parameters
    /// - `source_dvd`: Path to optical disc drive or mounted DVD folder.
    /// - `title_number`: Optical disc title index (1-based).
    /// - `output_path`: Target file destination (e.g., `Movie.mp4`).
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::application::RipSingleTitleRequest;
    ///
    /// let request = RipSingleTitleRequest::builder("D:\\", 1, "C:\\Videos\\Movie.mp4")
    ///     .display_title("The Matrix")
    ///     .min_free_gb(15)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(request.title_number, 1);
    /// assert_eq!(request.display_title, "The Matrix");
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn builder(
        source_dvd: impl Into<PathBuf>,
        title_number: u32,
        output_path: impl Into<PathBuf>,
    ) -> RipSingleTitleRequestBuilder {
        RipSingleTitleRequestBuilder::new(source_dvd, title_number, output_path)
    }
}

/// Fluent builder for constructing [`RipSingleTitleRequest`] instances.
#[derive(Debug, Clone)]
pub struct RipSingleTitleRequestBuilder {
    source_dvd: PathBuf,
    title_number: u32,
    output_path: PathBuf,
    display_title: Option<String>,
    expected_duration_secs: Option<f64>,
    metadata: Option<FilmMetadata>,
    min_free_gb: u64,
}

impl RipSingleTitleRequestBuilder {
    pub fn new(
        source_dvd: impl Into<PathBuf>,
        title_number: u32,
        output_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_dvd: source_dvd.into(),
            title_number,
            output_path: output_path.into(),
            display_title: None,
            expected_duration_secs: None,
            metadata: None,
            min_free_gb: 10,
        }
    }

    pub fn display_title(mut self, title: impl Into<String>) -> Self {
        self.display_title = Some(title.into());
        self
    }

    pub fn expected_duration(mut self, secs: f64) -> Self {
        self.expected_duration_secs = Some(secs);
        self
    }

    pub fn metadata(mut self, metadata: FilmMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn min_free_gb(mut self, gb: u64) -> Self {
        self.min_free_gb = gb;
        self
    }

    pub fn build(self) -> Result<RipSingleTitleRequest> {
        let display_title = self.display_title.unwrap_or_else(|| {
            self.output_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("Title {}", self.title_number))
        });

        Ok(RipSingleTitleRequest {
            source_dvd: self.source_dvd,
            title_number: self.title_number,
            output_path: self.output_path,
            display_title,
            expected_duration_secs: self.expected_duration_secs,
            metadata: self.metadata,
            min_free_gb: self.min_free_gb,
        })
    }
}

/// Use case that orchestrates the single-title ripping workflow:
/// 1. Verifies free disk space threshold.
/// 2. Dispatches RipStarted notification.
/// 3. Executes transcoding with live progress observer.
/// 4. Generates sidecar .nfo file and saves artwork if metadata exists.
/// 5. Records event in persistent history.
/// 6. Dispatches RipSuccess, RipCancelled, or RipFailed notification.
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

    /// Executes the single-title ripping workflow.
    ///
    /// # Parameters
    /// - `request`: Specification for source disc, title, and output path.
    /// - `observer`: Optional progress and log stream callback handler.
    /// - `cancel_flag`: Optional cooperative thread cancellation token.
    ///
    /// # Errors
    /// Returns [`DvdError::InsufficientDiskSpace`](crate::domain::entities::error::DvdError::InsufficientDiskSpace)
    /// if available free space falls below `min_free_gb`.
    /// Returns an error if the underlying transcoder fails or process execution errors out.
    ///
    /// # Panics
    /// Never panics.
    pub fn execute(
        &self,
        request: &RipSingleTitleRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
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
        let transcode_res = self
            .transcoder
            .transcode(&job, observer, cancel_flag.clone());

        let media_type = if request.metadata.as_ref().is_some_and(|m| m.is_series) {
            "tv"
        } else {
            "movie"
        };
        let out_str = request.output_path.to_string_lossy();

        match transcode_res {
            Ok(()) => {
                let duration_secs = start_time.elapsed().as_secs_f64();

                // Generate NFO sidecar if metadata exists
                if let Some(ref meta) = request.metadata {
                    let _ = crate::infrastructure::storage::nfo_writer::generate_nfo_file(
                        &request.output_path,
                        meta,
                        if meta.is_series { "tvshow" } else { "movie" },
                    );
                }

                // Append record to history
                let record = RipRecord::with_details(
                    &request.display_title,
                    media_type,
                    &out_str,
                    "Completed",
                    None,
                    request.metadata.as_ref().and_then(|m| m.year),
                    Some(duration_secs),
                    std::fs::metadata(&request.output_path)
                        .ok()
                        .map(|m| m.len()),
                    None,
                    None,
                    None,
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
                let is_cancelled = cancel_flag
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::SeqCst))
                    || e.to_string().to_lowercase().contains("cancel");

                if is_cancelled {
                    let record =
                        RipRecord::new(&request.display_title, media_type, &out_str, "Cancelled");
                    let _ = self.history_repo.append_record(&record, None);

                    let _ = self.notifier.send(&NotificationEvent::RipCancelled {
                        title: &request.display_title,
                    });

                    Err(DvdError::Cancelled.into())
                } else {
                    let err_str = e.to_string();
                    let record =
                        RipRecord::new(&request.display_title, media_type, &out_str, "Failed");
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
