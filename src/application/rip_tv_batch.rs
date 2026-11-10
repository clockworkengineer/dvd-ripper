//! Use case for executing batch television series disc ripping.

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
use crate::domain::entities::disc::TvEpisodeInfo;
use crate::domain::entities::media::FilmMetadata;
use crate::domain::ports::media_probe::StreamProber;
use crate::domain::ports::metadata::TvSeriesMetadataProvider;
use crate::domain::ports::notification::{NotificationEvent, Notifier};
use crate::domain::ports::queue::BoxSetRepository;
use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use crate::domain::ports::transcoder::{ProgressObserver, Transcoder};

/// Request parameter object for executing a batch TV season ripping operation.
#[derive(Debug, Clone)]
pub struct RipTvBatchRequest {
    pub source_dvd: PathBuf,
    pub series_title: String,
    pub season_number: u32,
    pub start_episode: u32,
    pub end_episode: Option<u32>,
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
            end_episode: None,
            output_dir,
            episodes,
            metadata: None,
            min_free_gb,
            file_extension: None,
            lookup_episode_titles: false,
            track_boxset: false,
        }
    }

    /// Resolves the concrete season output directory for this batch request:
    /// `<TV_ROOT>/<Show Name (Year)>/Season <XX>/`
    pub fn resolve_destination_dir(&self) -> PathBuf {
        resolve_tv_batch_destination_dir(
            &self.output_dir,
            &self.series_title,
            self.season_number,
            self.metadata.as_ref().and_then(|m| m.year),
        )
    }
}

/// Resolves the destination directory for TV series episodes.
///
/// Ensures the folder path adheres to the standard Plex/Jellyfin convention:
/// `<TV_ROOT>/<Show Name (Year)>/Season <XX>/`
///
/// - If `output_dir` already terminates in `Season XX`, it is retained as-is to avoid duplicate folders.
/// - If `output_dir` terminates in the show folder, `Season <XX>` is appended.
/// - Otherwise, `resolve_tv_root` is used, and `<Show Name (Year)>/Season <XX>/` is constructed.
pub fn resolve_tv_batch_destination_dir(
    output_dir: &std::path::Path,
    series_title: &str,
    season_number: u32,
    series_year: Option<u32>,
) -> PathBuf {
    let clean_show = crate::utils::sanitize_filename(series_title);
    let effective_show = if clean_show.is_empty() {
        "TV Show".to_string()
    } else {
        clean_show
    };
    let show_folder = crate::utils::format_title_folder_name(&effective_show, series_year);
    let season_folder = format!("Season {:02}", season_number);

    let trimmed = output_dir.as_os_str().to_string_lossy();
    let trimmed_str = trimmed.trim_end_matches(['/', '\\']);
    let path = std::path::Path::new(trimmed_str);

    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
        if file_name.eq_ignore_ascii_case(&season_folder)
            || (file_name.to_ascii_lowercase().starts_with("season")
                && file_name.trim_start_matches(|c: char| !c.is_ascii_digit())
                    == format!("{}", season_number))
        {
            return path.to_path_buf();
        }

        if file_name.eq_ignore_ascii_case(&show_folder)
            || file_name.eq_ignore_ascii_case(&effective_show)
        {
            return path.join(&season_folder);
        }
    }

    let tv_root = crate::ffmpeg::resolve_tv_root(trimmed_str);
    tv_root.join(show_folder).join(season_folder)
}

/// Fluent builder for constructing [`RipTvBatchRequest`] instances.
#[derive(Debug, Clone)]
pub struct RipTvBatchRequestBuilder {
    source_dvd: PathBuf,
    series_title: String,
    season_number: u32,
    start_episode: u32,
    end_episode: Option<u32>,
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
            end_episode: None,
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

    pub fn end_episode(mut self, episode: u32) -> Self {
        self.end_episode = Some(episode);
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
            end_episode: self.end_episode,
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
    tv_metadata_provider: Option<Arc<dyn TvSeriesMetadataProvider>>,
    stream_prober: Option<Arc<dyn StreamProber>>,
    boxset_repo: Option<Arc<dyn BoxSetRepository>>,
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
            tv_metadata_provider: None,
            stream_prober: None,
            boxset_repo: None,
        }
    }

    /// Injects domain ports for TV episode catalog lookup, chapter stream probing, and box-set tracking (DIP).
    pub fn with_ports(
        mut self,
        tv_metadata_provider: Arc<dyn TvSeriesMetadataProvider>,
        stream_prober: Arc<dyn StreamProber>,
        boxset_repo: Arc<dyn BoxSetRepository>,
    ) -> Self {
        self.tv_metadata_provider = Some(tv_metadata_provider);
        self.stream_prober = Some(stream_prober);
        self.boxset_repo = Some(boxset_repo);
        self
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

        let default_tv_provider = crate::infrastructure::metadata::TvmazeMetadataProvider::new();
        let tv_provider: &dyn TvSeriesMetadataProvider = self
            .tv_metadata_provider
            .as_deref()
            .unwrap_or(&default_tv_provider);

        let catalog_episodes = if request.lookup_episode_titles {
            tv_provider
                .fetch_episodes(&request.series_title)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let episodes_to_rip: Vec<_> = if let Some(end_ep) = request.end_episode {
            request
                .episodes
                .iter()
                .filter(|ep| ep.episode_num <= end_ep && ep.episode_num >= request.start_episode)
                .collect()
        } else {
            request.episodes.iter().collect()
        };

        let destination_dir = request.resolve_destination_dir();

        // Detect if all episodes share the same monolithic "Play-All" title number and need chapter-range bounds
        let is_monolithic_play_all = episodes_to_rip.len() > 1
            && episodes_to_rip.first().is_some_and(|first| {
                episodes_to_rip
                    .iter()
                    .all(|e| e.title_num == first.title_num)
            });
        let play_all_spans = if is_monolithic_play_all {
            let play_all_title = episodes_to_rip[0].title_num.max(1);
            let default_prober = crate::infrastructure::ffmpeg::FfmpegMediaProber::new("ffmpeg");
            let prober: &dyn StreamProber = self.stream_prober.as_deref().unwrap_or(&default_prober);
            let details = prober.probe_stream_details(&request.source_dvd, play_all_title);
            crate::domain::heuristics::detect_tv_episodes_from_chapters(
                play_all_title,
                &details.chapters,
                &request.series_title,
                request.season_number,
                request.start_episode,
                None,
            )
        } else {
            Vec::new()
        };

        for (idx, ep) in episodes_to_rip.into_iter().enumerate() {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = self.notifier.send(&NotificationEvent::RipCancelled {
                    title: &request.series_title,
                });
                return Err(anyhow!("TV Batch ripping cancelled by user."));
            }

            if let Some(obs) = observer {
                obs.on_title_start(ep.title_num);
            }

            let ep_title = catalog_episodes
                .iter()
                .find(|e| e.season == request.season_number && e.episode_num == ep.episode_num)
                .map(|e| e.title.clone());

            let display_title = crate::utils::format_episode_name_with_title(
                &request.series_title,
                request.season_number,
                ep.episode_num,
                ep_title.as_deref(),
            );

            let ep_filename = format!("{}.{}", display_title, ext);
            let ep_output_path = destination_dir.join(&ep_filename);

            let single_req = RipSingleTitleRequest {
                source_dvd: request.source_dvd.clone(),
                title_number: ep.title_num,
                output_path: ep_output_path,
                display_title,
                expected_duration_secs: Some(ep.duration_secs),
                metadata: request.metadata.clone(),
                min_free_gb: request.min_free_gb,
            };

            let chapter_range = play_all_spans
                .iter()
                .find(|s| s.episode.episode_num == ep.episode_num)
                .or_else(|| play_all_spans.get(idx))
                .map(|s| (s.start_chapter, s.end_chapter));

            self.single_title_use_case.execute_with_chapter_range(
                &single_req,
                chapter_range,
                observer,
                cancel_flag.clone(),
            )?;
            completed_count += 1;
        }

        if request.track_boxset && completed_count > 0 {
            let default_boxset = crate::queue::FileBoxSetEpisodeTracker;
            let boxset_repo: &dyn BoxSetRepository =
                self.boxset_repo.as_deref().unwrap_or(&default_boxset);
            boxset_repo.record_episodes_ripped(
                &request.series_title,
                request.season_number,
                completed_count as u32,
            );
        }

        Ok(completed_count)
    }
}
