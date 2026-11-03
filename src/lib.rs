//! # DVD Ripper
//!
//! A high-performance, automated DVD backup and transcoding library written in Rust.
//!
//! DVD Ripper provides pure domain models, heuristics (automatic TV series episode detection,
//! telecine 3:2 pulldown detection, multi-angle parsing), port trait abstractions, and application
//! use cases for DVD ripping, 1:1 archival ISO dumping, and optical drive probing.
//!
//! ## Core Architecture
//!
//! Following Clean Architecture principles, `dvd-ripper` is structured into decoupled layers:
//! - [`domain`]: Pure business rules, entities ([`domain::entities`]), and port contracts ([`domain::ports`]).
//! - [`application`]: Orchestration use cases ([`application::RipSingleTitleUseCase`],
//!   [`application::RipTvBatchUseCase`], [`application::DumpIsoUseCase`], [`application::ProbeDiscUseCase`]).
//! - [`infrastructure`]: Adapters implementing domain ports for FFmpeg, optical drive OS APIs, and storage.
//! - [`prelude`]: Re-exports commonly used types, traits, and service containers.
//!
//! ## Feature Flags
//!
//! | Feature | Description | Default |
//! | :--- | :--- | :--- |
//! | `core` | Pure library core with zero GUI dependencies. | Yes |
//! | `cli` | Command-line interface parser and utilities. | Yes |
//! | `gui` | Portable native desktop graphical interface (`eframe`/`egui`). | Yes |
//! | `api` | Embedded HTTP REST appliance server and Prometheus metrics. | No |
//! | `full` | Enables all features (`cli`, `gui`, `api`). | No |
//!
//! ## Quickstart Example
//!
//! ```no_run
//! use dvd_ripper::prelude::*;
//!
//! // 1. Construct application container
//! let container = AppContainer::build_production("ffmpeg");
//!
//! // 2. Build a rip request using the fluent builder
//! let request = RipSingleTitleRequest::builder("D:\\", 1, "C:\\Videos\\Movie.mp4")
//!     .display_title("Example Movie")
//!     .min_free_gb(10)
//!     .build()
//!     .expect("valid request configuration");
//!
//! // 3. Execute the single title use case
//! let use_case = container.rip_single_title_use_case();
//! let _ = use_case.execute(&request, None, None);
//! ```

pub mod api;
pub mod application;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod domain;
pub mod dvd;
pub mod ffmpeg;
#[cfg(feature = "gui")]
pub mod gui;
pub mod history;
pub mod imdb;
pub mod infrastructure;
pub mod iso;
pub mod mqtt;
pub mod ocr;
pub mod prelude;
pub mod queue;
pub mod utils;

use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::Args;
use crate::domain::ports::benchmark::DriveBenchmarkService;
use crate::domain::ports::dialogs::FileDialogService;
use crate::domain::ports::media_probe::ThumbnailExtractor;
use crate::domain::ports::metadata::{MetadataProvider, TvSeriesMetadataProvider};
use crate::domain::ports::notification::Notifier;
use crate::domain::ports::ocr::OcrProcessor;
use crate::domain::ports::optical_drive::{DiscInspector, DriveDetector, TrayController};
use crate::domain::ports::post_processor::PostProcessor;
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::Transcoder;
use crate::dvd::normalize_dvd_path;

/// Single Responsibility (SRP/SOLID): Encapsulates application startup and configuration bootstrapping.
///
/// # Examples
///
/// ```
/// use dvd_ripper::AppInitializer;
/// use dvd_ripper::cli::Args;
///
/// let initializer = AppInitializer;
/// let mut args = Args::default();
/// let _cfg = initializer.bootstrap(&mut args);
/// ```
///
/// # Panics
/// Never panics under normal operation.
#[derive(Debug, Default)]
pub struct AppInitializer;

impl AppInitializer {
    /// Bootstraps configuration from optional file and applies CLI overrides.
    ///
    /// # Panics
    /// Never panics.
    pub fn bootstrap(&self, args: &mut Args) -> config::AppConfig {
        let app_cfg = config::load_config(args.config.as_deref());
        config::apply_config_defaults(args, &app_cfg);
        app_cfg
    }
}

/// Dependency Inversion (DIP/SOLID): Composition Root service container encapsulating production dependencies.
///
/// Holds thread-safe reference counted pointers to all domain ports.
///
/// # Examples
///
/// ```no_run
/// use dvd_ripper::AppContainer;
///
/// let container = AppContainer::build_production("ffmpeg");
/// let probe_uc = container.probe_disc_use_case();
/// ```
pub struct AppContainer {
    pub transcoder: Arc<dyn Transcoder>,
    pub metadata_provider: Arc<dyn MetadataProvider>,
    pub tv_metadata_provider: Arc<dyn TvSeriesMetadataProvider>,
    pub drive_detector: Arc<dyn DriveDetector>,
    pub disc_inspector: Arc<dyn DiscInspector>,
    pub tray_controller: Arc<dyn TrayController>,
    pub drive_benchmark: Arc<dyn DriveBenchmarkService>,
    pub thumbnail_extractor: Arc<dyn ThumbnailExtractor>,
    pub ocr_processor: Arc<dyn OcrProcessor>,
    pub notifier: Arc<dyn Notifier>,
    pub post_processor: Arc<dyn PostProcessor>,
    pub history_repo: Arc<dyn HistoryRepository>,
    pub disk_guard: Arc<dyn DiskSpaceGuard>,
    pub dialog_service: Arc<dyn FileDialogService>,
}

impl AppContainer {
    /// Constructs a production `AppContainer` with real OS and FFmpeg adapters.
    ///
    /// # Parameters
    /// - `ffmpeg_bin`: Path or command name for the FFmpeg binary.
    ///
    /// # Panics
    /// Never panics.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use dvd_ripper::AppContainer;
    /// let container = AppContainer::build_production("ffmpeg");
    /// ```
    pub fn build_production(ffmpeg_bin: &str) -> Self {
        let tray_controller: Arc<dyn TrayController> =
            Arc::new(crate::infrastructure::optical::SystemTrayController::new());
        let post_processor = Arc::new(
            crate::infrastructure::post_processors::PostProcessingPipeline::build_production(Some(
                tray_controller.clone(),
            )),
        );

        Self {
            transcoder: Arc::new(crate::infrastructure::ffmpeg::FfmpegTranscoder::new(
                ffmpeg_bin,
            )),
            metadata_provider: Arc::new(
                crate::infrastructure::metadata::OmdbMetadataProvider::new(None),
            ),
            tv_metadata_provider: Arc::new(
                crate::infrastructure::metadata::TvmazeMetadataProvider::new(),
            ),
            drive_detector: Arc::new(crate::infrastructure::optical::SystemDriveDetector::new()),
            disc_inspector: Arc::new(crate::infrastructure::optical::SystemDiscInspector::new()),
            tray_controller,
            drive_benchmark: Arc::new(
                crate::infrastructure::optical::FfmpegDriveBenchmarkService::new(ffmpeg_bin),
            ),
            thumbnail_extractor: Arc::new(crate::infrastructure::ffmpeg::FfmpegMediaProber::new(
                ffmpeg_bin,
            )),
            ocr_processor: Arc::new(crate::infrastructure::ocr::TesseractOcrService::default()),
            notifier: Arc::new(
                crate::infrastructure::notifications::CompositeNotifier::new(vec![Box::new(
                    crate::infrastructure::notifications::MediaServerNotifier::new(),
                )]),
            ),
            post_processor,
            history_repo: Arc::new(crate::infrastructure::storage::JsonHistoryRepository::new()),
            disk_guard: Arc::new(crate::infrastructure::storage::SystemDiskSpaceGuard::new()),
            dialog_service: Arc::new(crate::infrastructure::system::NativeFileDialogService::new()),
        }
    }

    /// Constructs an `AppContainer` configured with settings from CLI `Args`.
    ///
    /// # Panics
    /// Never panics.
    pub fn build_from_args(args: &Args) -> Self {
        let mut notifiers: Vec<Box<dyn crate::domain::ports::notification::Notifier>> = Vec::new();

        if let Some(ref webhook) = args.webhook_url
            && !webhook.trim().is_empty()
        {
            notifiers.push(Box::new(
                crate::infrastructure::notifications::WebhookNotifier::new(
                    webhook.trim(),
                    args.webhook_secret.clone(),
                ),
            ));
        }

        if let Some(ref mqtt) = args.mqtt_broker
            && !mqtt.trim().is_empty()
        {
            notifiers.push(Box::new(
                crate::infrastructure::notifications::MqttNotifier::new(mqtt.trim(), "dvd-ripper"),
            ));
        }

        notifiers.push(Box::new(
            crate::infrastructure::notifications::MediaServerNotifier::new(),
        ));

        let notifier =
            Arc::new(crate::infrastructure::notifications::CompositeNotifier::new(notifiers));

        let tray_controller: Arc<dyn TrayController> =
            Arc::new(crate::infrastructure::optical::SystemTrayController::new());
        let post_processor = Arc::new(
            crate::infrastructure::post_processors::PostProcessingPipeline::build_production(Some(
                tray_controller.clone(),
            )),
        );

        let ocr_processor: Arc<dyn OcrProcessor> =
            Arc::new(crate::infrastructure::ocr::TesseractOcrService::default());

        Self {
            transcoder: Arc::new(
                crate::infrastructure::ffmpeg::FfmpegTranscoder::with_options(
                    &args.ffmpeg,
                    args.into(),
                ),
            ),
            metadata_provider: Arc::new(
                crate::infrastructure::metadata::OmdbMetadataProvider::new(None),
            ),
            tv_metadata_provider: Arc::new(
                crate::infrastructure::metadata::TvmazeMetadataProvider::new(),
            ),
            drive_detector: Arc::new(crate::infrastructure::optical::SystemDriveDetector::new()),
            disc_inspector: Arc::new(crate::infrastructure::optical::SystemDiscInspector::new()),
            tray_controller,
            drive_benchmark: Arc::new(
                crate::infrastructure::optical::FfmpegDriveBenchmarkService::new(&args.ffmpeg),
            ),
            thumbnail_extractor: Arc::new(crate::infrastructure::ffmpeg::FfmpegMediaProber::new(
                &args.ffmpeg,
            )),
            ocr_processor,
            notifier,
            post_processor,
            history_repo: Arc::new(crate::infrastructure::storage::JsonHistoryRepository::new()),
            disk_guard: Arc::new(crate::infrastructure::storage::SystemDiskSpaceGuard::new()),
            dialog_service: Arc::new(crate::infrastructure::system::NativeFileDialogService::new()),
        }
    }

    /// Creates a new [`RipSingleTitleUseCase`](application::rip_single_title::RipSingleTitleUseCase)
    /// using the container's registered services.
    pub fn rip_single_title_use_case(
        &self,
    ) -> application::rip_single_title::RipSingleTitleUseCase {
        application::rip_single_title::RipSingleTitleUseCase::new(
            self.transcoder.clone(),
            self.notifier.clone(),
            self.disk_guard.clone(),
            self.history_repo.clone(),
        )
    }

    /// Creates a new [`RipTvBatchUseCase`](application::rip_tv_batch::RipTvBatchUseCase)
    /// using the container's registered services.
    pub fn rip_tv_batch_use_case(&self) -> application::rip_tv_batch::RipTvBatchUseCase {
        application::rip_tv_batch::RipTvBatchUseCase::new(
            self.transcoder.clone(),
            self.notifier.clone(),
            self.disk_guard.clone(),
            self.history_repo.clone(),
        )
    }

    /// Creates a new [`ProbeDiscUseCase`](application::probe_disc::ProbeDiscUseCase)
    /// using the container's registered services.
    pub fn probe_disc_use_case(&self) -> application::probe_disc::ProbeDiscUseCase {
        application::probe_disc::ProbeDiscUseCase::new(Some(self.metadata_provider.clone()))
    }

    /// Creates a new [`DumpIsoUseCase`](application::dump_iso::DumpIsoUseCase)
    /// using the container's registered services.
    pub fn dump_iso_use_case(&self) -> application::dump_iso::DumpIsoUseCase {
        application::dump_iso::DumpIsoUseCase::new(
            self.notifier.clone(),
            self.disk_guard.clone(),
            self.history_repo.clone(),
        )
    }
}

/// Dependency Inversion (DIP/SOLID): High-level trait abstraction for DVD path normalization.
#[allow(dead_code)]
pub trait PathResolver {
    /// Normalizes a raw path string into a canonical filesystem `PathBuf`.
    fn resolve_path(&self, raw_path: &str) -> PathBuf;
}

/// Default implementation of [`PathResolver`] handling Windows drive letters and Unix mount points.
///
/// # Examples
///
/// ```
/// use dvd_ripper::{DefaultPathResolver, PathResolver};
///
/// let resolver = DefaultPathResolver;
/// let path = resolver.resolve_path("D:");
/// assert!(!path.as_os_str().is_empty());
/// ```
#[derive(Debug, Default)]
pub struct DefaultPathResolver;

impl PathResolver for DefaultPathResolver {
    fn resolve_path(&self, raw_path: &str) -> PathBuf {
        normalize_dvd_path(raw_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_container_build_production_initializes_all_ports() {
        let container = AppContainer::build_production("ffmpeg");
        assert!(Arc::strong_count(&container.transcoder) >= 1);
        assert!(Arc::strong_count(&container.metadata_provider) >= 1);
        assert!(Arc::strong_count(&container.tv_metadata_provider) >= 1);
        assert!(Arc::strong_count(&container.drive_detector) >= 1);
        assert!(Arc::strong_count(&container.disc_inspector) >= 1);
        assert!(Arc::strong_count(&container.tray_controller) >= 1);
        assert!(Arc::strong_count(&container.drive_benchmark) >= 1);
        assert!(Arc::strong_count(&container.thumbnail_extractor) >= 1);
        assert!(Arc::strong_count(&container.ocr_processor) >= 1);
        assert!(Arc::strong_count(&container.post_processor) >= 1);
        assert!(Arc::strong_count(&container.notifier) >= 1);
        assert!(Arc::strong_count(&container.history_repo) >= 1);
        assert!(Arc::strong_count(&container.disk_guard) >= 1);
        assert!(Arc::strong_count(&container.dialog_service) >= 1);
    }

    #[test]
    fn test_app_container_build_from_args() {
        let args = Args::default();
        let container = AppContainer::build_from_args(&args);
        let _single_uc = container.rip_single_title_use_case();
        let _tv_uc = container.rip_tv_batch_use_case();
        let _probe_uc = container.probe_disc_use_case();
        let _iso_uc = container.dump_iso_use_case();
    }
}
