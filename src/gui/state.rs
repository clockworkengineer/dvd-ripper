//! Central state container and asynchronous event processing for the desktop GUI.
//! Follows Jakob Nielsen's Heuristics #1 (Visibility of System Status) and #6 (Recognition Rather than Recall).

use eframe::egui;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use crate::cli::Args;
use crate::domain::entities::disc::TvEpisodeInfo;
use crate::dvd::{get_volume_label, normalize_dvd_path};
use crate::ffmpeg::{
    ProgressEvent, detect_tv_episodes, resolve_output_path, resolve_tv_output_path,
};
use crate::history::{RipRecord, load_history, record_rip_event, save_history};
use crate::imdb::lookup_film_details;
use crate::utils::{format_episode_name, sanitize_filename};

use super::tabs::ActiveTab;

pub struct GuiProgressObserver {
    pub tx: Sender<ProgressEvent>,
}

impl crate::domain::ports::transcoder::ProgressObserver for GuiProgressObserver {
    fn on_progress(&self, percent: f64, fps: &str, speed: &str) {
        let _ = self.tx.send(ProgressEvent::Progress {
            percent,
            fps: fps.to_string(),
            speed: speed.to_string(),
        });
    }

    fn on_log(&self, line: &str) {
        let _ = self.tx.send(ProgressEvent::Log(line.to_string()));
    }
}

/// Main application state for the eframe GUI.
pub struct DvdRipperApp {
    // Navigation & UX mode flags
    pub active_tab: ActiveTab,
    pub simple_mode: bool,
    pub recent_out_dirs: Vec<String>,
    pub history_filter: String,
    pub start_time: Option<Instant>,
    pub eta_seconds: Option<f64>,
    pub benchmark_report: Option<crate::dvd::DriveBenchmarkReport>,

    // Confirmation & Help modals
    pub show_confirm_cancel: bool,
    pub show_confirm_clear: bool,
    pub show_confirm_reset: bool,
    pub show_help_modal: bool,

    // Primary Disc & Storage Fields
    pub input_drive: String,
    pub film_name: String,
    pub film_year: String,
    pub out_dir: String,
    pub title_number: u32,
    pub transcode: bool,
    pub mkv: bool,
    pub iso_mode: bool,
    pub preset: String,
    pub ffmpeg_path: String,
    pub expected_runtime_secs: Option<f64>,

    // TV Series mode options
    pub is_tv_mode: bool,
    pub season_number: u32,
    pub start_episode: u32,
    pub all_episodes: bool,
    pub detected_episodes: Vec<TvEpisodeInfo>,

    // Metadata fields
    pub plot: String,
    pub genre: String,
    pub director: String,
    pub actors: String,
    pub rating: String,
    pub raw_poster_bytes: Option<Vec<u8>>,
    pub poster_texture: Option<egui::TextureHandle>,
    pub title_thumbnails: HashMap<u32, (Vec<u8>, Option<egui::TextureHandle>)>,
    pub is_fetching_thumb: bool,

    // Stream & Notification options
    pub all_audio: bool,
    pub normalize_audio: bool,
    pub dual_audio: bool,
    pub audio_lang: String,
    pub subtitles: bool,
    pub sub_lang: String,
    pub sub_format: String,
    pub webhook_url: String,
    pub no_overwrite: bool,
    pub auto_boxset: bool,

    // Codec & Profile settings
    pub codec: String,
    pub profile: String,
    pub deinterlace: bool,
    pub deinterlace_algo: String,
    pub denoise: bool,
    pub min_free_gb: u64,
    pub no_eject: bool,
    pub sub_forced_only: bool,
    pub sub_external_srt: bool,
    pub ocr: bool,
    pub ocr_lang: String,
    pub tesseract: String,
    pub audio_track_str: String,
    pub hwaccel_fallback: bool,
    pub checksum: bool,
    pub nfo: bool,
    pub norm_target: f64,
    pub container: Option<Arc<crate::AppContainer>>,

    // Modals & Detection State
    pub show_search_modal: bool,
    pub search_candidates: Vec<crate::imdb::SearchResultItem>,
    pub show_about_modal: bool,

    pub detecting: bool,
    pub detect_status: String,

    pub is_ripping: bool,
    pub progress_percent: f32,
    pub fps: String,
    pub speed: String,
    pub status_message: String,
    pub logs: VecDeque<String>,

    pub event_tx: Sender<ProgressEvent>,
    pub event_rx: Receiver<ProgressEvent>,
    pub cancel_tx: Option<Sender<()>>,
    pub cancel_flag: Option<Arc<AtomicBool>>,

    pub history: Vec<RipRecord>,
}

impl Default for DvdRipperApp {
    fn default() -> Self {
        let (event_tx, event_rx) = channel();
        let cfg = crate::config::load_config(None);
        let default_out = cfg.out_dir.clone().unwrap_or_else(|| {
            crate::utils::get_default_rip_output_dir()
                .to_string_lossy()
                .to_string()
        });
        Self {
            active_tab: ActiveTab::Rip,
            simple_mode: true,
            recent_out_dirs: vec![default_out.clone()],
            history_filter: String::new(),
            start_time: None,
            eta_seconds: None,
            benchmark_report: None,

            show_confirm_cancel: false,
            show_confirm_clear: false,
            show_confirm_reset: false,
            show_help_modal: false,

            input_drive: "auto".to_string(),
            film_name: String::new(),
            film_year: String::new(),
            out_dir: default_out,
            title_number: 0,
            transcode: false,
            mkv: false,
            iso_mode: false,
            preset: "veryfast".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: None,

            is_tv_mode: false,
            season_number: 1,
            start_episode: 1,
            all_episodes: false,
            detected_episodes: Vec::new(),

            plot: String::new(),
            genre: String::new(),
            director: String::new(),
            actors: String::new(),
            rating: String::new(),
            raw_poster_bytes: None,
            poster_texture: None,
            title_thumbnails: HashMap::new(),
            is_fetching_thumb: false,

            all_audio: false,
            normalize_audio: false,
            dual_audio: false,
            audio_lang: String::new(),
            subtitles: false,
            sub_lang: String::new(),
            sub_format: "dvdsub".to_string(),
            webhook_url: String::new(),
            no_overwrite: false,
            auto_boxset: false,

            codec: "h264".to_string(),
            profile: "standard".to_string(),
            deinterlace: false,
            deinterlace_algo: "bwdif".to_string(),
            denoise: false,
            min_free_gb: 10,
            no_eject: false,
            sub_forced_only: false,
            sub_external_srt: false,
            ocr: false,
            ocr_lang: "eng".to_string(),
            tesseract: "tesseract".to_string(),
            audio_track_str: String::new(),
            hwaccel_fallback: false,
            checksum: false,
            nfo: false,
            norm_target: -23.0,
            container: None,

            show_search_modal: false,
            search_candidates: Vec::new(),
            show_about_modal: false,

            detecting: false,
            detect_status: String::new(),

            is_ripping: false,
            progress_percent: 0.0,
            fps: "N/A".to_string(),
            speed: "N/A".to_string(),
            status_message: "Ready to rip".to_string(),
            logs: VecDeque::new(),

            event_tx,
            event_rx,
            cancel_tx: None,
            cancel_flag: None,

            history: load_history(None),
        }
    }
}

impl DvdRipperApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        app.trigger_detection();
        app
    }

    #[allow(dead_code)]
    pub fn with_container(container: Arc<crate::AppContainer>) -> Self {
        let mut app = Self {
            container: Some(container),
            ..Default::default()
        };
        app.trigger_detection();
        app
    }

    pub fn film_name_opt(&self) -> Option<String> {
        let trimmed = self.film_name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn effective_show_name(&self) -> String {
        self.film_name_opt()
            .unwrap_or_else(|| "TV Show".to_string())
    }

    pub fn pick_output_dir(&mut self) {
        let starting_dir = if Path::new(&self.out_dir).exists() {
            self.out_dir.clone()
        } else {
            String::new()
        };
        let mut dialog =
            rfd::FileDialog::new().set_title("Select Default Output Folder for Ripped DVDs");
        if !starting_dir.is_empty() {
            dialog = dialog.set_directory(&starting_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            let path_str = folder.to_string_lossy().to_string();
            self.out_dir = path_str.clone();
            if !self.recent_out_dirs.contains(&path_str) {
                self.recent_out_dirs.insert(0, path_str);
                if self.recent_out_dirs.len() > 5 {
                    self.recent_out_dirs.pop();
                }
            }
            self.save_current_output_dir_preference();
        }
    }

    pub fn save_current_output_dir_preference(&self) {
        let mut cfg = crate::config::load_config(None);
        cfg.out_dir = Some(self.out_dir.clone());
        let _ = crate::config::save_default_config(&cfg);
    }

    pub fn reset_to_defaults(&mut self) {
        let def = Self::default();
        self.simple_mode = def.simple_mode;
        self.codec = def.codec;
        self.profile = def.profile;
        self.preset = def.preset;
        self.normalize_audio = def.normalize_audio;
        self.norm_target = def.norm_target;
        self.hwaccel_fallback = def.hwaccel_fallback;
        self.min_free_gb = def.min_free_gb;
        self.webhook_url = def.webhook_url;
        self.no_eject = def.no_eject;
        self.checksum = def.checksum;
        self.nfo = def.nfo;
        self.mkv = def.mkv;
        self.transcode = def.transcode;
        self.deinterlace = def.deinterlace;
        self.denoise = def.denoise;
    }

    pub fn build_encoding_options(&self) -> crate::cli::EncodingOptions {
        crate::cli::EncodingOptions {
            all_audio: self.all_audio,
            normalize_audio: self.normalize_audio,
            dual_audio: self.dual_audio,
            mkv: self.mkv,
            codec: self.codec.clone(),
            profile: self.profile.clone(),
            audio_lang: if self.audio_lang.trim().is_empty() {
                None
            } else {
                Some(self.audio_lang.trim().to_string())
            },
            subtitles: self.subtitles,
            sub_lang: if self.sub_lang.trim().is_empty() {
                None
            } else {
                Some(self.sub_lang.trim().to_string())
            },
            sub_format: Some(self.sub_format.clone()),
            webhook_url: if self.webhook_url.trim().is_empty() {
                None
            } else {
                Some(self.webhook_url.trim().to_string())
            },
            no_overwrite: self.no_overwrite,
            auto_boxset: self.auto_boxset,
            deinterlace: self.deinterlace,
            deinterlace_algo: Some(self.deinterlace_algo.clone()),
            denoise: self.denoise,
            min_free_gb: self.min_free_gb,
            no_eject: self.no_eject,
            sub_forced_only: self.sub_forced_only,
            sub_external_srt: self.sub_external_srt,
            ocr: self.ocr,
            ocr_lang: self.ocr_lang.clone(),
            tesseract: self.tesseract.clone(),
            audio_track: self.audio_track_str.trim().parse::<u32>().ok(),
            hwaccel_fallback: self.hwaccel_fallback,
            audio_title: None,
            webhook_secret: None,
            audit_log: None,
            spindown: false,
            sub_default: false,
            tags: None,
            checksum: self.checksum,
            fallback_out_dir: None,
            sub_burnin: false,
            norm_target: if self.normalize_audio {
                Some(self.norm_target as i32)
            } else {
                None
            },
            eject_autoclose: None,
            label_regex_replace: None,
            audio_downmix: None,
            auto_cleanup_days: None,
            drive_pool: None,
            auto_rip: false,
            tonemap: None,
            ignore_errors: true,
            iso: self.iso_mode,
        }
    }

    pub fn trigger_detection(&mut self) {
        self.trigger_detection_with_query(None);
    }

    pub fn update_detected_episode_names(&mut self) {
        let show_name = self.effective_show_name();
        let season = self.season_number;
        for (ep_num, ep) in (self.start_episode..).zip(self.detected_episodes.iter_mut()) {
            ep.episode_num = ep_num;
            ep.formatted_name = format_episode_name(&show_name, season, ep_num);
        }
    }

    pub fn trigger_custom_search(&mut self) {
        let query = self.film_name.trim().to_string();
        if query.is_empty() {
            self.detect_status =
                "Please enter a title/show name first (e.g. Doctor Who).".to_string();
            return;
        }
        self.trigger_detection_with_query(Some(query));
    }

    pub fn select_candidate_by_id(&mut self, imdb_id: &str) {
        self.detecting = true;
        self.detect_status = "Loading selected title details...".to_string();
        let id = imdb_id.to_string();
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
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

    pub fn trigger_detection_with_query(&mut self, query_override: Option<String>) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Searching metadata...".to_string();

        let drive = self.input_drive.clone();
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let dvd_path = normalize_dvd_path(&drive);
            let (best_title, best_dur) = crate::ffmpeg::detect_best_title_info("", &dvd_path, None);
            if best_title > 0 {
                let _ = tx.send(ProgressEvent::DetectedTitle {
                    title_num: best_title,
                    duration_secs: best_dur.unwrap_or(0.0),
                });
                let temp_thumb = crate::infrastructure::storage::TempResource::new(
                    &format!("dvd_thumb_title_{}", best_title),
                    "jpg",
                );
                if crate::ffmpeg::generate_title_thumbnail(
                    "ffmpeg",
                    &dvd_path,
                    best_title,
                    120.0,
                    temp_thumb.path(),
                )
                .is_ok()
                    && let Ok(bytes) = std::fs::read(temp_thumb.path())
                {
                    let _ = tx.send(ProgressEvent::TitleThumbnailReady {
                        title_num: best_title,
                        image_bytes: bytes,
                    });
                }
            }

            let volume_label = get_volume_label(&dvd_path.to_string_lossy());
            if query_override.is_none() && volume_label.is_some() {
                let fp = crate::dvd::compute_disc_fingerprint(&dvd_path.to_string_lossy());
                let cache = crate::infrastructure::metadata::DiskFingerprintCache::new();
                if let Some(entry) = cache.lookup_entry(&fp) {
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "💾 Instant Recognition from Disc Cache: {} ({:?})",
                        entry.metadata.title, entry.metadata.year
                    )));
                    let clean = sanitize_filename(&entry.metadata.title);
                    let year_str = entry
                        .metadata
                        .year
                        .map(|y| y.to_string())
                        .unwrap_or_default();
                    let runtime_desc = entry
                        .metadata
                        .runtime_secs
                        .map(|r| format!(" [Runtime: {:.0}m]", r / 60.0))
                        .unwrap_or_default();
                    let _ = tx.send(ProgressEvent::Log(format!(
                        "Metadata Found: {} ({}){}",
                        clean, year_str, runtime_desc
                    )));
                    let _ = tx.send(ProgressEvent::Metadata(entry.to_imdb_metadata()));
                    return;
                }
            }

            let search_term = if let Some(q) = query_override {
                q
            } else if let Some(label) = volume_label {
                let _ = tx.send(ProgressEvent::Log(format!(
                    "Detected Volume Label: {}",
                    label
                )));
                label
            } else {
                let _ = tx.send(ProgressEvent::Log(
                    "Could not read volume label.".to_string(),
                ));
                let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
                return;
            };

            let _ = tx.send(ProgressEvent::Log(format!(
                "Searching metadata candidates for query: '{}'",
                search_term
            )));
            let candidates = crate::imdb::fetch_search_candidates(&search_term);
            if !candidates.is_empty() {
                let _ = tx.send(ProgressEvent::SearchResults(candidates));
            } else {
                match lookup_film_details(&search_term) {
                    Ok(meta) => {
                        let clean = sanitize_filename(&meta.title);
                        let year_str = meta.year.map(|y| y.to_string()).unwrap_or_default();
                        let runtime_desc = meta
                            .runtime_secs
                            .map(|r| format!(" [Runtime: {:.0}m]", r / 60.0))
                            .unwrap_or_default();
                        let _ = tx.send(ProgressEvent::Log(format!(
                            "Metadata Found: {} ({}){}",
                            clean, year_str, runtime_desc
                        )));
                        let _ = tx.send(ProgressEvent::Metadata(meta));
                    }
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Log(format!(
                            "Lookup notice: {}. If '{}' is a disc catalog code, enter the show name (e.g. 'Doctor Who') and click '🔍 Search'.",
                            e, search_term
                        )));
                        let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
                    }
                }
            }
        });
    }

    pub fn trigger_tv_scan(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Scanning disc for TV episode titles...".to_string();

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());

        let ffmpeg_path = self.ffmpeg_path.clone();
        let dvd_path = normalize_dvd_path(&self.input_drive);
        let show_name = self.effective_show_name();
        let season = self.season_number;
        let start_ep = self.start_episode;
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let episodes = detect_tv_episodes(
                &ffmpeg_path,
                &dvd_path,
                &show_name,
                season,
                start_ep,
                Some(&cancel_flag),
            );
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = tx.send(ProgressEvent::Log(
                    "Disc episode scanning cancelled by user.".to_string(),
                ));
                return;
            }
            let _ = tx.send(ProgressEvent::Log(format!(
                "Scanned disc: found {} episode titles",
                episodes.len()
            )));
            let _ = tx.send(ProgressEvent::TvEpisodesDetected(episodes));
        });
    }

    pub fn trigger_thumbnail_preview(&mut self, title_num: u32) {
        if self.is_fetching_thumb || self.is_ripping {
            return;
        }
        let target_title = if title_num == 0 { 1 } else { title_num };
        let drive = self.input_drive.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let tx = self.event_tx.clone();
        self.is_fetching_thumb = true;
        self.status_message = format!("Capturing preview thumbnail for Title #{}...", target_title);

        std::thread::spawn(move || {
            let dvd_path = normalize_dvd_path(&drive);
            let temp_thumb = crate::infrastructure::storage::TempResource::new(
                &format!("dvd_thumb_title_{}", target_title),
                "jpg",
            );
            let ffmpeg = if ffmpeg_path.is_empty() {
                "ffmpeg"
            } else {
                &ffmpeg_path
            };
            if crate::ffmpeg::generate_title_thumbnail(
                ffmpeg,
                &dvd_path,
                target_title,
                120.0,
                temp_thumb.path(),
            )
            .is_ok()
            {
                if let Ok(bytes) = std::fs::read(temp_thumb.path()) {
                    let _ = tx.send(ProgressEvent::TitleThumbnailReady {
                        title_num: target_title,
                        image_bytes: bytes,
                    });
                }
            } else {
                let _ = tx.send(ProgressEvent::Log(format!(
                    "Thumbnail snapshot extraction failed for Title #{}",
                    target_title
                )));
            }
        });
    }

    pub fn trigger_benchmark(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Running 10-second optical drive throughput benchmark...".to_string();
        self.status_message = "Running drive benchmark...".to_string();

        let ffmpeg_path = self.ffmpeg_path.clone();
        let drive = self.input_drive.clone();
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let dvd_path = normalize_dvd_path(&drive);
            match crate::dvd::run_drive_benchmark(&ffmpeg_path, &dvd_path, 10) {
                Ok(report) => {
                    let msg = format!(
                        "Drive Benchmark: {:.2} MB/s throughput ({}, {} FPS)",
                        report.read_speed_mbps, report.rating_summary, report.fps
                    );
                    let _ = tx.send(ProgressEvent::Log(msg));
                    let _ = tx.send(ProgressEvent::BenchmarkFinished(report));
                }
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Error(format!("Benchmark error: {}", e)));
                }
            }
        });
    }

    pub fn start_ripping(&mut self) {
        if self.is_ripping {
            return;
        }

        let drive = self.input_drive.clone();
        let dvd_path = normalize_dvd_path(&drive);
        if !dvd_path.exists() {
            self.status_message =
                format!("Error: DVD path does not exist ({})", dvd_path.display());
            return;
        }

        self.is_ripping = true;
        self.progress_percent = 0.0;
        self.fps = "N/A".to_string();
        self.speed = "N/A".to_string();
        self.status_message = "Starting ripping process...".to_string();
        self.start_time = Some(Instant::now());
        self.eta_seconds = None;

        let (cancel_tx, _cancel_rx) = channel();
        self.cancel_tx = Some(cancel_tx);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());

        let is_iso = self.iso_mode;
        let is_tv = self.is_tv_mode;
        let all_eps = self.all_episodes;
        let show_name_opt = self.film_name_opt();
        let year_opt = self.film_year.trim().parse::<u32>().ok();
        let season = self.season_number;
        let start_ep = self.start_episode;
        let title_num = self.title_number;
        let out_dir = self.out_dir.clone();
        let transcode = self.transcode;
        let preset = self.preset.clone();
        let ffmpeg_path = self.ffmpeg_path.clone();
        let expected_runtime = self.expected_runtime_secs;
        let tx = self.event_tx.clone();

        let opts = self.build_encoding_options();

        let container = self.container.clone().unwrap_or_else(|| {
            let mut base_args = Args {
                input: drive.clone(),
                out_dir: out_dir.clone(),
                ffmpeg: ffmpeg_path.clone(),
                ..Default::default()
            };
            base_args.apply_encoding_options(opts.clone());
            Arc::new(crate::AppContainer::build_from_args(&base_args))
        });

        std::thread::spawn(move || {
            let observer = GuiProgressObserver { tx: tx.clone() };

            if is_iso {
                let mut args = Args::new_movie(
                    drive.clone(),
                    out_dir.clone(),
                    title_num,
                    false,
                    preset.clone(),
                    ffmpeg_path.clone(),
                );
                args.iso = true;
                args.apply_encoding_options(opts.clone());

                match resolve_output_path(&args, show_name_opt.as_deref(), year_opt) {
                    Ok(abs_out) => {
                        let label = crate::dvd::get_volume_label(&dvd_path.to_string_lossy());
                        let display_title =
                            show_name_opt.unwrap_or_else(|| "Archival ISO Dump".to_string());
                        let request = crate::application::dump_iso::DumpIsoRequest {
                            source_dvd: dvd_path.clone(),
                            output_path: abs_out.clone(),
                            volume_label: label,
                            display_title,
                            min_free_gb: opts.min_free_gb,
                        };
                        let use_case = container.dump_iso_use_case();
                        match use_case.execute(&request, Some(&observer), Some(cancel_flag.clone()))
                        {
                            Ok(_) => {
                                let _ = tx.send(ProgressEvent::Success(abs_out));
                            }
                            Err(e) => {
                                if cancel_flag.load(Ordering::SeqCst) {
                                    let _ = tx.send(ProgressEvent::Error(
                                        "Archival ISO dump cancelled by user.".to_string(),
                                    ));
                                } else {
                                    let _ = tx.send(ProgressEvent::Error(format!(
                                        "ISO dump error: {}",
                                        e
                                    )));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(format!(
                            "Path resolution error: {}",
                            e
                        )));
                    }
                }
                return;
            }

            if is_tv && all_eps {
                let show_name = show_name_opt.unwrap_or_else(|| "TV Show".to_string());
                let eps = detect_tv_episodes(
                    &ffmpeg_path,
                    &dvd_path,
                    &show_name,
                    season,
                    start_ep,
                    Some(&cancel_flag),
                );

                if cancel_flag.load(Ordering::SeqCst) {
                    let _ = tx.send(ProgressEvent::Error(
                        "TV batch ripping cancelled by user.".to_string(),
                    ));
                    return;
                }

                if eps.is_empty() {
                    let _ = tx.send(ProgressEvent::Error(
                        "No TV episode titles found on disc.".to_string(),
                    ));
                    return;
                }

                let total = eps.len();
                let _ = tx.send(ProgressEvent::Log(format!(
                    "Starting batch rip of {} TV episodes...",
                    total
                )));

                let request = crate::application::rip_tv_batch::RipTvBatchRequest {
                    source_dvd: dvd_path.clone(),
                    series_title: show_name.clone(),
                    season_number: season,
                    start_episode: start_ep,
                    output_dir: PathBuf::from(&out_dir),
                    episodes: eps,
                    metadata: None,
                    min_free_gb: opts.min_free_gb,
                    file_extension: if opts.mkv {
                        Some("mkv".to_string())
                    } else {
                        None
                    },
                    lookup_episode_titles: true,
                    track_boxset: opts.auto_boxset,
                };
                let use_case = container.rip_tv_batch_use_case();
                match use_case.execute(&request, Some(&observer), Some(cancel_flag.clone())) {
                    Ok(ripped_count) => {
                        let _ = tx.send(ProgressEvent::Success(PathBuf::from(format!(
                            "Completed batch ripping {} episodes to TV directory",
                            ripped_count
                        ))));
                    }
                    Err(e) => {
                        if cancel_flag.load(Ordering::SeqCst) {
                            let _ = tx.send(ProgressEvent::Error(
                                "TV batch ripping cancelled by user.".to_string(),
                            ));
                        } else {
                            let _ = tx.send(ProgressEvent::Error(format!(
                                "TV batch ripping error: {}",
                                e
                            )));
                        }
                    }
                }
                return;
            }

            let display_title = show_name_opt.unwrap_or_else(|| "DVD Title".to_string());
            let args = if is_tv {
                let ep_num = if start_ep > 0 { start_ep } else { 1 };
                let mut a = Args::new_tv(
                    drive.clone(),
                    out_dir.clone(),
                    title_num,
                    season,
                    ep_num,
                    transcode,
                    preset.clone(),
                    ffmpeg_path.clone(),
                );
                a.apply_encoding_options(opts.clone());
                a
            } else {
                let mut a = Args::new_movie(
                    drive.clone(),
                    out_dir.clone(),
                    title_num,
                    transcode,
                    preset.clone(),
                    ffmpeg_path.clone(),
                );
                a.apply_encoding_options(opts.clone());
                a
            };

            let res_out = if is_tv {
                let ep_num = if start_ep > 0 { start_ep } else { 1 };
                resolve_tv_output_path(&args, Some(&display_title), year_opt, season, ep_num)
            } else {
                resolve_output_path(&args, Some(&display_title), year_opt)
            };

            match res_out {
                Ok(abs_out) => {
                    let request = crate::application::rip_single_title::RipSingleTitleRequest {
                        source_dvd: dvd_path.clone(),
                        title_number: if title_num == 0 { 1 } else { title_num },
                        output_path: abs_out.clone(),
                        display_title: display_title.clone(),
                        expected_duration_secs: expected_runtime,
                        metadata: None,
                        min_free_gb: opts.min_free_gb,
                    };
                    let use_case = container.rip_single_title_use_case();
                    match use_case.execute(&request, Some(&observer), Some(cancel_flag.clone())) {
                        Ok(_) => {
                            let _ = tx.send(ProgressEvent::Success(abs_out));
                        }
                        Err(e) => {
                            if cancel_flag.load(Ordering::SeqCst) {
                                let _ = tx.send(ProgressEvent::Error(
                                    "Ripping process cancelled by user.".to_string(),
                                ));
                            } else {
                                let _ =
                                    tx.send(ProgressEvent::Error(format!("Ripping error: {}", e)));
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Error(format!(
                        "Path resolution error: {}",
                        e
                    )));
                }
            }
        });
    }

    pub fn cancel_ripping(&mut self) {
        if let Some(flag) = self.cancel_flag.take() {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        self.is_ripping = false;
        self.detecting = false;
        self.progress_percent = 0.0;
        self.fps = "N/A".to_string();
        self.speed = "N/A".to_string();
        self.status_message = "Ripping process cancelled by user.".to_string();
        self.start_time = None;
        self.eta_seconds = None;

        let title = self
            .film_name_opt()
            .unwrap_or_else(|| "Unknown DVD Rip".to_string());
        let media_type = if self.is_tv_mode {
            "TV Series"
        } else {
            "Movie"
        };
        let record = RipRecord::new(&title, media_type, &self.out_dir, "Cancelled");
        self.history.insert(0, record);
        let _ = save_history(&self.history, None);
    }

    pub fn poll_events(&mut self, ctx: &egui::Context) {
        self.process_events();

        if self.poster_texture.is_none()
            && let Some(ref bytes) = self.raw_poster_bytes
            && let Ok(image) = image::load_from_memory(bytes)
        {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as _, rgba.height() as _];
            let pixels = rgba.into_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            self.poster_texture =
                Some(ctx.load_texture("movie_poster", color_image, Default::default()));
        }

        for (title_num, (bytes, texture)) in &mut self.title_thumbnails {
            if texture.is_none()
                && let Ok(image) = image::load_from_memory(bytes)
            {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as _, rgba.height() as _];
                let pixels = rgba.into_raw();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                *texture = Some(ctx.load_texture(
                    format!("title_thumb_{}", title_num),
                    color_image,
                    Default::default(),
                ));
            }
        }
    }

    pub fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ProgressEvent::Log(line) => {
                    if line == "META_FAIL" {
                        self.detecting = false;
                        self.detect_status = "Detection finished.".to_string();
                    } else {
                        if self.is_ripping
                            && (line.starts_with("Auto-detecting")
                                || line.starts_with("Auto-selected")
                                || line.starts_with("Ripping"))
                        {
                            self.status_message = line.clone();
                        }
                        self.logs.push_back(line);
                        if self.logs.len() > 500 {
                            self.logs.pop_front();
                        }
                    }
                }
                ProgressEvent::SearchResults(candidates) => {
                    self.detecting = false;
                    self.detect_status = format!("Found {} search matches.", candidates.len());
                    if candidates.len() == 1 {
                        let id = candidates[0].imdb_id.clone();
                        self.select_candidate_by_id(&id);
                    } else if candidates.len() > 1 {
                        self.search_candidates = candidates;
                        self.show_search_modal = true;
                    } else {
                        self.status_message = "No search results found.".to_string();
                    }
                }
                ProgressEvent::Metadata(meta) => {
                    self.film_name = meta.title;
                    self.film_year = meta.year.map(|y| y.to_string()).unwrap_or_default();
                    self.expected_runtime_secs = meta.runtime_secs;
                    self.plot = meta.plot.unwrap_or_default();
                    self.genre = meta.genre.unwrap_or_default();
                    self.director = meta.director.unwrap_or_default();
                    self.actors = meta.actors.unwrap_or_default();
                    self.rating = meta.rating.unwrap_or_default();
                    self.raw_poster_bytes = meta.poster_bytes;
                    self.poster_texture = None;
                    if meta.is_series {
                        self.is_tv_mode = true;
                        self.update_detected_episode_names();
                    }
                    self.detecting = false;
                    self.detect_status = "Detection complete.".to_string();
                }
                ProgressEvent::TvEpisodesDetected(episodes) => {
                    self.detected_episodes = episodes;
                    self.detecting = false;
                    self.detect_status = format!(
                        "Found {} TV episode titles on disc.",
                        self.detected_episodes.len()
                    );
                }
                ProgressEvent::BenchmarkFinished(report) => {
                    self.detecting = false;
                    let msg = format!(
                        "Drive Benchmark: {:.2} MB/s throughput ({}, {} FPS)",
                        report.read_speed_mbps, report.rating_summary, report.fps
                    );
                    self.detect_status = msg.clone();
                    self.status_message = format!("Benchmark complete: {}", report.rating_summary);
                    self.benchmark_report = Some(report);
                }
                ProgressEvent::DetectedTitle {
                    title_num,
                    duration_secs,
                } => {
                    if self.title_number == 0 {
                        self.title_number = title_num;
                    }
                    if duration_secs > 0.0 {
                        let mins = (duration_secs / 60.0).round() as u32;
                        let h = mins / 60;
                        let m = mins % 60;
                        let dur_str = if h > 0 {
                            format!("{}h {}m", h, m)
                        } else {
                            format!("{}m", m)
                        };
                        self.detect_status =
                            format!("Auto-selected Title #{} ({})", title_num, dur_str);
                    } else {
                        self.detect_status = format!("Auto-selected Title #{}", title_num);
                    }
                }
                ProgressEvent::TitleThumbnailReady {
                    title_num,
                    image_bytes,
                } => {
                    self.is_fetching_thumb = false;
                    self.title_thumbnails.insert(title_num, (image_bytes, None));
                }
                ProgressEvent::Progress {
                    percent,
                    fps,
                    speed,
                } => {
                    if !self.is_ripping {
                        continue;
                    }
                    self.progress_percent = (percent as f32) / 100.0;
                    self.fps = fps;
                    self.speed = speed;

                    if self.start_time.is_none() {
                        self.start_time = Some(Instant::now());
                    }

                    if percent > 0.01
                        && let Some(start) = self.start_time
                    {
                        let elapsed = start.elapsed().as_secs_f64();
                        let fraction = (percent / 100.0).clamp(0.001, 1.0);
                        let eta = (1.0 - fraction) * (elapsed / fraction);
                        self.eta_seconds = Some(eta);
                    }

                    let pct_formatted = if percent < 1.0 {
                        format!("{:.2}%", percent)
                    } else {
                        format!("{:.1}%", percent)
                    };
                    self.status_message = format!("Ripping in progress... ({})", pct_formatted);
                }
                ProgressEvent::Success(path) => {
                    if !self.is_ripping {
                        continue;
                    }
                    self.is_ripping = false;
                    self.start_time = None;
                    self.eta_seconds = None;
                    self.progress_percent = 1.0;
                    self.status_message = format!("Success! Saved to {}", path.display());
                    self.cancel_tx = None;
                    self.cancel_flag = None;

                    let title = if self.film_name.trim().is_empty() {
                        path.file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown DVD Rip".to_string())
                    } else {
                        self.film_name.trim().to_string()
                    };
                    let media_type = if self.is_tv_mode {
                        "TV Series"
                    } else {
                        "Movie"
                    };
                    let _ =
                        record_rip_event(&title, media_type, &path.to_string_lossy(), "Success");
                    self.history = load_history(None);

                    if let Some(ref bytes) = self.raw_poster_bytes {
                        let _ = crate::utils::save_cover_artworks(&path, bytes);
                    }

                    let dvd_path = normalize_dvd_path(&self.input_drive);
                    if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                        let fp = crate::dvd::compute_disc_fingerprint(&dvd_path.to_string_lossy());
                        let entry = crate::infrastructure::metadata::CachedDiscEntry {
                            fingerprint: fp,
                            volume_label: Some(label),
                            metadata: crate::domain::entities::media::FilmMetadata {
                                title: self.film_name.clone(),
                                year: self.film_year.parse().ok(),
                                runtime_secs: self.expected_runtime_secs,
                                poster_url: None,
                                plot: if self.plot.is_empty() {
                                    None
                                } else {
                                    Some(self.plot.clone())
                                },
                                genre: if self.genre.is_empty() {
                                    None
                                } else {
                                    Some(self.genre.clone())
                                },
                                director: if self.director.is_empty() {
                                    None
                                } else {
                                    Some(self.director.clone())
                                },
                                actors: if self.actors.is_empty() {
                                    None
                                } else {
                                    Some(self.actors.clone())
                                },
                                rating: if self.rating.is_empty() {
                                    None
                                } else {
                                    Some(self.rating.clone())
                                },
                                is_series: self.is_tv_mode,
                                total_seasons: None,
                                poster_bytes: self.raw_poster_bytes.clone(),
                            },
                            title_number: if self.title_number > 0 {
                                Some(self.title_number)
                            } else {
                                None
                            },
                            is_tv: self.is_tv_mode,
                            season: if self.is_tv_mode {
                                Some(self.season_number)
                            } else {
                                None
                            },
                            cached_at: chrono::Local::now().to_rfc3339(),
                        };
                        let _ = crate::infrastructure::metadata::DiskFingerprintCache::new()
                            .save_entry(&entry);
                    }

                    if !self.no_eject {
                        let _ = crate::dvd::eject_disc(&self.input_drive);
                    }
                }
                ProgressEvent::Error(msg) => {
                    self.is_ripping = false;
                    self.detecting = false;
                    self.start_time = None;
                    self.eta_seconds = None;
                    self.cancel_tx = None;
                    self.cancel_flag = None;
                    self.progress_percent = 0.0;
                    self.fps = "N/A".to_string();
                    self.speed = "N/A".to_string();

                    if msg.contains("cancelled by user") || msg.contains("Ripping cancelled") {
                        self.status_message = "Ripping process cancelled by user.".to_string();
                    } else {
                        self.status_message = format!("Stopped: {}", msg);
                    }
                }
            }
        }
    }

    pub fn render_search_modal(&mut self, ctx: &egui::Context) {
        if !self.show_search_modal {
            return;
        }

        let mut selected_id = None;

        egui::Window::new("🔍 Select Movie / TV Show Match")
            .collapsible(false)
            .resizable(true)
            .default_size([550.0, 380.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Multiple matches were found on OMDb/IMDb. Select the correct title:");
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for cand in &self.search_candidates {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        let year_str = cand
                                            .year
                                            .map(|y| format!(" ({})", y))
                                            .unwrap_or_default();
                                        let type_str =
                                            format!("[{}]", cand.type_field.to_uppercase());
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} {}",
                                                cand.title, year_str
                                            ))
                                            .strong(),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Type: {}  |  IMDb ID: {}",
                                                type_str, cand.imdb_id
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Select ▶").clicked() {
                                                selected_id = Some(cand.imdb_id.clone());
                                            }
                                        },
                                    );
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_search_modal = false;
                    }
                });
            });

        if let Some(imdb_id) = selected_id {
            self.show_search_modal = false;
            self.select_candidate_by_id(&imdb_id);
        }
    }
}
