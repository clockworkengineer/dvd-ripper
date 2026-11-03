//! Use case for executing batch television series disc ripping.

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
use crate::domain::entities::disc::TvEpisodeInfo;
use crate::domain::entities::media::FilmMetadata;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::{ProgressObserver, Transcoder};

/// Request parameter object for executing a batch TV season ripping operation.
#[derive(Debug, Clone)]
pub struct RipTvBatchRequest {
    pub source_dvd: PathBuf,
    pub series_title: String,
    pub season_number: u32,
    pub start_episode: u32,
    pub output_dir: PathBuf,
    pub episodes: Vec<TvEpisodeInfo>,
    pub metadata: Option<FilmMetadata>,
    pub min_free_gb: u64,
    pub file_extension: Option<String>,
    pub lookup_episode_titles: bool,
    pub track_boxset: bool,
}

impl RipTvBatchRequest {
    /// Creates a new fluent builder for constructing a [`RipTvBatchRequest`].
    ///
    /// # Parameters
    /// - `source_dvd`: Path to optical disc drive or mounted DVD folder.
    /// - `series_title`: Television show name (e.g. `"Doctor Who"`).
    /// - `output_dir`: Output directory path where episodes will be organized.
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::application::RipTvBatchRequest;
    ///
    /// let request = RipTvBatchRequest::builder("D:\\", "Doctor Who", "C:\\Videos\\TV")
    ///     .season(1)
    ///     .start_episode(1)
    ///     .min_free_gb(20)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(request.series_title, "Doctor Who");
    /// assert_eq!(request.season_number, 1);
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn builder(
        source_dvd: impl Into<PathBuf>,
        series_title: impl Into<String>,
        output_dir: impl Into<PathBuf>,
    ) -> RipTvBatchRequestBuilder {
        RipTvBatchRequestBuilder::new(source_dvd, series_title, output_dir)
    }

    pub fn new(
        source_dvd: PathBuf,
        series_title: String,
        season_number: u32,
        start_episode: u32,
        output_dir: PathBuf,
        episodes: Vec<TvEpisodeInfo>,
        min_free_gb: u64,
    ) -> Self {
        Self {
            source_dvd,
            series_title,
            season_number,
            start_episode,
            output_dir,
            episodes,
            metadata: None,
            min_free_gb,
            file_extension: None,
            lookup_episode_titles: false,
            track_boxset: false,
        }
    }
}

/// Fluent builder for constructing [`RipTvBatchRequest`] instances.
#[derive(Debug, Clone)]
pub struct RipTvBatchRequestBuilder {
    source_dvd: PathBuf,
    series_title: String,
    season_number: u32,
    start_episode: u32,
    output_dir: PathBuf,
    episodes: Vec<TvEpisodeInfo>,
    metadata: Option<FilmMetadata>,
    min_free_gb: u64,
    file_extension: Option<String>,
    lookup_episode_titles: bool,
    track_boxset: bool,
}

impl RipTvBatchRequestBuilder {
    pub fn new(
        source_dvd: impl Into<PathBuf>,
        series_title: impl Into<String>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_dvd: source_dvd.into(),
            series_title: series_title.into(),
            season_number: 1,
            start_episode: 1,
            output_dir: output_dir.into(),
            episodes: Vec::new(),
            metadata: None,
            min_free_gb: 10,
            file_extension: None,
            lookup_episode_titles: false,
            track_boxset: false,
        }
    }

    pub fn season(mut self, season: u32) -> Self {
        self.season_number = season;
        self
    }

    pub fn start_episode(mut self, episode: u32) -> Self {
        self.start_episode = episode;
        self
    }

    pub fn episodes(mut self, episodes: Vec<TvEpisodeInfo>) -> Self {
        self.episodes = episodes;
        self
    }

    pub fn metadata(mut self, meta: FilmMetadata) -> Self {
        self.metadata = Some(meta);
        self
    }

    pub fn min_free_gb(mut self, gb: u64) -> Self {
        self.min_free_gb = gb;
        self
    }

    pub fn file_extension(mut self, ext: impl Into<String>) -> Self {
        self.file_extension = Some(ext.into());
        self
    }

    pub fn lookup_episode_titles(mut self, enable: bool) -> Self {
        self.lookup_episode_titles = enable;
        self
    }

    pub fn track_boxset(mut self, enable: bool) -> Self {
        self.track_boxset = enable;
        self
    }

    pub fn build(self) -> Result<RipTvBatchRequest> {
        Ok(RipTvBatchRequest {
            source_dvd: self.source_dvd,
            series_title: self.series_title,
            season_number: self.season_number,
            start_episode: self.start_episode,
            output_dir: self.output_dir,
            episodes: self.episodes,
            metadata: self.metadata,
            min_free_gb: self.min_free_gb,
            file_extension: self.file_extension,
            lookup_episode_titles: self.lookup_episode_titles,
            track_boxset: self.track_boxset,
        })
    }
}

/// Use case that orchestrates batch extraction of multiple TV episodes from an optical disc:
/// 1. Queries TVMaze episode metadata titles when enabled.
/// 2. Iterates through detected TV titles with live progress observation.
/// 3. Propagates cooperative cancellation tokens across episode transitions.
/// 4. Synchronizes cumulative episode counters in persistent box set tracking (`boxsets.json`).
pub struct RipTvBatchUseCase {
    single_title_use_case: RipSingleTitleUseCase,
    notifier: Arc<dyn Notifier>,
}

impl RipTvBatchUseCase {
    pub fn new(
        transcoder: Arc<dyn Transcoder>,
        notifier: Arc<dyn Notifier>,
        disk_guard: Arc<dyn DiskSpaceGuard>,
        history_repo: Arc<dyn HistoryRepository>,
    ) -> Self {
        Self {
            single_title_use_case: RipSingleTitleUseCase::new(
                transcoder,
                notifier.clone(),
                disk_guard,
                history_repo,
            ),
            notifier,
        }
    }

    /// Executes batch ripping of all configured TV series episodes.
    ///
    /// # Parameters
    /// - `request`: Specification for series title, season, episodes, and directory.
    /// - `observer`: Optional progress observer receiving transcode progress.
    /// - `cancel_flag`: Optional cooperative cancellation token.
    ///
    /// # Returns
    /// Number of successfully transcoded episodes.
    ///
    /// # Errors
    /// Returns an error if disk space is exhausted or transcoding fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn execute(
        &self,
        request: &RipTvBatchRequest,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<usize> {
        let mut completed_count = 0;
        let ext = request.file_extension.as_deref().unwrap_or("mp4");

        let tvmaze_episodes = if request.lookup_episode_titles {
            crate::infrastructure::metadata::fetch_tvmaze_episodes(&request.series_title)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        for ep in &request.episodes {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = self.notifier.send(&NotificationEvent::RipCancelled {
                    title: &request.series_title,
                });
                return Err(anyhow!("TV Batch ripping cancelled by user."));
            }

            let ep_title = if !tvmaze_episodes.is_empty() {
                crate::infrastructure::metadata::find_episode_title(
                    &tvmaze_episodes,
                    request.season_number,
                    ep.episode_num,
                )
            } else {
                None
            };

            let display_title = crate::utils::format_episode_name_with_title(
                &request.series_title,
                request.season_number,
                ep.episode_num,
                ep_title.as_deref(),
            );

            let ep_filename = format!("{}.{}", display_title, ext);
            let ep_output_path = request.output_dir.join(&ep_filename);

            let single_req = RipSingleTitleRequest {
                source_dvd: request.source_dvd.clone(),
                title_number: ep.title_num,
                output_path: ep_output_path,
                display_title,
                expected_duration_secs: Some(ep.duration_secs),
                metadata: request.metadata.clone(),
                min_free_gb: request.min_free_gb,
            };

            self.single_title_use_case
                .execute(&single_req, observer, cancel_flag.clone())?;
            completed_count += 1;
        }

        if request.track_boxset && completed_count > 0 {
            crate::queue::record_boxset_episodes_ripped(
                &request.series_title,
                request.season_number,
                completed_count as u32,
            );
        }

        Ok(completed_count)
    }
}
