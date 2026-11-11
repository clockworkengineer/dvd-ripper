//! Central state container and asynchronous event processing for the desktop GUI.
//! Follows Jakob Nielsen's Heuristics #1 (Visibility of System Status) and #6 (Recognition Rather than Recall).

use eframe::egui;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;

use super::controller::{GuiApplicationController, RipTaskSpecification};
use super::tabs::ActiveTab;
use crate::domain::entities::disc::TvEpisodeInfo;
use crate::dvd::{get_volume_label, normalize_dvd_path};
use crate::ffmpeg::ProgressEvent;
use crate::history::{RipRecord, load_history, record_rip_event};
use crate::utils::format_episode_name;

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

    fn on_title_start(&self, title_num: u32) {
        let _ = self.tx.send(ProgressEvent::TvEpisodeStarted { title_num });
    }

    fn on_title_complete(&self, title_num: u32, output_path: &Path) {
        let _ = self.tx.send(ProgressEvent::TvEpisodeCompleted {
            title_num,
            output_path: output_path.to_path_buf(),
        });
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
    pub end_episode: Option<u32>,
    pub all_episodes: bool,
    pub detected_episodes: Vec<TvEpisodeInfo>,
    pub completed_episodes: HashSet<u32>,
    pub current_ripping_title: Option<u32>,
    pub current_disc_fingerprint: Option<String>,
    pub last_disc_poll: Option<Instant>,

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
    pub auto_crop: bool,
    pub detelecine: bool,
    pub start_chapter: Option<u32>,
    pub end_chapter: Option<u32>,
    pub split_chapters: bool,
    pub angle: Option<u32>,
    pub custom_crf: Option<u8>,
    pub spool_mode: bool,
    pub probed_stream_details: Option<crate::domain::entities::disc::TitleStreamDetails>,
    pub queued_jobs: Vec<crate::queue::JobItem>,
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

    pub controller: GuiApplicationController,
    pub event_tx: Sender<ProgressEvent>,

    pub history: Vec<RipRecord>,
}

impl Default for DvdRipperApp {
    fn default() -> Self {
        let container = Arc::new(crate::AppContainer::build_production("ffmpeg"));
        let controller = GuiApplicationController::new(container.clone());
        let event_tx = controller.event_tx();

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
            end_episode: None,
            all_episodes: false,
            detected_episodes: Vec::new(),
            completed_episodes: HashSet::new(),
            current_ripping_title: None,
            current_disc_fingerprint: None,
            last_disc_poll: None,

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
            auto_crop: false,
            detelecine: false,
            start_chapter: None,
            end_chapter: None,
            split_chapters: false,
            angle: None,
            custom_crf: None,
            spool_mode: false,
            probed_stream_details: None,
            queued_jobs: crate::queue::list_jobs(),
            container: Some(container),

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

            controller,
            event_tx,

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
        let controller = GuiApplicationController::new(container.clone());
        let event_tx = controller.event_tx();
        let mut app = Self {
            container: Some(container),
            controller,
            event_tx,
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
        let starting = if Path::new(&self.out_dir).exists() {
            Some(Path::new(&self.out_dir))
        } else {
            None
        };
        if let Some(folder) = self
            .controller
            .pick_folder(starting, "Select Default Output Folder for Ripped DVDs")
        {
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
        self.auto_crop = def.auto_crop;
        self.detelecine = def.detelecine;
        self.start_chapter = def.start_chapter;
        self.end_chapter = def.end_chapter;
        self.split_chapters = def.split_chapters;
        self.angle = def.angle;
        self.custom_crf = def.custom_crf;
        self.spool_mode = def.spool_mode;
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
            start_chapter: self.start_chapter,
            end_chapter: self.end_chapter,
            split_chapters: self.split_chapters,
            angle: self.angle,
            auto_crop: self.auto_crop,
            detelecine: self.detelecine,
            crf: self.custom_crf,
            spool: self.spool_mode,
            sector_retries: 2,
            target_size_mb: None,
            rip_extras: false,
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
        if !self.detected_episodes.is_empty() {
            let max_ep = self.start_episode + self.detected_episodes.len() as u32 - 1;
            if self
                .end_episode
                .is_none_or(|end| end < self.start_episode || end > max_ep)
            {
                self.end_episode = Some(max_ep);
            }
        }
        self.refresh_completed_episodes();
    }

    /// Checks whether a specific episode `(show_name, season, episode_num)` has already been ripped
    /// according to the ripping history or destination directory on disk.
    pub fn is_episode_ripped_in_history_or_disk(&self, episode_num: u32) -> bool {
        let show_name = self.film_name.trim();
        if show_name.is_empty() {
            return false;
        }
        let season = self.season_number;
        let formatted_name = format_episode_name(show_name, season, episode_num);
        let se_tag = format!("S{:02}E{:02}", season, episode_num);

        let in_history = self.history.iter().any(|rec| {
            (rec.status == "Success" || rec.status == "Completed")
                && (rec.title.contains(&formatted_name)
                    || (show_name.len() > 2
                        && rec.title.contains(show_name)
                        && (rec.title.contains(&se_tag) || rec.output_path.contains(&se_tag))))
        });
        if in_history {
            return true;
        }

        let yr = self.film_year.trim().parse::<u32>().ok();
        let dest_dir = crate::application::rip_tv_batch::resolve_tv_batch_destination_dir(
            std::path::Path::new(&self.out_dir),
            show_name,
            season,
            yr,
        );
        for ext in ["mp4", "mkv", "mpg"] {
            let ep_filename = format!("{}.{}", formatted_name, ext);
            if dest_dir.join(&ep_filename).exists() {
                return true;
            }
        }

        false
    }

    /// Advances `start_episode` past any episodes completed on the previous disc and clears
    /// per-disc completion state (`completed_episodes` and `title_thumbnails`).
    pub fn advance_start_episode_for_next_disc(&mut self) {
        if !self.completed_episodes.is_empty()
            && let Some(max_done_ep) = self
                .detected_episodes
                .iter()
                .filter(|ep| self.completed_episodes.contains(&ep.title_num))
                .map(|ep| ep.episode_num)
                .max()
            && self.start_episode <= max_done_ep
        {
            self.start_episode = max_done_ep + 1;
            self.end_episode = None;
        }

        if self.auto_boxset {
            let show_name = self.effective_show_name();
            let next_boxset_ep = self
                .controller
                .container()
                .boxset_repo
                .get_next_episode(&show_name, self.season_number);
            if next_boxset_ep > self.start_episode {
                self.start_episode = next_boxset_ep;
                self.end_episode = None;
            }
        }

        while self.is_episode_ripped_in_history_or_disk(self.start_episode)
            && self.start_episode < 999
        {
            self.start_episode += 1;
            self.end_episode = None;
        }

        self.completed_episodes.clear();
        self.current_ripping_title = None;
        self.title_thumbnails.clear();
    }

    /// Checks local file system and historical rip activity to mark previously ripped episodes as completed.
    pub fn refresh_completed_episodes(&mut self) {
        self.completed_episodes.clear();
        if self.detected_episodes.is_empty() || self.film_name.trim().is_empty() {
            return;
        }

        let completed_titles: Vec<u32> = self
            .detected_episodes
            .iter()
            .filter(|ep| self.is_episode_ripped_in_history_or_disk(ep.episode_num))
            .map(|ep| ep.title_num)
            .collect();

        for title_num in completed_titles {
            self.completed_episodes.insert(title_num);
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
        self.controller.start_search_lookup(imdb_id);
    }

    pub fn trigger_detection_with_query(&mut self, query_override: Option<String>) {
        if self.detecting || self.is_ripping {
            return;
        }

        if !self.completed_episodes.is_empty() {
            self.advance_start_episode_for_next_disc();
            self.update_detected_episode_names();
            self.completed_episodes.clear();
        }
        let dvd_path = normalize_dvd_path(&self.input_drive);
        if dvd_path.exists() {
            self.current_disc_fingerprint = Some(
                self.controller
                    .container()
                    .disc_inspector
                    .compute_fingerprint(&dvd_path),
            );
        }

        self.detecting = true;
        self.detect_status = "Searching metadata...".to_string();
        self.controller
            .start_detection(&self.input_drive, query_override);
    }

    pub fn trigger_tv_scan(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.advance_start_episode_for_next_disc();
        let dvd_path = normalize_dvd_path(&self.input_drive);
        if dvd_path.exists() {
            self.current_disc_fingerprint = Some(
                self.controller
                    .container()
                    .disc_inspector
                    .compute_fingerprint(&dvd_path),
            );
        }

        self.detecting = true;
        self.detect_status = "Scanning disc for TV episode titles...".to_string();
        self.controller.start_tv_scan(
            &self.input_drive,
            &self.effective_show_name(),
            self.season_number,
            self.start_episode,
        );
    }

    pub fn trigger_thumbnail_preview(&mut self, title_num: u32) {
        if self.is_fetching_thumb || self.is_ripping {
            return;
        }
        let target_title = if title_num == 0 { 1 } else { title_num };
        self.is_fetching_thumb = true;
        self.status_message = format!("Capturing preview thumbnail for Title #{}...", target_title);
        self.controller
            .start_thumbnail_preview(&self.input_drive, target_title);
    }

    pub fn trigger_benchmark(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Running 10-second optical drive throughput benchmark...".to_string();
        self.status_message = "Running drive benchmark...".to_string();
        self.controller.start_benchmark(&self.input_drive);
    }

    pub fn trigger_preview_clip(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }
        let mut args = crate::cli::Args::default();
        args.apply_encoding_options(self.build_encoding_options());
        args.transcode = true;
        args.preset = "veryfast".to_string();
        args.auto_crop = self.auto_crop;
        args.detelecine = self.detelecine;
        args.angle = self.angle;
        args.crf = self.custom_crf;
        self.status_message = "Generating 10s live video preview sample...".to_string();
        self.controller
            .start_preview_clip(&self.input_drive, self.title_number.max(1), args);
    }

    pub fn enqueue_current_disc(&mut self) {
        let title = self
            .film_name_opt()
            .unwrap_or_else(|| "Optical Disc".to_string());
        let media_type = if self.is_tv_mode {
            "TV Series"
        } else {
            "Movie"
        };
        let queue_repo = &self.controller.container().job_queue_repo;
        let _id = queue_repo.add_job(&title, media_type, &self.input_drive);
        self.queued_jobs = queue_repo.list_jobs();
        self.status_message = format!("Added '{}' to batch job queue.", title);
    }

    pub fn trigger_auto_crf(&mut self) {
        let dvd_path = normalize_dvd_path(&self.input_drive);
        let (tested_crf, mean_ssim) = self
            .controller
            .container()
            .video_analyzer
            .predict_optimal_crf(&dvd_path, self.title_number.max(1), &self.codec, 0.98);
        self.custom_crf = Some(tested_crf);
        self.status_message = format!(
            "Auto-CRF tuned to {} (target SSIM {:.1}%)",
            tested_crf,
            mean_ssim * 100.0
        );
    }

    pub fn start_next_queued_job(&mut self) {
        if self.is_ripping {
            return;
        }
        let queue_repo = self.controller.container().job_queue_repo.clone();
        if let Some(job) = queue_repo.next_ready_job() {
            queue_repo.update_job_status(&job.id, "Encoding...", 0.6);
            self.film_name = job.title.clone();
            self.is_tv_mode = job.media_type.to_lowercase().contains("tv");
            if let Some(ref spooled) = job.spool_path {
                self.input_drive = spooled.clone();
            } else {
                self.input_drive = job.drive.clone();
            }
            self.queued_jobs = queue_repo.list_jobs();
            self.start_ripping();
            let _ = queue_repo.remove_job(&job.id);
            let _ = crate::queue::cleanup_job_spool(&job.id, Path::new(&self.out_dir));
            self.queued_jobs = queue_repo.list_jobs();
        } else {
            self.status_message = "No queued jobs ready to process.".to_string();
        }
    }

    /// Resets per-episode/per-title ripping telemetry (elapsed timer, ETA, progress bar, FPS, and speed).
    pub fn reset_episode_rip_telemetry(&mut self) {
        self.start_time = None;
        self.eta_seconds = None;
        self.progress_percent = 0.0;
        self.fps = "N/A".to_string();
        self.speed = "N/A".to_string();
    }

    /// Computes a stable per-episode/per-title remaining time estimate in seconds.
    fn compute_episode_eta_seconds(&self, percent: f64, speed_str: &str) -> Option<f64> {
        if percent <= 0.01 {
            return None;
        }
        let fraction = (percent / 100.0).clamp(0.001, 1.0);
        if fraction >= 1.0 {
            return Some(0.0);
        }

        let speed_mult = speed_str
            .trim()
            .trim_end_matches('x')
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|&s| s > 0.05 && s <= 100.0);

        let active_duration_secs = if self.is_tv_mode {
            self.current_ripping_title
                .and_then(|t| {
                    self.detected_episodes
                        .iter()
                        .find(|ep| ep.title_num == t && !self.completed_episodes.contains(&ep.title_num))
                        .or_else(|| self.detected_episodes.iter().find(|ep| ep.title_num == t))
                        .map(|ep| ep.duration_secs)
                })
                .or(self.expected_runtime_secs)
        } else {
            self.expected_runtime_secs
        }
        .filter(|&d| d > 0.0);

        let speed_eta = match (active_duration_secs, speed_mult) {
            (Some(dur), Some(spd)) => Some((dur * (1.0 - fraction)) / spd),
            _ => None,
        };

        let elapsed_eta = self.start_time.map(|start| {
            let elapsed = start.elapsed().as_secs_f64();
            (1.0 - fraction) * (elapsed / fraction)
        });

        // During the initial 0..5% of an episode, elapsed wall-clock time includes DVD drive spin-up/seek
        // latency, which can inflate (elapsed / fraction) to hundreds of minutes. Prefer live speed-based ETA.
        if percent < 5.0
            && let Some(s_eta) = speed_eta
        {
            return Some(s_eta);
        }

        match (elapsed_eta, speed_eta) {
            (Some(e_eta), Some(s_eta)) => {
                // Guard against stale or seek-spiked elapsed estimates > 2.5x the speed-derived ETA
                if e_eta > s_eta * 2.5 && s_eta > 1.0 {
                    Some(s_eta)
                } else {
                    Some(e_eta)
                }
            }
            (Some(e_eta), None) => Some(e_eta),
            (None, Some(s_eta)) => Some(s_eta),
            (None, None) => None,
        }
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
        self.current_disc_fingerprint = Some(
            self.controller
                .container()
                .disc_inspector
                .compute_fingerprint(&dvd_path),
        );

        self.is_ripping = true;
        self.current_ripping_title = None;
        self.reset_episode_rip_telemetry();
        self.status_message = "Starting ripping process...".to_string();

        let filtered_episodes = if let Some(end_ep) = self.end_episode {
            self.detected_episodes
                .iter()
                .filter(|ep| ep.episode_num <= end_ep && ep.episode_num >= self.start_episode)
                .cloned()
                .collect()
        } else {
            self.detected_episodes.clone()
        };

        let spec = RipTaskSpecification {
            drive: self.input_drive.clone(),
            out_dir: self.out_dir.clone(),
            is_iso: self.iso_mode,
            is_tv: self.is_tv_mode,
            all_episodes: self.all_episodes,
            transcode: self.transcode,
            film_name: self.film_name_opt(),
            film_year: self.film_year.trim().parse::<u32>().ok(),
            season: self.season_number,
            start_episode: self.start_episode,
            title_number: self.title_number,
            preset: self.preset.clone(),
            ffmpeg_path: self.ffmpeg_path.clone(),
            expected_runtime_secs: self.expected_runtime_secs,
            detected_episodes: filtered_episodes,
            poster_bytes: self.raw_poster_bytes.clone(),
            encoding_options: self.build_encoding_options(),
        };

        self.controller.start_ripping(spec);
    }

    pub fn cancel_ripping(&mut self) {
        self.controller.cancel();
        self.is_ripping = false;
        self.detecting = false;
        self.current_ripping_title = None;
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
        let _ = crate::history::save_history(&self.history, None);
    }

    pub fn poll_events(&mut self, ctx: &egui::Context) {
        self.process_events();

        if !self.is_ripping && !self.detecting && !self.completed_episodes.is_empty() {
            let should_poll_disc = self
                .last_disc_poll
                .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(2));
            if should_poll_disc {
                self.last_disc_poll = Some(Instant::now());
                let dvd_path = normalize_dvd_path(&self.input_drive);
                let media_mounted = dvd_path.join("VIDEO_TS").exists()
                    || dvd_path.join("video_ts").exists()
                    || dvd_path.join("BDMV").exists()
                    || dvd_path.is_file();
                if media_mounted {
                    let new_fp = crate::dvd::compute_disc_fingerprint(&self.input_drive);
                    if self
                        .current_disc_fingerprint
                        .as_ref()
                        .is_some_and(|old_fp| old_fp != &new_fp)
                    {
                        self.current_disc_fingerprint = Some(new_fp);
                        self.advance_start_episode_for_next_disc();
                        self.update_detected_episode_names();
                        self.completed_episodes.clear();
                        self.status_message = format!(
                            "New disc detected — advanced to Episode #{} and reset completion marks.",
                            self.start_episode
                        );
                    }
                }
            }
        }

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
        while let Some(event) = self.controller.poll_event() {
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
                        if !self.completed_episodes.is_empty()
                            || self.is_episode_ripped_in_history_or_disk(self.start_episode)
                        {
                            self.advance_start_episode_for_next_disc();
                        }
                        self.update_detected_episode_names();
                        self.completed_episodes.clear();
                    }
                    self.detecting = false;
                    self.detect_status = "Detection complete.".to_string();
                }
                ProgressEvent::TvEpisodesDetected(episodes) => {
                    self.advance_start_episode_for_next_disc();
                    self.detected_episodes = episodes;
                    let show_name = self.effective_show_name();
                    let season = self.season_number;
                    for (ep_num, ep) in
                        (self.start_episode..).zip(self.detected_episodes.iter_mut())
                    {
                        if ep.episode_num != ep_num {
                            ep.episode_num = ep_num;
                            ep.formatted_name = format_episode_name(&show_name, season, ep_num);
                        }
                    }
                    if !self.detected_episodes.is_empty() {
                        let max_ep = self.start_episode + self.detected_episodes.len() as u32 - 1;
                        self.end_episode = Some(max_ep);
                    } else {
                        self.end_episode = None;
                    }
                    self.completed_episodes.clear();
                    self.detecting = false;
                    self.detect_status = format!(
                        "Found {} TV episode titles on disc.",
                        self.detected_episodes.len()
                    );
                }
                ProgressEvent::TvEpisodeStarted { title_num } => {
                    self.reset_episode_rip_telemetry();
                    self.current_ripping_title = Some(title_num);
                    if let Some(ep) = self
                        .detected_episodes
                        .iter()
                        .find(|e| e.title_num == title_num && !self.completed_episodes.contains(&e.title_num))
                        .or_else(|| self.detected_episodes.iter().find(|e| e.title_num == title_num))
                    {
                        self.status_message =
                            format!("Starting episode: {} (Title #{})...", ep.formatted_name, title_num);
                    }
                }
                ProgressEvent::TvEpisodeCompleted {
                    title_num,
                    output_path: _,
                } => {
                    self.completed_episodes.insert(title_num);
                    if self.current_ripping_title == Some(title_num) {
                        self.current_ripping_title = None;
                    }
                    self.reset_episode_rip_telemetry();
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
                ProgressEvent::TitleStreamsProbed(details) => {
                    self.probed_stream_details = Some(details);
                }
                ProgressEvent::PreviewClipReady(path) => {
                    self.status_message = format!("Preview sample ready: {}", path.display());
                    if let Some(parent) = path.parent() {
                        crate::utils::open_folder_in_explorer(parent);
                    }
                }
                ProgressEvent::Progress {
                    percent,
                    fps,
                    speed,
                } => {
                    if !self.is_ripping {
                        continue;
                    }
                    if percent <= 0.0 {
                        self.reset_episode_rip_telemetry();
                        self.fps = fps;
                        self.speed = speed;
                        continue;
                    }
                    if percent >= 100.0 {
                        self.progress_percent = 1.0;
                        self.fps = fps;
                        self.speed = speed;
                        self.eta_seconds = Some(0.0);
                        self.start_time = None;
                        self.status_message = "Ripping in progress... (100.0%)".to_string();
                        continue;
                    }

                    self.progress_percent = (percent as f32) / 100.0;
                    self.fps = fps;
                    self.speed = speed.clone();

                    if self.start_time.is_none() {
                        self.start_time = Some(Instant::now());
                    }

                    self.eta_seconds = self.compute_episode_eta_seconds(percent, &speed);

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

                    if let Some(active) = self.current_ripping_title {
                        self.completed_episodes.insert(active);
                    }
                    if self.is_tv_mode && self.title_number > 0 {
                        self.completed_episodes.insert(self.title_number);
                    }
                    self.current_ripping_title = None;

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
                    self.current_ripping_title = None;
                    self.start_time = None;
                    self.eta_seconds = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::media::FilmMetadata;
    use crate::domain::ports::transcoder::ProgressObserver;
    use std::path::PathBuf;

    #[test]
    fn test_gui_progress_observer() {
        let (tx, rx) = std::sync::mpsc::channel();
        let observer = GuiProgressObserver { tx };

        observer.on_progress(50.0, "30", "1.5x");
        observer.on_log("Transcode step");

        let ev1 = rx.recv().unwrap();
        assert!(matches!(ev1, ProgressEvent::Progress { .. }));

        let ev2 = rx.recv().unwrap();
        assert!(matches!(ev2, ProgressEvent::Log(_)));
    }

    #[test]
    fn test_dvd_ripper_app_defaults_and_helpers() {
        let mut app = DvdRipperApp::default();
        assert_eq!(app.film_name_opt(), None);
        assert_eq!(app.effective_show_name(), "TV Show");

        app.film_name = "  Inception  ".to_string();
        assert_eq!(app.film_name_opt().as_deref(), Some("Inception"));
        assert_eq!(app.effective_show_name(), "Inception");

        let encoding_opts = app.build_encoding_options();
        assert_eq!(encoding_opts.codec, "h264");
        assert_eq!(encoding_opts.profile, "standard");

        app.codec = "hevc".to_string();
        app.reset_to_defaults();
        assert_eq!(app.codec, "h264");

        app.save_current_output_dir_preference();
    }

    #[test]
    fn test_dvd_ripper_app_episode_names_update() {
        let mut app = DvdRipperApp::default();
        app.film_name = "Firefly".to_string();
        app.season_number = 1;
        app.start_episode = 1;

        app.detected_episodes = vec![
            TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                duration_secs: 2700.0,
                formatted_name: "Original 1".to_string(),
            },
            TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                duration_secs: 2600.0,
                formatted_name: "Original 2".to_string(),
            },
        ];

        app.update_detected_episode_names();
        assert_eq!(app.detected_episodes[0].formatted_name, "Firefly - S01E01");
        assert_eq!(app.detected_episodes[1].formatted_name, "Firefly - S01E02");
    }

    #[test]
    fn test_dvd_ripper_app_process_events_all_variants() {
        let mut app = DvdRipperApp::default();
        app.is_ripping = true;

        // Progress event
        let _ = app.event_tx.send(ProgressEvent::Progress {
            percent: 45.0,
            fps: "60.0".to_string(),
            speed: "2.5x".to_string(),
        });

        // Log event
        let _ = app
            .event_tx
            .send(ProgressEvent::Log("FFmpeg line output".to_string()));

        // SearchResults event
        let _ = app.event_tx.send(ProgressEvent::SearchResults(vec![
            crate::imdb::SearchResultItem {
                title: "Movie 1".to_string(),
                year: Some(2000),
                imdb_id: "tt1234567".to_string(),
                type_field: "movie".to_string(),
                poster_url: None,
            },
            crate::imdb::SearchResultItem {
                title: "Movie 2".to_string(),
                year: Some(2002),
                imdb_id: "tt7654321".to_string(),
                type_field: "movie".to_string(),
                poster_url: None,
            },
        ]));

        // Metadata event
        let _ = app.event_tx.send(ProgressEvent::Metadata(FilmMetadata {
            title: "Discovered Title".to_string(),
            year: Some(2005),
            runtime_secs: Some(7200.0),
            plot: Some("Plot desc".to_string()),
            genre: Some("Action".to_string()),
            director: Some("Director".to_string()),
            actors: Some("Actors".to_string()),
            rating: Some("8.0".to_string()),
            poster_bytes: None,
            is_series: false,
            ..Default::default()
        }));

        // DetectedTitle event
        let _ = app.event_tx.send(ProgressEvent::DetectedTitle {
            title_num: 3,
            duration_secs: 5400.0,
        });

        // TitleThumbnailReady event
        let _ = app.event_tx.send(ProgressEvent::TitleThumbnailReady {
            title_num: 3,
            image_bytes: vec![0, 1, 2],
        });

        // BenchmarkFinished event
        let _ = app.event_tx.send(ProgressEvent::BenchmarkFinished(
            crate::dvd::DriveBenchmarkReport {
                drive_path: "D:\\".to_string(),
                test_duration_secs: 10,
                read_bytes: 50_000_000,
                read_speed_mbps: 5.0,
                demux_speed_mbps: 4.8,
                fps: "120.0".to_string(),
                rating_summary: "Adequate Optical Read".to_string(),
            },
        ));

        // Success event
        let _ = app
            .event_tx
            .send(ProgressEvent::Success(PathBuf::from("C:\\Out\\Movie.mp4")));

        app.process_events();

        assert_eq!(app.progress_percent, 1.0);
        assert!(!app.is_ripping);
        assert_eq!(app.film_name, "Discovered Title");
        assert_eq!(app.film_year, "2005");
        assert_eq!(app.title_number, 3);
        assert!(app.benchmark_report.is_some());
    }

    #[test]
    fn test_dvd_ripper_app_headless_search_modal() {
        let mut app = DvdRipperApp::default();
        app.show_search_modal = true;
        app.search_candidates = vec![crate::imdb::SearchResultItem {
            title: "Test Movie".to_string(),
            year: Some(2021),
            imdb_id: "tt9876543".to_string(),
            type_field: "movie".to_string(),
            poster_url: None,
        }];

        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_search_modal(ctx);
        });

        assert!(app.show_search_modal);
    }

    #[test]
    fn test_dvd_ripper_app_with_container_and_triggers() {
        let container = Arc::new(crate::AppContainer::build_production("ffmpeg"));
        let mut app = DvdRipperApp::with_container(container);

        app.trigger_custom_search();
        app.trigger_tv_scan();
        app.trigger_thumbnail_preview(1);
        app.trigger_benchmark();

        let ctx = egui::Context::default();
        app.poll_events(&ctx);
    }

    #[test]
    fn test_tv_episode_events_and_refresh_completed() {
        let mut app = DvdRipperApp::default();
        app.film_name = "Blackadder".to_string();
        app.season_number = 2;
        app.detected_episodes = vec![
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                formatted_name: "Blackadder - S02E01".to_string(),
                duration_secs: 1800.0,
            },
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                formatted_name: "Blackadder - S02E02".to_string(),
                duration_secs: 1800.0,
            },
        ];

        // 1. Episode started
        let _ = app
            .event_tx
            .send(ProgressEvent::TvEpisodeStarted { title_num: 1 });
        app.process_events();
        assert_eq!(app.current_ripping_title, Some(1));
        assert!(!app.completed_episodes.contains(&1));

        // 2. Episode completed
        let _ = app.event_tx.send(ProgressEvent::TvEpisodeCompleted {
            title_num: 1,
            output_path: PathBuf::from("C:\\Out\\Blackadder - S02E01.mp4"),
        });
        app.process_events();
        assert_eq!(app.current_ripping_title, None);
        assert!(app.completed_episodes.contains(&1));
        assert!(!app.completed_episodes.contains(&2));

        // 3. Test refresh_completed_episodes with history
        app.history.push(crate::history::RipRecord::new(
            "Blackadder - S02E02",
            "TV",
            "C:\\Out\\Blackadder - S02E02.mp4",
            "Success",
        ));
        app.refresh_completed_episodes();
        assert!(app.completed_episodes.contains(&2));
    }

    #[test]
    fn test_tv_disc_change_clears_completed_and_advances_episodes() {
        let mut app = DvdRipperApp::default();
        app.film_name = "The Office".to_string();
        app.season_number = 1;
        app.start_episode = 1;
        app.detected_episodes = vec![
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                formatted_name: "The Office - S01E01".to_string(),
                duration_secs: 1320.0,
            },
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                formatted_name: "The Office - S01E02".to_string(),
                duration_secs: 1320.0,
            },
        ];

        // 1. Complete Disc 1 episodes (Titles #1 and #2)
        for t in [1, 2] {
            let _ = app.event_tx.send(ProgressEvent::TvEpisodeCompleted {
                title_num: t,
                output_path: PathBuf::from(format!("C:\\Out\\The Office - S01E0{}.mp4", t)),
            });
            app.history.push(crate::history::RipRecord::new(
                &format!("The Office - S01E0{}", t),
                "TV Series",
                &format!("C:\\Out\\The Office - S01E0{}.mp4", t),
                "Success",
            ));
        }
        app.process_events();
        assert!(app.completed_episodes.contains(&1));
        assert!(app.completed_episodes.contains(&2));

        // 2. Insert Disc 2 and receive TvEpisodesDetected (authored as Titles #1 and #2 on Disc 2)
        let _ = app.event_tx.send(ProgressEvent::TvEpisodesDetected(vec![
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                formatted_name: "The Office - S01E01".to_string(),
                duration_secs: 1320.0,
            },
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                formatted_name: "The Office - S01E02".to_string(),
                duration_secs: 1320.0,
            },
        ]));
        app.process_events();

        // Disc 2 must advance to S01E03..S01E04 and have NO episodes marked as completed
        assert_eq!(app.start_episode, 3);
        assert_eq!(app.end_episode, Some(4));
        assert_eq!(app.detected_episodes[0].episode_num, 3);
        assert_eq!(app.detected_episodes[0].formatted_name, "The Office - S01E03");
        assert_eq!(app.detected_episodes[1].episode_num, 4);
        assert_eq!(app.detected_episodes[1].formatted_name, "The Office - S01E04");
        assert!(app.completed_episodes.is_empty());

        // 3. Complete Disc 2 episodes, then manually change Start Ep # to 5 for Disc 3
        app.completed_episodes.insert(1);
        app.completed_episodes.insert(2);
        app.start_episode = 5;
        app.end_episode = None;
        app.update_detected_episode_names();
        assert_eq!(app.end_episode, Some(6));
        assert!(app.completed_episodes.is_empty());
    }

    #[test]
    fn test_tv_episode_transition_resets_eta_and_progress() {
        let mut app = DvdRipperApp::default();
        app.is_ripping = true;
        app.is_tv_mode = true;
        app.film_name = "Fawlty Towers".to_string();
        app.season_number = 1;
        app.detected_episodes = vec![
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                formatted_name: "Fawlty Towers - S01E01".to_string(),
                duration_secs: 1800.0,
            },
            crate::domain::entities::disc::TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                formatted_name: "Fawlty Towers - S01E02".to_string(),
                duration_secs: 1800.0,
            },
        ];

        // 1. Episode 1 starts and has been ripping for 25 minutes (1500s elapsed)
        let _ = app
            .event_tx
            .send(ProgressEvent::TvEpisodeStarted { title_num: 1 });
        app.process_events();
        app.start_time = Some(Instant::now() - std::time::Duration::from_secs(1500));

        let _ = app.event_tx.send(ProgressEvent::Progress {
            percent: 95.0,
            fps: "30.0".to_string(),
            speed: "1.2x".to_string(),
        });
        app.process_events();
        assert!(app.eta_seconds.is_some());

        // 2. Episode 1 reaches 100% and completes
        let _ = app.event_tx.send(ProgressEvent::Progress {
            percent: 100.0,
            fps: "N/A".to_string(),
            speed: "N/A".to_string(),
        });
        let _ = app.event_tx.send(ProgressEvent::TvEpisodeCompleted {
            title_num: 1,
            output_path: PathBuf::from("C:\\Out\\Fawlty Towers - S01E01.mp4"),
        });
        app.process_events();
        assert!(app.start_time.is_none());
        assert!(app.eta_seconds.is_none());
        assert_eq!(app.progress_percent, 0.0);

        // 3. Episode 2 starts and emits its first early progress event (0.5%)
        let _ = app
            .event_tx
            .send(ProgressEvent::TvEpisodeStarted { title_num: 2 });
        let _ = app.event_tx.send(ProgressEvent::Progress {
            percent: 0.5,
            fps: "30.0".to_string(),
            speed: "1.2x".to_string(),
        });
        app.process_events();

        assert_eq!(app.current_ripping_title, Some(2));
        let ep2_eta = app.eta_seconds.expect("Episode 2 should have a valid ETA");
        // 1800s episode at 1.2x speed should have an ETA of ~1492s (~25 mins), never 600 mins (36000s)
        assert!(
            ep2_eta > 1000.0 && ep2_eta < 2500.0,
            "Expected realistic Episode 2 ETA (~1492s), got {:.1}s",
            ep2_eta
        );
    }
}
