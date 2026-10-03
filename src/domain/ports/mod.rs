pub mod dialogs;
pub mod metadata;
pub mod notification;
pub mod optical_drive;
pub mod storage;
pub mod transcoder;

pub use dialogs::FileDialogService;
pub use metadata::MetadataProvider;
pub use notification::{NotificationEvent, Notifier};
pub use optical_drive::{DriveDetector, OpticalDrive};
pub use storage::{DiskSpaceGuard, HistoryRepository};
pub use transcoder::{ProgressObserver, TranscodeJob, Transcoder};
