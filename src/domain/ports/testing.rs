//! Reusable, thread-safe in-memory test doubles for domain ports.
//!
//! Designed for deterministic unit and integration testing without filesystem I/O,
//! network sockets, or external FFmpeg subprocesses.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, anyhow};

use crate::domain::entities::job::RipRecord;
use crate::domain::ports::{DiskSpaceGuard, HistoryRepository, NotificationEvent, Notifier, ProgressObserver};

/// Thread-safe, in-memory [`HistoryRepository`] test double.
///
/// # Examples
///
/// ```
/// use dvd_ripper::domain::entities::job::RipRecord;
/// use dvd_ripper::domain::ports::HistoryRepository;
/// use dvd_ripper::domain::ports::testing::InMemoryHistoryRepository;
///
/// let repo = InMemoryHistoryRepository::new();
/// repo.append_record(
///     &RipRecord::new("The Matrix", "Movie", "/out/The Matrix.mkv", "Success"),
///     None,
/// ).unwrap();
/// assert_eq!(repo.records().len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct InMemoryHistoryRepository {
    records: Mutex<Vec<RipRecord>>,
}

impl InMemoryHistoryRepository {
    /// Creates an empty [`InMemoryHistoryRepository`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of all recorded [`RipRecord`] entries.
    pub fn records(&self) -> Vec<RipRecord> {
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl HistoryRepository for InMemoryHistoryRepository {
    fn load_history(&self, _custom_path: Option<&Path>) -> Vec<RipRecord> {
        self.records()
    }

    fn append_record(&self, record: &RipRecord, _custom_path: Option<&Path>) -> Result<()> {
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(record.clone());
        Ok(())
    }

    fn clear_history(&self, _custom_path: Option<&Path>) -> Result<()> {
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        Ok(())
    }
}

/// Owned representation of a recorded [`NotificationEvent`] for assertions in tests.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedNotification {
    pub kind: String,
    pub title: String,
    pub detail: String,
}

/// Thread-safe [`Notifier`] test double that records all dispatched notification events in memory.
#[derive(Debug, Default)]
pub struct RecordingNotifier {
    events: Mutex<Vec<RecordedNotification>>,
}

impl RecordingNotifier {
    /// Creates a new [`RecordingNotifier`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of all recorded notifications.
    pub fn events(&self) -> Vec<RecordedNotification> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Notifier for RecordingNotifier {
    fn send(&self, event: &NotificationEvent) -> Result<()> {
        let recorded = match event {
            NotificationEvent::RipStarted { title, drive } => RecordedNotification {
                kind: "RipStarted".to_string(),
                title: (*title).to_string(),
                detail: (*drive).to_string(),
            },
            NotificationEvent::RipProgress { title, percent } => RecordedNotification {
                kind: "RipProgress".to_string(),
                title: (*title).to_string(),
                detail: format!("{:.1}", percent),
            },
            NotificationEvent::RipSuccess {
                title,
                output_file,
                ..
            } => RecordedNotification {
                kind: "RipSuccess".to_string(),
                title: (*title).to_string(),
                detail: (*output_file).to_string(),
            },
            NotificationEvent::RipFailed {
                title,
                error_message,
            } => RecordedNotification {
                kind: "RipFailed".to_string(),
                title: (*title).to_string(),
                detail: (*error_message).to_string(),
            },
            NotificationEvent::RipCancelled { title } => RecordedNotification {
                kind: "RipCancelled".to_string(),
                title: (*title).to_string(),
                detail: String::new(),
            },
        };
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(recorded);
        Ok(())
    }
}

/// Configurable [`DiskSpaceGuard`] test double that simulates a fixed amount of available free space in GB.
#[derive(Debug, Clone, Copy)]
pub struct FixedDiskSpaceGuard {
    pub available_gb: u64,
}

impl FixedDiskSpaceGuard {
    /// Creates a [`FixedDiskSpaceGuard`] reporting `available_gb` of free disk capacity.
    pub fn new(available_gb: u64) -> Self {
        Self { available_gb }
    }
}

impl Default for FixedDiskSpaceGuard {
    fn default() -> Self {
        Self { available_gb: 500 }
    }
}

impl DiskSpaceGuard for FixedDiskSpaceGuard {
    fn check_free_space(&self, _path: &Path, min_free_gb: u64) -> Result<()> {
        if self.available_gb < min_free_gb {
            return Err(anyhow!(
                "Insufficient disk space: {} GB available, {} GB required",
                self.available_gb,
                min_free_gb
            ));
        }
        Ok(())
    }
}

/// Thread-safe [`ProgressObserver`] test double that records progress percentages and log lines.
#[derive(Debug, Default)]
pub struct RecordingProgressObserver {
    progress_updates: Mutex<Vec<(f64, String, String)>>,
    logs: Mutex<Vec<String>>,
}

impl RecordingProgressObserver {
    /// Creates an empty [`RecordingProgressObserver`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all `(percent, fps, speed)` progress snapshots recorded so far.
    pub fn progress_updates(&self) -> Vec<(f64, String, String)> {
        self.progress_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Returns all log lines recorded so far.
    pub fn logs(&self) -> Vec<String> {
        self.logs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl ProgressObserver for RecordingProgressObserver {
    fn on_progress(&self, percent: f64, fps: &str, speed: &str) {
        self.progress_updates
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((percent, fps.to_string(), speed.to_string()));
    }

    fn on_log(&self, line: &str) {
        self.logs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(line.to_string());
    }
}
