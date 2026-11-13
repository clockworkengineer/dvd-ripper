//! Autonomous Optical Media Arrival Monitor.
//!
//! Watches optical disc drives for disc insertion events, transitioning the system
//! from idle standby to automated recognition, ripping, and ejection.

use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Optical disc insertion event details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaArrivalEvent {
    /// Filesystem or device path of the optical drive (e.g. "D:\" or "/dev/sr0").
    pub drive_path: PathBuf,
    /// Detected volume label, if available.
    pub volume_label: Option<String>,
}

/// Domain contract for monitoring optical drive state transitions.
pub trait MediaArrivalMonitor: Send + Sync {
    /// Polls or waits for a new optical disc arrival event across monitored drives.
    /// Returns `Some(event)` when a newly inserted disc is detected.
    fn check_for_arrival(&self, drive_path: &Path) -> Option<MediaArrivalEvent>;
}

/// Polling-based implementation that tracks known loaded drive states and fires on transition.
pub struct PollingMediaMonitor {
    loaded_drives: std::sync::Mutex<HashSet<PathBuf>>,
}

impl Default for PollingMediaMonitor {
    fn default() -> Self {
        Self {
            loaded_drives: std::sync::Mutex::new(HashSet::new()),
        }
    }
}

impl PollingMediaMonitor {
    /// Creates a new monitor instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets the internal state of a drive (e.g. after it has been ejected).
    pub fn mark_ejected(&self, drive_path: &Path) {
        if let Ok(mut set) = self.loaded_drives.lock() {
            set.remove(drive_path);
        }
    }

    /// Monitors in a loop until a disc arrives or `cancel_flag` is set.
    pub fn wait_for_disc(
        &self,
        drive_path: &Path,
        poll_interval: Duration,
        cancel_flag: &AtomicBool,
    ) -> Result<Option<MediaArrivalEvent>> {
        while !cancel_flag.load(Ordering::Relaxed) {
            if let Some(event) = self.check_for_arrival(drive_path) {
                return Ok(Some(event));
            }
            std::thread::sleep(poll_interval);
        }
        Ok(None)
    }
}

impl MediaArrivalMonitor for PollingMediaMonitor {
    fn check_for_arrival(&self, drive_path: &Path) -> Option<MediaArrivalEvent> {
        let mut set = self.loaded_drives.lock().ok()?;
        let label_opt = crate::dvd::get_volume_label(drive_path.to_string_lossy().as_ref());
        let has_video_ts = drive_path.join("VIDEO_TS").exists();
        let is_loaded = label_opt.is_some() || has_video_ts;

        if is_loaded {
            if !set.contains(drive_path) {
                set.insert(drive_path.to_path_buf());
                return Some(MediaArrivalEvent {
                    drive_path: drive_path.to_path_buf(),
                    volume_label: label_opt,
                });
            }
        } else {
            set.remove(drive_path);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_state_transition() {
        let monitor = PollingMediaMonitor::new();
        let path = Path::new("Z:\\VirtualNonExistentDrive");

        // Initially not a DVD -> None
        assert!(monitor.check_for_arrival(path).is_none());

        // Mark ejected works cleanly
        monitor.mark_ejected(path);
    }
}
