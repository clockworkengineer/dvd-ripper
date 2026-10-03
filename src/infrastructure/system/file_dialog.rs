use std::path::{Path, PathBuf};
use crate::domain::ports::dialogs::FileDialogService;

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
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = starting_dir {
            dialog = dialog.set_directory(dir);
        }
        dialog.pick_folder()
    }

    fn open_in_explorer(&self, directory: &Path) {
        crate::utils::open_folder_in_explorer(directory);
    }
}
