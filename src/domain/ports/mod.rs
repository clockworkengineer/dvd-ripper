pub mod benchmark;
pub mod dialogs;
pub mod media_probe;
pub mod metadata;
pub mod notification;
pub mod ocr;
pub mod optical_drive;
pub mod post_processor;
pub mod storage;
pub mod transcoder;

pub use benchmark::DriveBenchmarkService;
pub use dialogs::FileDialogService;
pub use media_probe::{StreamProber, ThumbnailExtractor};
pub use metadata::{MetadataProvider, TvSeriesMetadataProvider};
pub use notification::{NotificationEvent, Notifier};
pub use ocr::OcrProcessor;
pub use optical_drive::{
    DiscInspector, DriveDetector, DriveIdentity, OpticalDrive, ProtectionInspector, TitleProber,
    TrayController, TrayOperator,
};
pub use post_processor::{PostProcessContext, PostProcessor};
pub use storage::{DiskSpaceGuard, HistoryRepository};
pub use transcoder::{
    DefaultVideoCodecResolver, ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
    VideoCodecResolver,
};
