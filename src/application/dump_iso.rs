//! Use case for executing 1:1 byte-for-byte archival ISO optical disc dumps.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::domain::ports::iso_archiver::IsoArchiver;
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

impl DumpIsoRequest {
    /// Creates a new fluent builder for constructing a [`DumpIsoRequest`].
    ///
    /// # Parameters
    /// - `source_dvd`: Path to optical drive device or disc mount.
    /// - `output_path`: Target `.iso` file destination.
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::application::DumpIsoRequest;
    ///
    /// let request = DumpIsoRequest::builder("D:\\", "C:\\Backups\\Disc.iso")
    ///     .volume_label("MY_DISC")
    ///     .min_free_gb(10)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(request.display_title, "MY_DISC");
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn builder(
        source_dvd: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> DumpIsoRequestBuilder {
        DumpIsoRequestBuilder::new(source_dvd, output_path)
    }
}

/// Fluent builder for constructing [`DumpIsoRequest`] instances.
#[derive(Debug, Clone)]
pub struct DumpIsoRequestBuilder {
    source_dvd: PathBuf,
    output_path: PathBuf,
    volume_label: Option<String>,
    display_title: Option<String>,
    min_free_gb: u64,
}

impl DumpIsoRequestBuilder {
    pub fn new(source_dvd: impl Into<PathBuf>, output_path: impl Into<PathBuf>) -> Self {
        Self {
            source_dvd: source_dvd.into(),
            output_path: output_path.into(),
            volume_label: None,
            display_title: None,
            min_free_gb: 10,
        }
    }

    pub fn volume_label(mut self, label: impl Into<String>) -> Self {
        self.volume_label = Some(label.into());
        self
    }

    pub fn display_title(mut self, title: impl Into<String>) -> Self {
        self.display_title = Some(title.into());
        self
    }

    pub fn min_free_gb(mut self, gb: u64) -> Self {
        self.min_free_gb = gb;
        self
    }

    pub fn build(self) -> Result<DumpIsoRequest> {
        let display_title = self.display_title.unwrap_or_else(|| {
            self.volume_label
                .clone()
                .unwrap_or_else(|| "Archival ISO Dump".to_string())
        });

        Ok(DumpIsoRequest {
            source_dvd: self.source_dvd,
            output_path: self.output_path,
            volume_label: self.volume_label,
            display_title,
            min_free_gb: self.min_free_gb,
        })
    }
}

/// Use case that orchestrates sector-by-sector archival ISO imaging:
/// 1. Verifies free disk space threshold on target partition.
/// 2. Dispatches RipStarted notification.
/// 3. Streams live sector copying progress to observers.
/// 4. Handles cooperative cancellation and records "Cancelled" or "Completed" in history.
/// 5. Dispatches RipSuccess, RipCancelled, or RipFailed notifications.
pub struct DumpIsoUseCase {
    iso_archiver: Arc<dyn IsoArchiver>,
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
            iso_archiver: Arc::new(crate::infrastructure::optical::SystemIsoArchiver::new()),
            notifier,
            disk_guard,
            history_repo,
        }
    }

    /// Creates a [`DumpIsoUseCase`] with an explicitly injected [`IsoArchiver`] port (DIP/LSP).
    pub fn with_archiver(
        iso_archiver: Arc<dyn IsoArchiver>,
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
        history_repo: Arc<dyn HistoryRepository>,
    ) -> Self {
        Self {
            iso_archiver,
            notifier,
            disk_guard,
            history_repo,
        }
    }

    /// Executes the 1:1 archival sector-by-sector ISO dump.
    ///
    /// # Parameters
    /// - `request`: Specification for source optical device and output ISO path.
    /// - `observer`: Optional progress observer receiving real-time sector copy percentage.
    /// - `cancel_flag`: Optional cooperative cancellation token.
    ///
    /// # Errors
    /// Returns [`DvdError::InsufficientDiskSpace`](crate::domain::entities::error::DvdError::InsufficientDiskSpace)
    /// if the target disk has insufficient space.
    /// Returns an error if sector copying is cancelled or fails with an I/O error.
    ///
    /// # Panics
    /// Never panics.
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

        // 3. Delegate sector imaging to the injected IsoArchiver port
        let dump_res = self.iso_archiver.dump_disc_to_iso(
            &request.source_dvd,
            &request.output_path,
            request.volume_label.as_deref(),
            observer,
            cancel_flag.clone(),
        );

        let duration_secs = start_time.elapsed().as_secs_f64();
        let bytes_written = dump_res.as_ref().ok().copied();

        let outcome_ctx = super::lifecycle::RipOutcomeContext {
            display_title: &request.display_title,
            media_type: "ISO Image",
            output_path: &request.output_path,
            volume_label: request.volume_label.clone(),
            year: None,
            duration_secs,
            bytes_written,
            codec: Some("copy".to_string()),
        };

        super::lifecycle::record_and_notify_outcome(
            dump_res,
            outcome_ctx,
            self.history_repo.as_ref(),
            self.notifier.as_ref(),
            cancel_flag.as_ref(),
        )
    }
}
