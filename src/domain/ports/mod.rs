pub mod dialogs;
pub mod metadata;
pub mod notification;
pub mod ocr;
pub mod optical_drive;
pub mod storage;
pub mod transcoder;

pub use dialogs::FileDialogService;
pub use metadata::MetadataProvider;
pub use notification::{NotificationEvent, Notifier};
pub use ocr::OcrProcessor;
pub use optical_drive::{DiscInspector, DriveDetector, OpticalDrive, TrayController};
pub use storage::{DiskSpaceGuard, HistoryRepository};
pub use transcoder::{ProgressObserver, TranscodeJob, Transcoder};
