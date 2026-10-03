use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Thread-safe coordinator for active optical drives and cancellation flags.
#[derive(Debug, Default)]
pub struct DrivePool {
    active_drives: Mutex<HashMap<PathBuf, Arc<AtomicBool>>>,
}

impl DrivePool {
    pub fn new() -> Self {
        Self {
            active_drives: Mutex::new(HashMap::new()),
        }
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
}
