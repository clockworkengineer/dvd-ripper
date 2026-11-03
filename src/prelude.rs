//! # DVD Ripper Library Prelude
//!
//! Convenient re-exports of core domain entities, port traits, application use cases,
//! and composition root containers for external library consumers.
//!
//! ## Example
//!
//! ```no_run
//! use dvd_ripper::prelude::*;
//!
//! let container = AppContainer::build_production("ffmpeg");
//! let probe_uc = container.probe_disc_use_case();
//! ```

// Domain Entities
pub use crate::domain::entities::{
    Chapter, DiscCopyProtectionReport, DiscInfo, DiscOptions, DriveStatusInfo, DvdError,
    DvdTitleInfo, FilmMetadata, JobStatus, MediaType, RipProgress, RipRecord, SearchCandidate,
    TvEpisodeInfo, TvEpisodeMetadata,
};

// Domain Ports
pub use crate::domain::ports::{
    benchmark::DriveBenchmarkService,
    dialogs::FileDialogService,
    media_probe::{StreamProber, ThumbnailExtractor},
    metadata::{MetadataProvider, TvSeriesMetadataProvider},
    notification::{NotificationEvent, Notifier},
    ocr::OcrProcessor,
    optical_drive::{
        DiscInspector, DriveDetector, DriveIdentity, OpticalDrive, ProtectionInspector,
        TitleProber, TrayController, TrayOperator,
    },
    storage::{DiskSpaceGuard, HistoryRepository},
    transcoder::{
        DefaultVideoCodecResolver, ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
        VideoCodecResolver,
    },
};

// Application Use Cases & DTOs
pub use crate::application::{
    DiscProbeResult, DrivePool, DumpIsoRequest, DumpIsoRequestBuilder, DumpIsoUseCase,
    ProbeDiscUseCase, RipSingleTitleRequest, RipSingleTitleRequestBuilder, RipSingleTitleUseCase,
    RipTvBatchRequest, RipTvBatchRequestBuilder, RipTvBatchUseCase,
};

// Application Container & Lifecycle
pub use crate::infrastructure::storage::TempResource;
pub use crate::{AppContainer, AppInitializer, DefaultPathResolver, PathResolver};
