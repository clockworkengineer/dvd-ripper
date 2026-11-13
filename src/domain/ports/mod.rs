pub mod acoustic_supervisor;
pub mod benchmark;
pub mod dialogs;
pub mod iso_archiver;
pub mod media_probe;
pub mod metadata;
pub mod notification;
pub mod ocr;
pub mod optical_drive;
pub mod post_processor;
pub mod queue;
pub mod rosetta;
pub mod storage;
pub mod testing;
pub mod transcoder;

pub use acoustic_supervisor::{AcousticSupervisor, DriveAcousticProfile};
pub use benchmark::DriveBenchmarkService;
pub use dialogs::FileDialogService;
pub use iso_archiver::IsoArchiver;
pub use media_probe::{StreamProber, ThumbnailExtractor, VideoAnalyzer};
pub use metadata::{DiscFingerprintRepository, MetadataProvider, TvSeriesMetadataProvider};
pub use notification::{NotificationEvent, Notifier};
pub use ocr::OcrProcessor;
pub use optical_drive::{
    DiscInspector, DriveDetector, DriveIdentity, OpticalDrive, ProtectionInspector, TitleProber,
    TrayController, TrayOperator,
};
pub use post_processor::{PostProcessContext, PostProcessor};
pub use queue::{BoxSetRepository, JobQueueRepository};
pub use rosetta::{RosettaDiscEntry, RosettaEpisodeEntry, RosettaStoneRegistry};
pub use storage::{DiskSpaceGuard, HistoryRepository};
pub use testing::{
    FixedDiskSpaceGuard, InMemoryHistoryRepository, RecordedNotification, RecordingNotifier,
    RecordingProgressObserver,
};
pub use transcoder::{
    DefaultVideoCodecResolver, ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
    VideoCodecResolver,
};
