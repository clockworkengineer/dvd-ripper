use crate::domain::ports::dialogs::FileDialogService;
use std::path::{Path, PathBuf};

/// Concrete implementation of FileDialogService using rfd and OS shell.
#[derive(Debug, Default, Clone)]
pub struct NativeFileDialogService;

impl NativeFileDialogService {
    pub fn new() -> Self {
        Self
    }
}

impl FileDialogService for NativeFileDialogService {
    fn pick_folder(&self, starting_dir: Option<&Path>, title: &str) -> Option<PathBuf> {
        #[cfg(feature = "gui")]
        {
            let mut dialog = rfd::FileDialog::new().set_title(title);
            if let Some(dir) = starting_dir {
                dialog = dialog.set_directory(dir);
            }
            dialog.pick_folder()
        }
        #[cfg(not(feature = "gui"))]
        {
            let _ = (starting_dir, title);
            None
        }
    }

    fn open_in_explorer(&self, directory: &Path) {
        crate::utils::open_folder_in_explorer(directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_dialog_service_instantiation() {
        let service = NativeFileDialogService::new();
        let service_def = NativeFileDialogService::default();
        let cloned = service.clone();
        assert_eq!(format!("{:?}", service), format!("{:?}", cloned));
        assert_eq!(format!("{:?}", service), format!("{:?}", service_def));

        // Note: pick_folder cannot be invoked in headless CI, but open_in_explorer can be called
        service.open_in_explorer(Path::new("dummy_nonexistent_dir"));
    }
}
