//! Thread-safe coordinator for active optical drives, status tracking, and cancellation flags.

use crate::domain::entities::disc::DriveStatusInfo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Thread-safe coordinator for active optical drives, status tracking, and cancellation flags.
#[derive(Debug, Default)]
pub struct DrivePool {
    active_drives: Mutex<HashMap<PathBuf, Arc<AtomicBool>>>,
    statuses: Mutex<HashMap<String, DriveStatusInfo>>,
}

impl DrivePool {
    /// Creates a new, empty [`DrivePool`].
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::application::DrivePool;
    ///
    /// let pool = DrivePool::new();
    /// assert_eq!(pool.get_statuses().len(), 0);
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn new() -> Self {
        Self {
            active_drives: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
        }
    }

    /// Initializes pool with a list of optical drive paths.
    pub fn init_drives(&self, drives: &[String]) {
        let mut map = self.statuses.lock().unwrap();
        for drive in drives {
            map.entry(drive.clone())
                .or_insert_with(|| DriveStatusInfo::new(drive));
        }
    }

    /// Retrieves status list for all drives, sorted by drive path.
    pub fn get_statuses(&self) -> Vec<DriveStatusInfo> {
        let map = self.statuses.lock().unwrap();
        let mut list: Vec<DriveStatusInfo> = map.values().cloned().collect();
        list.sort_by(|a, b| a.drive.cmp(&b.drive));
        list
    }

    /// Retrieves status for a specific drive.
    pub fn get_drive_status(&self, drive: &str) -> Option<DriveStatusInfo> {
        let map = self.statuses.lock().unwrap();
        map.get(drive).cloned()
    }

    /// Updates status entry for a drive.
    pub fn update_drive_status<F>(&self, drive: &str, update_fn: F)
    where
        F: FnOnce(&mut DriveStatusInfo),
    {
        let mut map = self.statuses.lock().unwrap();
        let entry = map
            .entry(drive.to_string())
            .or_insert_with(|| DriveStatusInfo::new(drive));
        update_fn(entry);
    }

    /// Registers a drive as actively ripping and returns its atomic cancel flag handle.
    pub fn register(&self, drive_path: &Path) -> Arc<AtomicBool> {
        let mut map = self.active_drives.lock().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        map.insert(drive_path.to_path_buf(), flag.clone());
        flag
    }

    /// Signals cancellation for an ongoing rip on a specific drive.
    pub fn cancel(&self, drive_path: &Path) -> bool {
        let map = self.active_drives.lock().unwrap();
        if let Some(flag) = map.get(drive_path) {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// Signals cancellation across all currently running rip operations.
    pub fn cancel_all(&self) {
        let map = self.active_drives.lock().unwrap();
        for flag in map.values() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Deregisters a drive when its job completes or errors.
    pub fn unregister(&self, drive_path: &Path) {
        let mut map = self.active_drives.lock().unwrap();
        map.remove(drive_path);
    }

    /// Checks if a drive is actively ripping.
    pub fn is_active(&self, drive_path: &Path) -> bool {
        let map = self.active_drives.lock().unwrap();
        map.contains_key(drive_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_pool_lifecycle() {
        let pool = DrivePool::new();
        let p = Path::new("D:\\");
        assert!(!pool.is_active(p));

        let flag = pool.register(p);
        assert!(pool.is_active(p));
        assert!(!flag.load(Ordering::SeqCst));

        assert!(pool.cancel(p));
        assert!(flag.load(Ordering::SeqCst));

        pool.unregister(p);
        assert!(!pool.is_active(p));
    }

    #[test]
    fn test_drive_pool_status_tracking() {
        let pool = DrivePool::new();
        let drives = vec!["D:\\".to_string(), "E:\\".to_string()];
        pool.init_drives(&drives);

        let statuses = pool.get_statuses();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].drive, "D:\\");
        assert_eq!(statuses[1].drive, "E:\\");

        pool.update_drive_status("D:\\", |st| {
            st.status = "Ripping".to_string();
            st.progress = 50.0;
        });

        let updated = pool.get_drive_status("D:\\").unwrap();
        assert_eq!(updated.status, "Ripping");
        assert_eq!(updated.progress, 50.0);
    }
}
