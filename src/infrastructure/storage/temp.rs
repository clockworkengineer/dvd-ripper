//! RAII temporary resource manager ensuring collision-free paths and automated cleanup.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII Temporary file resource that generates a collision-resistant path in the system
/// temp directory and automatically cleans up the file on [`Drop`].
///
/// # Examples
///
/// ```
/// use dvd_ripper::infrastructure::storage::TempResource;
/// use std::fs;
///
/// let path;
/// {
///     let temp = TempResource::new("test_resource", "tmp");
///     path = temp.path().to_path_buf();
///     fs::write(&path, b"temporary data").unwrap();
///     assert!(path.exists());
/// } // temp dropped here
/// assert!(!path.exists());
/// ```
#[derive(Debug)]
pub struct TempResource {
    path: PathBuf,
    persist: bool,
}

impl TempResource {
    /// Creates a new temporary resource with a given prefix and file extension.
    ///
    /// The filename incorporates the current process ID, timestamp in nanos,
    /// and an atomic sequence counter to guarantee uniqueness.
    pub fn new(prefix: &str, extension: &str) -> Self {
        let pid = std::process::id();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let ext = if extension.is_empty() {
            String::new()
        } else if extension.starts_with('.') {
            extension.to_string()
        } else {
            format!(".{}", extension)
        };

        let file_name = format!("{}_{}_{}_{}{}", prefix, pid, nanos, counter, ext);
        let path = std::env::temp_dir().join(file_name);

        Self {
            path,
            persist: false,
        }
    }

    /// Returns a reference to the temporary file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Disarms the automatic deletion on [`Drop`], allowing the file to persist.
    pub fn persist(&mut self) {
        self.persist = true;
    }

    /// Consumes the resource, disarms cleanup, and returns the underlying [`PathBuf`].
    pub fn into_path(mut self) -> PathBuf {
        self.persist = true;
        self.path.clone()
    }
}

impl AsRef<Path> for TempResource {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempResource {
    fn drop(&mut self) {
        if !self.persist && self.path.exists() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_resource_lifecycle() {
        let path;
        {
            let temp = TempResource::new("unit_test", "txt");
            path = temp.path().to_path_buf();
            fs::write(&path, b"hello temp world").unwrap();
            assert!(path.exists());
        }
        assert!(!path.exists());
    }

    #[test]
    fn test_temp_resource_persist() {
        let path;
        {
            let mut temp = TempResource::new("persist_test", "txt");
            path = temp.path().to_path_buf();
            fs::write(&path, b"persist this file").unwrap();
            temp.persist();
        }
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_temp_resource_into_path_and_as_ref() {
        let temp_empty_ext = TempResource::new("no_ext", "");
        assert_eq!(temp_empty_ext.as_ref(), temp_empty_ext.path());

        let temp_dot_ext = TempResource::new("dot_ext", ".custom");
        let path = temp_dot_ext.into_path();
        assert!(path.to_string_lossy().ends_with(".custom"));
    }
}
