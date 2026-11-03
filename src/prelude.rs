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
    post_processor::{PostProcessContext, PostProcessor},
    storage::{DiskSpaceGuard, HistoryRepository},
    transcoder::{
        DefaultVideoCodecResolver, ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
        VideoCodecResolver,
    },
};

// Heuristics Strategies (OCP)
pub use crate::domain::heuristics::{
    ClusterTitleRankingStrategy, DurationClusteringTvStrategy, EpisodeDetectionStrategy,
    LongestTrackStrategy, TitleRankingStrategy, TvDetectionConfig,
};

// Application Use Cases & DTOs
pub use crate::application::{
    DiscProbeResult, DrivePool, DumpIsoRequest, DumpIsoRequestBuilder, DumpIsoUseCase,
    ProbeDiscUseCase, RipSingleTitleRequest, RipSingleTitleRequestBuilder, RipSingleTitleUseCase,
    RipTvBatchRequest, RipTvBatchRequestBuilder, RipTvBatchUseCase,
};

// Infrastructure Adapters (DIP)
pub use crate::infrastructure::{
    ffmpeg::{FfmpegMediaProber, FfmpegTranscoder},
    metadata::{OmdbMetadataProvider, TvmazeMetadataProvider},
    optical::{
        FfmpegDriveBenchmarkService, PhysicalOpticalDrive, SystemDiscInspector,
        SystemDriveDetector, SystemTrayController,
    },
    post_processors::PostProcessingPipeline,
    storage::{JsonHistoryRepository, SystemDiskSpaceGuard, TempResource},
    system::NativeFileDialogService,
};

// Application Container & Lifecycle
pub use crate::{AppContainer, AppInitializer, DefaultPathResolver, PathResolver};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_prelude_reexports() {
        let container = AppContainer::build_production("ffmpeg");
        assert!(Arc::strong_count(&container.transcoder) >= 1);

        let resolver = DefaultPathResolver;
        let p = resolver.resolve_path("D:\\");
        assert!(!p.as_os_str().is_empty());

        let _disc_opts = DiscOptions::default();
        let _trans_opts = TranscodeOptions::default();
        let _status = DriveStatusInfo::new("D:\\");
        let _codec_res = DefaultVideoCodecResolver;
    }
}
