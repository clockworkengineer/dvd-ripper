//! Single Responsibility (SRP/SOLID): GUI task controller managing background worker threads and task orchestration.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::AppContainer;
use crate::cli::{Args, EncodingOptions};
use crate::domain::entities::disc::TvEpisodeInfo;
use crate::domain::entities::job::RipRecord;
use crate::domain::ports::post_processor::PostProcessContext;
use crate::ffmpeg::{
    ProgressEvent, detect_tv_episodes, resolve_output_path, resolve_tv_output_path_with_title,
};

/// Specification parameters required to start a background ripping process.
#[derive(Debug, Clone)]
pub struct RipTaskSpecification {
    pub drive: String,
    pub out_dir: String,
    pub is_iso: bool,
    pub is_tv: bool,
    pub all_episodes: bool,
    pub transcode: bool,
    pub film_name: Option<String>,
    pub film_year: Option<u32>,
    pub season: u32,
    pub start_episode: u32,
    pub title_number: u32,
    pub preset: String,
    pub engine: String,
    pub handbrake_path: String,
    pub makemkv_path: String,
    pub ffmpeg_path: String,
    pub expected_runtime_secs: Option<f64>,
    pub detected_episodes: Vec<TvEpisodeInfo>,
    pub poster_bytes: Option<Vec<u8>>,
    pub encoding_options: EncodingOptions,
}

/// Controller managing background worker threads and asynchronous progress dispatch for the desktop UI.
pub struct GuiApplicationController {
    container: Arc<AppContainer>,
    event_tx: Sender<ProgressEvent>,
    event_rx: Receiver<ProgressEvent>,
    cancel_flag: Option<Arc<AtomicBool>>,
    cancel_tx: Option<Sender<()>>,
}

impl GuiApplicationController {
    /// Creates a new `GuiApplicationController` wrapping the composition root container.
    pub fn new(container: Arc<AppContainer>) -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            container,
            event_tx,
            event_rx,
            cancel_flag: None,
            cancel_tx: None,
        }
    }

    /// Returns a shared reference to the inner `AppContainer`.
    pub fn container(&self) -> &Arc<AppContainer> {
        &self.container
    }

    /// Returns a cloned sender handle for dispatching progress events.
    pub fn event_tx(&self) -> Sender<ProgressEvent> {
        self.event_tx.clone()
    }

    /// Polls for the next pending event from the background thread channel.
    pub fn poll_event(&self) -> Option<ProgressEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Requests cancellation of any actively executing background job.
    pub fn cancel(&mut self) {
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(tx) = &self.cancel_tx {
            let _ = tx.send(());
        }
        let _ = self.event_tx.send(ProgressEvent::Error(
            "Ripping process cancelled by user.".to_string(),
        ));
    }

    /// Opens an OS directory picker dialog.
    pub fn pick_folder(&self, starting_dir: Option<&Path>, title: &str) -> Option<PathBuf> {
        self.container
            .dialog_service
            .pick_folder(starting_dir, title)
    }

    /// Opens the specified folder in the OS desktop file explorer.
    pub fn open_folder(&self, directory: &Path) {
        self.container.dialog_service.open_in_explorer(directory);
    }

    /// Loads history records via the container's history repository.
    pub fn load_history(&self, custom_path: Option<&Path>) -> Vec<RipRecord> {
        self.container.history_repo.load_history(custom_path)
    }

    /// Clears rip history records.
    pub fn clear_history(&self, custom_path: Option<&Path>) -> Result<()> {
        self.container.history_repo.clear_history(custom_path)
    }

    /// Launches a background worker to detect disc titles, volume labels, and film metadata.
    pub fn start_detection(&self, drive: &str, query_override: Option<String>) {
        let drive = drive.to_string();
        let tx = self.event_tx.clone();
        let container = self.container.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&drive);
            let (best_title, best_dur) = crate::ffmpeg::detect_best_title_info("", &dvd_path, None);
            if best_title > 0 {
                let _ = tx.send(ProgressEvent::DetectedTitle {
                    title_num: best_title,
                    duration_secs: best_dur.unwrap_or(0.0),
                });
                let stream_details = container
                    .stream_prober
                    .probe_stream_details(&dvd_path, best_title);
                let _ = tx.send(ProgressEvent::TitleStreamsProbed(stream_details));

                let temp_thumb = crate::infrastructure::storage::TempResource::new(
                    &format!("dvd_thumb_title_{}", best_title),
                    "jpg",
                );
                if container
                    .thumbnail_extractor
                    .extract_thumbnail(&dvd_path, best_title, 120.0, temp_thumb.path())
                    .is_ok()
                    && let Ok(bytes) = std::fs::read(temp_thumb.path())
                {
                    let _ = tx.send(ProgressEvent::TitleThumbnailReady {
                        title_num: best_title,
                        image_bytes: bytes,
                    });
                }
            }

            let volume_label = container.disc_inspector.read_volume_label(&dvd_path);
            if query_override.is_none() && volume_label.is_some() {
                let fp = container.disc_inspector.compute_fingerprint(&dvd_path);
                if let Some(cached_meta) = container.fingerprint_repo.lookup_fingerprint(&fp) {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "Instant offline fingerprint match: {}",
                        cached_meta.title
                    )));
                    let _ = tx.send(ProgressEvent::Metadata(cached_meta));
                    return;
                }
            }

            let search_term = query_override.or_else(|| {
                volume_label.as_ref().map(|l| {
                    crate::utils::normalize_volume_label_title(&crate::dvd::clean_volume_label(l))
                })
            });

            if let Some(term) = search_term {
                let _ = tx.send(ProgressEvent::Log(format!(
                    "Searching metadata for '{}'...",
                    term
                )));
                let candidates = crate::imdb::fetch_search_candidates(&term);
                if candidates.len() > 1 {
                    let _ = tx.send(ProgressEvent::SearchResults(candidates));
                } else if let Ok(meta) = crate::imdb::lookup_film_details(&term) {
                    let fp = container.disc_inspector.compute_fingerprint(&dvd_path);
                    let cache = crate::infrastructure::metadata::DiskFingerprintCache::new();
                    let entry = crate::infrastructure::metadata::CachedDiscEntry {
                        fingerprint: fp,
                        volume_label,
                        metadata: meta.clone(),
                        title_number: if best_title > 0 {
                            Some(best_title)
                        } else {
                            None
                        },
                        is_tv: false,
                        season: None,
                        cached_at: chrono::Local::now().to_rfc3339(),
                    };
                    let _ = cache.save_entry(&entry);
                    let _ = tx.send(ProgressEvent::Metadata(meta));
                } else {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "No metadata found for '{}'.",
                        term
                    )));
                    let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
                }
            } else {
                let _ = tx.send(ProgressEvent::Log(
                    "No DVD disc detected or volume label is empty. Insert a disc or enter title manually.".to_string(),
                ));
                let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
            }
        });
    }

    /// Queries OMDb/IMDb asynchronously for a specific IMDb title ID.
    pub fn start_search_lookup(&self, imdb_id: &str) {
        let id = imdb_id.to_string();
        let tx = self.event_tx.clone();

        thread::spawn(move || {
            if let Some(meta) = crate::imdb::lookup_omdb_by_id(&id) {
                let _ = tx.send(ProgressEvent::Metadata(meta));
            } else {
                let _ = tx.send(ProgressEvent::Log(
                    "Failed to load details for selected title.".to_string(),
                ));
                let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
            }
        });
    }

    /// Scans the disc asynchronously for TV episode title tracks.
    pub fn start_tv_scan(&self, drive: &str, show_name: &str, season: u32, start_ep: u32) {
        let drive = drive.to_string();
        let show_name = show_name.to_string();
        let tx = self.event_tx.clone();
        let container = self.container.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&drive);
            let mut episodes =
                detect_tv_episodes("ffmpeg", &dvd_path, &show_name, season, start_ep, None);

            // Fallback for monolithic "Play All" TV DVDs where all episodes are authored as chapters inside Title #1
            if episodes.len() <= 1 {
                let target_title = episodes.first().map(|e| e.title_num).unwrap_or(1);
                let details = container
                    .stream_prober
                    .probe_stream_details(&dvd_path, target_title);
                if details.chapters.len() >= 2 && details.duration_secs >= 2400.0 {
                    let spans = crate::domain::heuristics::detect_tv_episodes_from_chapters(
                        target_title,
                        &details.chapters,
                        &show_name,
                        season,
                        start_ep,
                        None,
                    );
                    if spans.len() > episodes.len() {
                        episodes = spans.into_iter().map(|s| s.episode).collect();
                    }
                }
            }

            if let Ok(tvmaze_eps) = container.tv_metadata_provider.fetch_episodes(&show_name) {
                for ep in &mut episodes {
                    if let Some(online_ep) = tvmaze_eps
                        .iter()
                        .find(|e| e.season == season && e.episode_num == ep.episode_num)
                    {
                        ep.formatted_name = format!(
                            "{} - S{:02}E{:02} - {}",
                            show_name, season, ep.episode_num, online_ep.title
                        );
                    }
                }
            }

            let _ = tx.send(ProgressEvent::TvEpisodesDetected(episodes));
        });
    }

    /// Extracts a preview thumbnail frame from the disc title asynchronously.
    pub fn start_thumbnail_preview(&self, drive: &str, title_num: u32) {
        let drive = drive.to_string();
        let tx = self.event_tx.clone();
        let container = self.container.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&drive);
            let temp_thumb = crate::infrastructure::storage::TempResource::new(
                &format!("dvd_preview_thumb_title_{}", title_num),
                "jpg",
            );
            if container
                .thumbnail_extractor
                .extract_thumbnail(&dvd_path, title_num, 120.0, temp_thumb.path())
                .is_ok()
                && let Ok(bytes) = std::fs::read(temp_thumb.path())
            {
                let _ = tx.send(ProgressEvent::TitleThumbnailReady {
                    title_num,
                    image_bytes: bytes,
                });
            } else {
                let _ = tx.send(ProgressEvent::Log(format!(
                    "Thumbnail snapshot extraction failed for Title #{}",
                    title_num
                )));
            }
        });
    }

    /// Generates a 10-second live encoded video preview clip asynchronously.
    pub fn start_preview_clip(&self, drive: &str, title_num: u32, args: Args) {
        let drive = drive.to_string();
        let tx = self.event_tx.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&drive);
            let out_preview = std::env::temp_dir()
                .join(format!("dvd_ripper_preview_title_{}.mp4", title_num.max(1)));
            let _ = tx.send(ProgressEvent::Log(format!(
                "Generating 10-second live preview sample for Title #{}...",
                title_num.max(1)
            )));
            match crate::ffmpeg::generate_preview_clip(
                &args,
                &dvd_path,
                title_num.max(1),
                120.0,
                &out_preview,
            ) {
                Ok(path) => {
                    let _ = tx.send(ProgressEvent::PreviewClipReady(path));
                }
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "Preview clip generation failed: {}",
                        e
                    )));
                }
            }
        });
    }

    /// Runs a 10-second optical drive throughput benchmark asynchronously.
    pub fn start_benchmark(&self, drive: &str) {
        let drive = drive.to_string();
        let tx = self.event_tx.clone();
        let container = self.container.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&drive);
            let _ = tx.send(ProgressEvent::Log(format!(
                "Starting 10-second optical benchmark on '{}'...",
                dvd_path.display()
            )));
            match container.drive_benchmark.benchmark(&dvd_path, 10) {
                Ok(report) => {
                    let _ = tx.send(ProgressEvent::BenchmarkFinished(report));
                }
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Log(format!("Benchmark failed: {}", e)));
                    let _ = tx.send(ProgressEvent::Log("BENCH_FAIL".to_string()));
                }
            }
        });
    }

    /// Orchestrates and spawns the background ripping process based on the provided specification.
    pub fn start_ripping(&mut self, spec: RipTaskSpecification) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());

        let (cancel_tx, _cancel_rx) = channel();
        self.cancel_tx = Some(cancel_tx);

        let tx = self.event_tx.clone();
        let container = self.container.clone();

        thread::spawn(move || {
            let dvd_path = crate::dvd::normalize_dvd_path(&spec.drive);
            let mut base_args = Args {
                input: spec.drive.clone(),
                out_dir: spec.out_dir.clone(),
                ffmpeg: spec.ffmpeg_path.clone(),
                handbrake: spec.handbrake_path.clone(),
                makemkv: spec.makemkv_path.clone(),
                engine: spec.engine.clone(),
                transcode: spec.transcode,
                preset: spec.preset.clone(),
                ..Default::default()
            };
            base_args.apply_encoding_options(spec.encoding_options.clone());

            let trans_options: crate::domain::ports::TranscodeOptions = (&base_args).into();
            let ffmpeg_transcoder = Arc::new(
                crate::infrastructure::ffmpeg::FfmpegTranscoder::with_args(
                    &spec.ffmpeg_path,
                    base_args.clone(),
                ),
            );
            let native_transcoder = Arc::new(
                crate::infrastructure::engines::NativeIfoTranscoder::new(
                    &spec.ffmpeg_path,
                    trans_options.clone(),
                ),
            );

            let task_transcoder: Arc<dyn crate::domain::ports::transcoder::Transcoder> =
                match spec.engine.to_lowercase().as_str() {
                    "handbrake" => Arc::new(crate::infrastructure::engines::HandBrakeTranscoder::new(
                        &spec.handbrake_path,
                        trans_options,
                    )),
                    "makemkv" => Arc::new(crate::infrastructure::engines::MakeMkvTranscoder::new(
                        &spec.makemkv_path,
                    )),
                    "native" => native_transcoder,
                    "ffmpeg" => ffmpeg_transcoder,
                    _ => {
                        // "auto": Smart fallback across engines
                        let mut engines: Vec<(&'static str, Arc<dyn crate::domain::ports::transcoder::Transcoder>)> = vec![
                            ("ffmpeg", ffmpeg_transcoder),
                            ("native_ifo", native_transcoder),
                        ];
                        if crate::infrastructure::engines::HandBrakeTranscoder::is_available(&spec.handbrake_path) {
                            engines.push((
                                "handbrake",
                                Arc::new(crate::infrastructure::engines::HandBrakeTranscoder::new(
                                    &spec.handbrake_path,
                                    trans_options,
                                )),
                            ));
                        }
                        if crate::infrastructure::engines::MakeMkvTranscoder::is_available(&spec.makemkv_path) {
                            engines.push((
                                "makemkv",
                                Arc::new(crate::infrastructure::engines::MakeMkvTranscoder::new(
                                    &spec.makemkv_path,
                                )),
                            ));
                        }
                        Arc::new(crate::infrastructure::engines::AutoFallbackTranscoder::new(engines))
                    }
                };
            let single_title_uc = crate::application::rip_single_title::RipSingleTitleUseCase::new(
                task_transcoder.clone(),
                container.notifier.clone(),
                container.disk_guard.clone(),
                container.history_repo.clone(),
            );
            let tv_batch_uc = crate::application::rip_tv_batch::RipTvBatchUseCase::new(
                task_transcoder,
                container.notifier.clone(),
                container.disk_guard.clone(),
                container.history_repo.clone(),
            )
            .with_ports(
                container.tv_metadata_provider.clone(),
                container.stream_prober.clone(),
                container.boxset_repo.clone(),
            );

            let mut rip_history_records: Vec<(String, String, PathBuf, f64)> = Vec::new();
            let mut last_output_file: Option<PathBuf> = None;

            if spec.is_iso {
                let _ = tx.send(ProgressEvent::Log(
                    "Starting bit-for-bit ISO Disc Image Backup...".to_string(),
                ));
                let absolute_output = match resolve_output_path(
                    &base_args,
                    spec.film_name.as_deref(),
                    spec.film_year,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        return;
                    }
                };

                let observer = super::state::GuiProgressObserver { tx: tx.clone() };
                let mut req_builder = crate::application::dump_iso::DumpIsoRequest::builder(
                    &dvd_path,
                    &absolute_output,
                )
                .min_free_gb(spec.encoding_options.min_free_gb);
                if let Some(ref title) = spec.film_name {
                    req_builder = req_builder.display_title(title);
                }
                let request = match req_builder.build() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        return;
                    }
                };

                match container.dump_iso_use_case().execute(
                    &request,
                    Some(&observer),
                    Some(cancel_flag.clone()),
                ) {
                    Ok(_) => {
                        let _ = tx.send(ProgressEvent::Log(
                            "ISO dump completed successfully!".to_string(),
                        ));
                        let title = spec
                            .film_name
                            .clone()
                            .unwrap_or_else(|| "DVD ISO Backup".to_string());
                        rip_history_records.push((
                            title,
                            "ISO Backup".to_string(),
                            absolute_output.clone(),
                            0.0,
                        ));
                        last_output_file = Some(absolute_output);
                    }
                    Err(e) => {
                        if cancel_flag.load(Ordering::SeqCst) {
                            let _ = tx.send(ProgressEvent::Error(
                                "Ripping process cancelled by user.".to_string(),
                            ));
                        } else {
                            let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        }
                        return;
                    }
                }
            } else if spec.is_tv {
                let show_name = spec.film_name.as_deref().unwrap_or("TV Show");
                if spec.all_episodes {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "Batch ripping {} TV episodes for '{}'...",
                        spec.detected_episodes.len(),
                        show_name
                    )));
                    let ext = if spec.encoding_options.mkv {
                        "mkv"
                    } else {
                        "mp4"
                    };
                    let mut req_builder =
                        crate::application::rip_tv_batch::RipTvBatchRequest::builder(
                            &dvd_path,
                            show_name,
                            &spec.out_dir,
                        )
                        .season(spec.season)
                        .start_episode(spec.start_episode)
                        .episodes(spec.detected_episodes.clone())
                        .min_free_gb(spec.encoding_options.min_free_gb)
                        .file_extension(ext)
                        .lookup_episode_titles(true)
                        .track_boxset(base_args.auto_boxset);

                    if let Some(year) = spec.film_year {
                        req_builder = req_builder.metadata(crate::domain::FilmMetadata {
                            title: show_name.to_string(),
                            year: Some(year),
                            is_series: true,
                            ..Default::default()
                        });
                    }

                    let request = match req_builder.build() {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = tx.send(ProgressEvent::Error(e.to_string()));
                            return;
                        }
                    };

                    let observer = super::state::GuiProgressObserver { tx: tx.clone() };
                    match tv_batch_uc.execute(
                        &request,
                        Some(&observer),
                        Some(cancel_flag.clone()),
                    ) {
                        Ok(count) => {
                            let _ = tx.send(ProgressEvent::Log(format!(
                                "Batch rip complete: {} episodes ripped.",
                                count
                            )));
                            for ep in &spec.detected_episodes {
                                if let Ok(ep_path) = resolve_tv_output_path_with_title(
                                    &base_args,
                                    Some(show_name),
                                    spec.film_year,
                                    spec.season,
                                    ep.episode_num,
                                    None,
                                ) {
                                    rip_history_records.push((
                                        ep.formatted_name.clone(),
                                        "TV Series".to_string(),
                                        ep_path.clone(),
                                        ep.duration_secs,
                                    ));
                                    last_output_file = Some(ep_path);
                                }
                            }
                        }
                        Err(e) => {
                            if cancel_flag.load(Ordering::SeqCst) {
                                let _ = tx.send(ProgressEvent::Error(
                                    "Ripping process cancelled by user.".to_string(),
                                ));
                            } else {
                                let _ = tx.send(ProgressEvent::Error(e.to_string()));
                            }
                            return;
                        }
                    }
                } else {
                    let ep_num = if spec.start_episode > 0 {
                        spec.start_episode
                    } else {
                        1
                    };
                    let ep_title = container
                        .tv_metadata_provider
                        .find_episode_title(show_name, spec.season, ep_num)
                        .ok()
                        .flatten();
                    let absolute_output = match resolve_tv_output_path_with_title(
                        &base_args,
                        Some(show_name),
                        spec.film_year,
                        spec.season,
                        ep_num,
                        ep_title.as_deref(),
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = tx.send(ProgressEvent::Error(e.to_string()));
                            return;
                        }
                    };

                    let formatted_title = crate::utils::format_episode_name_with_title(
                        show_name,
                        spec.season,
                        ep_num,
                        ep_title.as_deref(),
                    );
                    let mut req_builder =
                        crate::application::rip_single_title::RipSingleTitleRequest::builder(
                            &dvd_path,
                            if spec.title_number == 0 {
                                1
                            } else {
                                spec.title_number
                            },
                            &absolute_output,
                        )
                        .display_title(&formatted_title)
                        .min_free_gb(spec.encoding_options.min_free_gb);
                    if let Some(dur) = spec.expected_runtime_secs {
                        req_builder = req_builder.expected_duration(dur);
                    }
                    let request = match req_builder.build() {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = tx.send(ProgressEvent::Error(e.to_string()));
                            return;
                        }
                    };

                    let observer = super::state::GuiProgressObserver { tx: tx.clone() };
                    match single_title_uc.execute(
                        &request,
                        Some(&observer),
                        Some(cancel_flag.clone()),
                    ) {
                        Ok(_) => {
                            rip_history_records.push((
                                formatted_title,
                                "TV Series".to_string(),
                                absolute_output.clone(),
                                spec.expected_runtime_secs.unwrap_or(0.0),
                            ));
                            last_output_file = Some(absolute_output);
                        }
                        Err(e) => {
                            if cancel_flag.load(Ordering::SeqCst) {
                                let _ = tx.send(ProgressEvent::Error(
                                    "Ripping process cancelled by user.".to_string(),
                                ));
                            } else {
                                let _ = tx.send(ProgressEvent::Error(e.to_string()));
                            }
                            return;
                        }
                    }
                }
            } else {
                let absolute_output = match resolve_output_path(
                    &base_args,
                    spec.film_name.as_deref(),
                    spec.film_year,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        return;
                    }
                };

                let display_title = spec
                    .film_name
                    .clone()
                    .unwrap_or_else(|| "Unknown DVD Title".to_string());
                let mut req_builder =
                    crate::application::rip_single_title::RipSingleTitleRequest::builder(
                        &dvd_path,
                        if spec.title_number == 0 {
                            1
                        } else {
                            spec.title_number
                        },
                        &absolute_output,
                    )
                    .display_title(&display_title)
                    .min_free_gb(spec.encoding_options.min_free_gb);
                if let Some(dur) = spec.expected_runtime_secs {
                    req_builder = req_builder.expected_duration(dur);
                }
                let request = match req_builder.build() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        return;
                    }
                };

                let observer = super::state::GuiProgressObserver { tx: tx.clone() };
                match container.rip_single_title_use_case().execute(
                    &request,
                    Some(&observer),
                    Some(cancel_flag.clone()),
                ) {
                    Ok(_) => {
                        rip_history_records.push((
                            display_title,
                            "Movie".to_string(),
                            absolute_output.clone(),
                            spec.expected_runtime_secs.unwrap_or(0.0),
                        ));
                        last_output_file = Some(absolute_output);
                    }
                    Err(e) => {
                        if cancel_flag.load(Ordering::SeqCst) {
                            let _ = tx.send(ProgressEvent::Error(
                                "Ripping process cancelled by user.".to_string(),
                            ));
                        } else {
                            let _ = tx.send(ProgressEvent::Error(e.to_string()));
                        }
                        return;
                    }
                }
            }

            // Record history and run post-processing
            for (title, media_type, path, dur) in rip_history_records {
                let mut rec =
                    RipRecord::new(&title, &media_type, &path.to_string_lossy(), "Success");
                if dur > 0.0 {
                    rec.duration_secs = Some(dur);
                }
                let _ = container.history_repo.append_record(&rec, None);
            }

            let post_title = spec.film_name.unwrap_or_else(|| "DVD Rip".to_string());
            let post_context = PostProcessContext::from_args(
                &base_args,
                last_output_file.clone(),
                &post_title,
                spec.film_year,
                dvd_path,
                spec.poster_bytes,
            );
            let _ = container.post_processor.process(&post_context);

            if let Some(saved_path) = last_output_file {
                let _ = tx.send(ProgressEvent::Success(saved_path));
            } else {
                let fallback = if spec.is_tv {
                    crate::ffmpeg::resolve_tv_root(&spec.out_dir)
                } else {
                    crate::ffmpeg::resolve_films_root(&spec.out_dir)
                };
                let _ = tx.send(ProgressEvent::Success(fallback));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_controller_lifecycle_and_channel() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let mut controller = GuiApplicationController::new(container.clone());

        assert!(Arc::ptr_eq(controller.container(), &container));

        let tx = controller.event_tx();
        let _ = tx.send(ProgressEvent::Log("Test log".to_string()));

        let ev = controller.poll_event();
        assert!(matches!(ev, Some(ProgressEvent::Log(_))));

        // Cancellation
        controller.cancel();
        let cancel_ev = controller.poll_event();
        assert!(matches!(cancel_ev, Some(ProgressEvent::Error(_))));

        // Folder methods
        controller.open_folder(Path::new("dummy_nonexistent"));

        // History methods
        let history = controller.load_history(None);
        assert_eq!(history.len(), controller.load_history(None).len());
        let _ = controller.clear_history(None);
    }

    #[test]
    fn test_rip_task_specification_clone_and_debug() {
        let spec = RipTaskSpecification {
            drive: "D:\\".to_string(),
            out_dir: "C:\\Out".to_string(),
            is_iso: false,
            is_tv: false,
            all_episodes: false,
            transcode: true,
            film_name: Some("The Matrix".to_string()),
            film_year: Some(1999),
            season: 1,
            start_episode: 1,
            title_number: 1,
            preset: "slow".to_string(),
            engine: "auto".to_string(),
            handbrake_path: "HandBrakeCLI".to_string(),
            makemkv_path: "makemkvcon".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: Some(8160.0),
            detected_episodes: Vec::new(),
            poster_bytes: None,
            encoding_options: crate::cli::EncodingOptions::default(),
        };

        let cloned = spec.clone();
        assert_eq!(spec.drive, cloned.drive);
        assert_eq!(format!("{:?}", spec), format!("{:?}", cloned));
    }

    #[test]
    fn test_gui_controller_async_detection_and_lookups() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let controller = GuiApplicationController::new(container);

        controller.start_detection("Z:\\NonExistentDrive", None);
        controller.start_search_lookup("tt0000000_fake");
        controller.start_tv_scan("Z:\\NonExistentDrive", "FakeShow", 1, 1);
        controller.start_thumbnail_preview("Z:\\NonExistentDrive", 1);
        controller.start_benchmark("Z:\\NonExistentDrive");

        // Wait a short moment for background threads to emit events
        std::thread::sleep(std::time::Duration::from_millis(150));

        // Poll whatever events were emitted
        while let Some(_event) = controller.poll_event() {}
    }

    #[test]
    fn test_gui_controller_start_ripping_iso_fails_gracefully() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let mut controller = GuiApplicationController::new(container);

        let spec = RipTaskSpecification {
            drive: "Z:\\NonExistentDrive".to_string(),
            out_dir: "C:\\Out".to_string(),
            is_iso: true,
            is_tv: false,
            all_episodes: false,
            transcode: false,
            film_name: Some("Test ISO".to_string()),
            film_year: None,
            season: 1,
            start_episode: 1,
            title_number: 1,
            preset: "slow".to_string(),
            engine: "auto".to_string(),
            handbrake_path: "HandBrakeCLI".to_string(),
            makemkv_path: "makemkvcon".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: None,
            detected_episodes: Vec::new(),
            poster_bytes: None,
            encoding_options: crate::cli::EncodingOptions::default(),
        };

        controller.start_ripping(spec);

        std::thread::sleep(std::time::Duration::from_millis(150));
        while let Some(_ev) = controller.poll_event() {}
    }

    #[test]
    fn test_gui_controller_start_ripping_titles_and_engines_fail_gracefully() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let mut controller = GuiApplicationController::new(container);

        // 1. Movie rip with HandBrake engine
        let spec_hb = RipTaskSpecification {
            drive: "Z:\\NonExistentDrive".to_string(),
            out_dir: "C:\\Out".to_string(),
            is_iso: false,
            is_tv: false,
            all_episodes: false,
            transcode: true,
            film_name: Some("HandBrake Movie".to_string()),
            film_year: Some(2020),
            season: 1,
            start_episode: 1,
            title_number: 1,
            preset: "fast".to_string(),
            engine: "handbrake".to_string(),
            handbrake_path: "HandBrakeCLI".to_string(),
            makemkv_path: "makemkvcon".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: Some(3600.0),
            detected_episodes: Vec::new(),
            poster_bytes: None,
            encoding_options: crate::cli::EncodingOptions::default(),
        };
        controller.start_ripping(spec_hb);

        // 2. TV rip with MakeMKV engine
        let spec_mmkv = RipTaskSpecification {
            drive: "Z:\\NonExistentDrive".to_string(),
            out_dir: "C:\\Out".to_string(),
            is_iso: false,
            is_tv: true,
            all_episodes: true,
            transcode: false,
            film_name: Some("Show Name".to_string()),
            film_year: None,
            season: 1,
            start_episode: 1,
            title_number: 1,
            preset: "fast".to_string(),
            engine: "makemkv".to_string(),
            handbrake_path: "HandBrakeCLI".to_string(),
            makemkv_path: "makemkvcon".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: None,
            detected_episodes: vec![crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                duration_secs: 2700.0,
                formatted_name: "Pilot".to_string(),
            }],
            poster_bytes: None,
            encoding_options: crate::cli::EncodingOptions::default(),
        };
        controller.start_ripping(spec_mmkv);

        // 3. Native IFO engine
        let spec_native = RipTaskSpecification {
            drive: "Z:\\NonExistentDrive".to_string(),
            out_dir: "C:\\Out".to_string(),
            is_iso: false,
            is_tv: false,
            all_episodes: false,
            transcode: false,
            film_name: Some("Native Movie".to_string()),
            film_year: None,
            season: 1,
            start_episode: 1,
            title_number: 1,
            preset: "fast".to_string(),
            engine: "native".to_string(),
            handbrake_path: "HandBrakeCLI".to_string(),
            makemkv_path: "makemkvcon".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: None,
            detected_episodes: Vec::new(),
            poster_bytes: None,
            encoding_options: crate::cli::EncodingOptions::default(),
        };
        controller.start_ripping(spec_native);

        std::thread::sleep(std::time::Duration::from_millis(200));
        while let Some(_ev) = controller.poll_event() {}
    }
}
