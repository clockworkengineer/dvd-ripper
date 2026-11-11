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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_history_repository() {
        let repo = InMemoryHistoryRepository::new();
        assert!(repo.records().is_empty());
        assert!(repo.load_history(None).is_empty());

        let record = RipRecord::new("Test Movie", "Movie", "C:\\out\\movie.mkv", "Success");
        repo.append_record(&record, None).unwrap();

        let loaded = repo.load_history(None);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].title, "Test Movie");

        repo.clear_history(None).unwrap();
        assert!(repo.records().is_empty());
    }

    #[test]
    fn test_recording_notifier_all_events() {
        let notifier = RecordingNotifier::new();

        notifier
            .send(&NotificationEvent::RipStarted {
                title: "Movie A",
                drive: "D:\\",
            })
            .unwrap();

        notifier
            .send(&NotificationEvent::RipProgress {
                title: "Movie A",
                percent: 45.5,
            })
            .unwrap();

        notifier
            .send(&NotificationEvent::RipSuccess {
                title: "Movie A",
                output_file: "movie_a.mkv",
                duration_secs: 7200.0,
            })
            .unwrap();

        notifier
            .send(&NotificationEvent::RipFailed {
                title: "Movie A",
                error_message: "Disk error",
            })
            .unwrap();

        notifier
            .send(&NotificationEvent::RipCancelled { title: "Movie A" })
            .unwrap();

        let events = notifier.events();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].kind, "RipStarted");
        assert_eq!(events[0].detail, "D:\\");
        assert_eq!(events[1].kind, "RipProgress");
        assert_eq!(events[1].detail, "45.5");
        assert_eq!(events[2].kind, "RipSuccess");
        assert_eq!(events[2].detail, "movie_a.mkv");
        assert_eq!(events[3].kind, "RipFailed");
        assert_eq!(events[3].detail, "Disk error");
        assert_eq!(events[4].kind, "RipCancelled");
        assert_eq!(events[4].detail, "");
    }

    #[test]
    fn test_fixed_disk_space_guard() {
        let guard_default = FixedDiskSpaceGuard::default();
        assert_eq!(guard_default.available_gb, 500);

        let guard = FixedDiskSpaceGuard::new(20);
        let path = Path::new("C:\\");
        assert!(guard.check_free_space(path, 15).is_ok());
        let err = guard.check_free_space(path, 25).unwrap_err();
        assert!(err.to_string().contains("Insufficient disk space"));
    }

    #[test]
    fn test_recording_progress_observer() {
        let observer = RecordingProgressObserver::new();
        assert!(observer.progress_updates().is_empty());
        assert!(observer.logs().is_empty());

        observer.on_progress(50.0, "30.0", "1.5x");
        observer.on_log("Processing sector 1234...");

        let updates = observer.progress_updates();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0], (50.0, "30.0".to_string(), "1.5x".to_string()));

        let logs = observer.logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0], "Processing sector 1234...");
    }
}

