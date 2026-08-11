/**
 * @file gui.rs
 * @brief Portable graphical user interface built with eframe/egui supporting Movies & TV Series discs.
 */

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use eframe::egui;

use crate::cli::Args;
use crate::dvd::{get_volume_label, normalize_dvd_path};
use crate::ffmpeg::{
    detect_tv_episodes, resolve_output_path, resolve_tv_output_path, run_ffmpeg_with_channel,
    ProgressEvent, TvEpisodeInfo,
};
use crate::imdb::lookup_film_details;
use crate::utils::{find_next_start_episode, infer_start_episode_from_label, sanitize_filename};

/// Main application state for the eframe GUI.
pub struct DvdRipperApp {
    input_drive: String,
    film_name: String,
    film_year: String,
    out_dir: String,
    title_number: u32,
    transcode: bool,
    preset: String,
    ffmpeg_path: String,
    expected_runtime_secs: Option<f64>,

    // TV Series mode options
    is_tv_mode: bool,
    season_number: u32,
    start_episode: u32,
    all_episodes: bool,
    detected_episodes: Vec<TvEpisodeInfo>,

    // Metadata fields
    plot: String,
    genre: String,
    director: String,
    actors: String,
    rating: String,
    raw_poster_bytes: Option<Vec<u8>>,
    poster_texture: Option<egui::TextureHandle>,

    detecting: bool,
    detect_status: String,

    is_ripping: bool,
    progress_percent: f32,
    fps: String,
    speed: String,
    status_message: String,
    logs: Vec<String>,

    event_tx: Sender<ProgressEvent>,
    event_rx: Receiver<ProgressEvent>,
    cancel_tx: Option<Sender<()>>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for DvdRipperApp {
    fn default() -> Self {
        let (event_tx, event_rx) = channel();
        Self {
            input_drive: "D:\\".to_string(),
            film_name: String::new(),
            film_year: String::new(),
            out_dir: "Films".to_string(),
            title_number: 0,
            transcode: false,
            preset: "veryfast".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            expected_runtime_secs: None,

            is_tv_mode: false,
            season_number: 1,
            start_episode: 1,
            all_episodes: true,
            detected_episodes: Vec::new(),

            plot: String::new(),
            genre: String::new(),
            director: String::new(),
            actors: String::new(),
            rating: String::new(),
            raw_poster_bytes: None,
            poster_texture: None,

            detecting: false,
            detect_status: String::new(),

            is_ripping: false,
            progress_percent: 0.0,
            fps: "N/A".to_string(),
            speed: "N/A".to_string(),
            status_message: "Ready to rip.".to_string(),
            logs: Vec::new(),

            event_tx,
            event_rx,
            cancel_tx: None,
            cancel_flag: None,
        }
    }
}

impl DvdRipperApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        app.trigger_detection();
        app
    }

    fn trigger_detection(&mut self) {
        self.trigger_detection_with_query(None);
    }

    fn check_and_auto_set_start_episode(&mut self) {
        if self.is_tv_mode {
            let dvd_path = normalize_dvd_path(&self.input_drive);
            if let Some(lbl) = get_volume_label(&dvd_path.to_string_lossy()) {
                if let Some(ep) = infer_start_episode_from_label(&lbl, 3) {
                    self.start_episode = ep;
                    self.detect_status = format!(
                        "Inferred Start Episode #{} directly from DVD label ({})",
                        self.start_episode, lbl
                    );
                    return;
                }
            }

            if !self.film_name.trim().is_empty() {
                let next_ep = find_next_start_episode(
                    &self.out_dir,
                    self.film_name.trim(),
                    self.film_year.trim().parse::<u32>().ok(),
                    self.season_number,
                );
                if next_ep > 1 {
                    self.start_episode = next_ep;
                    self.detect_status = format!(
                        "Found existing episodes in Season {:02}. Auto-starting next episode at #{}.",
                        self.season_number, self.start_episode
                    );
                }
            }
        }
    }

    fn trigger_custom_search(&mut self) {
        let query = self.film_name.trim().to_string();
        if query.is_empty() {
            self.detect_status = "Please enter a title/show name first (e.g. Doctor Who).".to_string();
            return;
        }
        self.trigger_detection_with_query(Some(query));
    }

    fn trigger_detection_with_query(&mut self, query_override: Option<String>) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Searching metadata...".to_string();

        let drive = self.input_drive.clone();
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let dvd_path = normalize_dvd_path(&drive);
            let search_term = if let Some(q) = query_override {
                q
            } else if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                let _ = tx.send(ProgressEvent::Log(format!("Detected Volume Label: {}", label)));
                label
            } else {
                let _ = tx.send(ProgressEvent::Log("Could not read volume label.".to_string()));
                let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
                return;
            };

            let _ = tx.send(ProgressEvent::Log(format!("Searching metadata for query: '{}'", search_term)));
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
        });
    }

    fn trigger_tv_scan(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Scanning disc for TV episode titles...".to_string();

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());

        let ffmpeg_path = self.ffmpeg_path.clone();
        let dvd_path = normalize_dvd_path(&self.input_drive);
        let show_name = if self.film_name.trim().is_empty() {
            "TV Show".to_string()
        } else {
            self.film_name.trim().to_string()
        };
        let season = self.season_number;
        let start_ep = self.start_episode;
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let episodes = detect_tv_episodes(&ffmpeg_path, &dvd_path, &show_name, season, start_ep, Some(&cancel_flag));
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = tx.send(ProgressEvent::Log("Disc episode scanning cancelled by user.".to_string()));
                return;
            }
            let _ = tx.send(ProgressEvent::Log(format!(
                "Scanned disc: found {} episode titles",
                episodes.len()
            )));
            let _ = tx.send(ProgressEvent::TvEpisodesDetected(episodes));
        });
    }

    fn start_ripping(&mut self) {
        if self.is_ripping {
            return;
        }

        let drive = self.input_drive.clone();
        let dvd_path = normalize_dvd_path(&drive);
        if !dvd_path.exists() {
            self.status_message = format!("Error: DVD path does not exist ({})", dvd_path.display());
            return;
        }

        self.is_ripping = true;
        self.progress_percent = 0.0;
        self.fps = "N/A".to_string();
        self.speed = "N/A".to_string();
        self.status_message = "Starting ripping process...".to_string();

        let (cancel_tx, cancel_rx) = channel();
        self.cancel_tx = Some(cancel_tx);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(cancel_flag.clone());

        let is_tv = self.is_tv_mode;
        let all_eps = self.all_episodes;
        let show_name_opt = if self.film_name.trim().is_empty() {
            None
        } else {
            Some(self.film_name.trim().to_string())
        };
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

        std::thread::spawn(move || {
            if is_tv && all_eps {
                let show_name = show_name_opt.as_deref().unwrap_or("TV Show");
                let eps = detect_tv_episodes(&ffmpeg_path, &dvd_path, show_name, season, start_ep, Some(&cancel_flag));

                if cancel_flag.load(Ordering::SeqCst) {
                    let _ = tx.send(ProgressEvent::Error("TV batch ripping cancelled by user.".to_string()));
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

                for (idx, ep) in eps.iter().enumerate() {
                    if cancel_flag.load(Ordering::SeqCst) {
                        let _ = tx.send(ProgressEvent::Error("TV batch ripping cancelled by user.".to_string()));
                        return;
                    }

                    let args = Args {
                        input: drive.clone(),
                        output: None,
                        out_dir: out_dir.clone(),
                        title: ep.title_num,
                        transcode,
                        preset: preset.clone(),
                        ffmpeg: ffmpeg_path.clone(),
                        cli: false,
                        tv: true,
                        season,
                        start_episode: ep.episode_num,
                        all_episodes: false,
                    };

                    match resolve_tv_output_path(&args, show_name_opt.as_deref(), year_opt, season, ep.episode_num) {
                        Ok(abs_out) => {
                            let ep_desc = format!(
                                "Episode {}/{} ({})",
                                idx + 1,
                                total,
                                ep.formatted_name
                            );
                            let _ = tx.send(ProgressEvent::Log(format!("Ripping: {}", ep_desc)));

                            if let Err(e) = run_ffmpeg_with_channel(
                                &args,
                                &dvd_path,
                                &abs_out,
                                &ep.formatted_name,
                                Some(ep.duration_secs),
                                Some(tx.clone()),
                                None,
                                Some(cancel_flag.clone()),
                            ) {
                                if cancel_flag.load(Ordering::SeqCst) {
                                    let _ = tx.send(ProgressEvent::Error("TV batch ripping cancelled by user.".to_string()));
                                } else {
                                    let _ = tx.send(ProgressEvent::Error(format!("Error ripping episode {}: {}", ep.episode_num, e)));
                                }
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(ProgressEvent::Error(format!("Path resolution error: {}", e)));
                            return;
                        }
                    }
                }

                let _ = tx.send(ProgressEvent::Success(PathBuf::from(format!(
                    "Completed batch ripping {} episodes to TV directory",
                    total
                ))));
            } else if is_tv {
                let show_name = show_name_opt.as_deref().unwrap_or("TV Show");
                let ep_num = if start_ep > 0 { start_ep } else { 1 };
                let args = Args {
                    input: drive.clone(),
                    output: None,
                    out_dir: out_dir.clone(),
                    title: title_num,
                    transcode,
                    preset: preset.clone(),
                    ffmpeg: ffmpeg_path.clone(),
                    cli: false,
                    tv: true,
                    season,
                    start_episode: ep_num,
                    all_episodes: false,
                };

                match resolve_tv_output_path(&args, show_name_opt.as_deref(), year_opt, season, ep_num) {
                    Ok(abs_out) => {
                        let ep_name = format!("{} - S{:02}E{:02}", show_name, season, ep_num);
                        if let Err(e) = run_ffmpeg_with_channel(
                            &args,
                            &dvd_path,
                            &abs_out,
                            &ep_name,
                            expected_runtime,
                            Some(tx.clone()),
                            Some(cancel_rx),
                            Some(cancel_flag.clone()),
                        ) {
                            let _ = tx.send(ProgressEvent::Error(format!("Ripping error: {}", e)));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(format!("Path resolution error: {}", e)));
                    }
                }
            } else {
                let args = Args {
                    input: drive.clone(),
                    output: None,
                    out_dir: out_dir.clone(),
                    title: title_num,
                    transcode,
                    preset: preset.clone(),
                    ffmpeg: ffmpeg_path.clone(),
                    cli: false,
                    tv: false,
                    season: 1,
                    start_episode: 1,
                    all_episodes: false,
                };

                match resolve_output_path(&args, show_name_opt.as_deref(), year_opt) {
                    Ok(abs_out) => {
                        let display_title = show_name_opt.unwrap_or_else(|| "Unknown DVD Title".to_string());
                        if let Err(e) = run_ffmpeg_with_channel(
                            &args,
                            &dvd_path,
                            &abs_out,
                            &display_title,
                            expected_runtime,
                            Some(tx.clone()),
                            Some(cancel_rx),
                            Some(cancel_flag.clone()),
                        ) {
                            let _ = tx.send(ProgressEvent::Error(format!("Ripping error: {}", e)));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Error(format!("Path resolution error: {}", e)));
                    }
                }
            }
        });
    }

    fn cancel_ripping(&mut self) {
        if let Some(flag) = self.cancel_flag.take() {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        self.is_ripping = false;
        self.detecting = false;
        self.status_message = "Ripping process cancelled by user.".to_string();
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ProgressEvent::Log(line) => {
                    if line == "META_FAIL" {
                        self.detecting = false;
                        self.detect_status = "Detection finished.".to_string();
                    } else {
                        self.logs.push(line);
                        if self.logs.len() > 500 {
                            self.logs.remove(0);
                        }
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
                        self.out_dir = "TV".to_string();
                        self.check_and_auto_set_start_episode();
                    }
                    self.detecting = false;
                    self.detect_status = "Detection complete.".to_string();
                }
                ProgressEvent::TvEpisodesDetected(episodes) => {
                    self.detected_episodes = episodes;
                    self.detecting = false;
                    self.detect_status = format!("Found {} TV episode titles on disc.", self.detected_episodes.len());
                }
                ProgressEvent::Progress { percent, fps, speed } => {
                    self.progress_percent = (percent as f32) / 100.0;
                    self.fps = fps;
                    self.speed = speed;
                    self.status_message = format!("Ripping in progress... ({:.1}%)", percent);
                }
                ProgressEvent::Success(path) => {
                    self.is_ripping = false;
                    self.progress_percent = 1.0;
                    self.status_message = format!("Success! Saved to {}", path.display());
                    self.cancel_tx = None;
                    self.cancel_flag = None;
                }
                ProgressEvent::Error(msg) => {
                    self.is_ripping = false;
                    self.status_message = format!("Stopped: {}", msg);
                    self.cancel_tx = None;
                    self.cancel_flag = None;
                }
            }
        }

        if self.poster_texture.is_none() {
            if let Some(ref bytes) = self.raw_poster_bytes {
                if let Ok(img) = image::load_from_memory(bytes) {
                    let size = [img.width() as usize, img.height() as usize];
                    let rgba = img.to_rgba8();
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
                    let texture = ctx.load_texture("poster_thumb", color_img, Default::default());
                    self.poster_texture = Some(texture);
                }
            }
        }
    }
}

impl eframe::App for DvdRipperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events(ctx);

        if self.is_ripping || self.detecting {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("📀 DVD Ripper Desktop Utility");
            ui.add_space(8.0);

            // Drive Selection Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("1. DVD Drive & Detection").strong());
                ui.horizontal(|ui| {
                    ui.label("Drive Path:");
                    ui.text_edit_singleline(&mut self.input_drive);
                    if ui.button("🔍 Detect DVD").clicked() && !self.detecting && !self.is_ripping {
                        self.trigger_detection();
                    }
                });
                if !self.detect_status.is_empty() {
                    ui.label(egui::RichText::new(&self.detect_status).italics().small());
                }
            });

            ui.add_space(8.0);

            // Metadata & Mode Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("2. Media Metadata & Mode Settings").strong());

                ui.horizontal(|ui| {
                    ui.label("Ripping Mode:");
                    if ui.radio_value(&mut self.is_tv_mode, false, "🎬 Movie").changed() {
                        self.out_dir = "Films".to_string();
                    }
                    if ui.radio_value(&mut self.is_tv_mode, true, "📺 TV Series").changed() {
                        self.out_dir = "TV".to_string();
                    }
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if let Some(ref texture) = self.poster_texture {
                        ui.add(egui::Image::new(texture).max_height(140.0).rounding(6.0));
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(if self.is_tv_mode { "Show Name:" } else { "Film Title:" });
                            ui.text_edit_singleline(&mut self.film_name);
                            ui.label("Year:");
                            ui.add(egui::TextEdit::singleline(&mut self.film_year).desired_width(50.0));
                            if ui.button("🔍 Search").clicked() && !self.detecting && !self.is_ripping {
                                self.trigger_custom_search();
                            }
                        });

                        if !self.rating.is_empty() || !self.genre.is_empty() {
                            ui.horizontal(|ui| {
                                if !self.rating.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!("⭐ {}/10", self.rating))
                                            .strong()
                                            .color(egui::Color32::from_rgb(245, 175, 25)),
                                    );
                                }
                                if !self.genre.is_empty() {
                                    ui.label(egui::RichText::new(&self.genre).weak().small());
                                }
                            });
                        }

                        if !self.plot.is_empty() {
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new(&self.plot).italics().small());
                        }

                        if !self.director.is_empty() || !self.actors.is_empty() {
                            let mut info = Vec::new();
                            if !self.director.is_empty() {
                                info.push(format!("Director: {}", self.director));
                            }
                            if !self.actors.is_empty() {
                                info.push(format!("Cast: {}", self.actors));
                            }
                            ui.label(egui::RichText::new(info.join(" | ")).weak().small());
                        }

                        ui.add_space(4.0);

                        if self.is_tv_mode {
                            ui.horizontal(|ui| {
                                ui.label("Season #:");
                                if ui.add(egui::DragValue::new(&mut self.season_number).range(1..=99)).changed() {
                                    self.check_and_auto_set_start_episode();
                                }
                                ui.label("Start Ep #:");
                                ui.add(egui::DragValue::new(&mut self.start_episode).range(1..=99));
                                if ui.button("🔄 Auto Ep").clicked() {
                                    self.check_and_auto_set_start_episode();
                                }
                                ui.checkbox(&mut self.all_episodes, "Rip All Episodes");
                                if ui.button("🔍 Scan Disc").clicked() && !self.detecting && !self.is_ripping {
                                    self.trigger_tv_scan();
                                }
                            });

                            if !self.detected_episodes.is_empty() {
                                ui.label(egui::RichText::new(format!("Detected Episodes ({}):", self.detected_episodes.len())).strong().small());
                                egui::ScrollArea::vertical().max_height(60.0).show(ui, |ui| {
                                    for ep in &self.detected_episodes {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "• {} (Title #{}, {:.0} mins)",
                                                ep.formatted_name,
                                                ep.title_num,
                                                ep.duration_secs / 60.0
                                            ))
                                            .small(),
                                        );
                                    }
                                });
                            }
                        }

                        ui.horizontal(|ui| {
                            ui.label("Output Root Directory:");
                            ui.text_edit_singleline(&mut self.out_dir);
                            if !self.is_tv_mode || !self.all_episodes {
                                ui.label("Title #:");
                                ui.add(egui::DragValue::new(&mut self.title_number).range(0..=99));
                                if self.title_number == 0 {
                                    ui.label(egui::RichText::new("(0 = Auto)").weak());
                                }
                            }
                        });
                    });
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.transcode, "Re-encode (H.264 / AAC)");
                    if self.transcode {
                        ui.label("Preset:");
                        egui::ComboBox::from_id_salt("preset_combo")
                            .selected_text(&self.preset)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.preset, "ultrafast".to_string(), "ultrafast");
                                ui.selectable_value(&mut self.preset, "superfast".to_string(), "superfast");
                                ui.selectable_value(&mut self.preset, "veryfast".to_string(), "veryfast");
                                ui.selectable_value(&mut self.preset, "fast".to_string(), "fast");
                                ui.selectable_value(&mut self.preset, "medium".to_string(), "medium");
                            });
                    } else {
                        ui.label(egui::RichText::new("(Fast Lossless Stream Copy)").weak());
                    }
                });
            });

            ui.add_space(8.0);

            // Action & Progress Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("3. Ripping Process").strong());

                ui.horizontal(|ui| {
                    if !self.is_ripping && !self.detecting {
                        let btn_label = if self.is_tv_mode && self.all_episodes {
                            "▶ Batch Rip All Episodes"
                        } else {
                            "▶ Start Rip"
                        };
                        if ui.add_sized([160.0, 32.0], egui::Button::new(btn_label)).clicked() {
                            self.start_ripping();
                        }
                    } else {
                        if ui.add_sized([120.0, 32.0], egui::Button::new("⏹ Cancel")).clicked() {
                            self.cancel_ripping();
                        }
                    }

                    ui.label(egui::RichText::new(&self.status_message).strong());
                });

                ui.add_space(6.0);

                let progress_bar = egui::ProgressBar::new(self.progress_percent)
                    .show_percentage()
                    .animate(self.is_ripping);
                ui.add(progress_bar);

                ui.horizontal(|ui| {
                    ui.label(format!("FPS: {}", self.fps));
                    ui.separator();
                    ui.label(format!("Speed: {}", self.speed));
                });
            });

            ui.add_space(8.0);

            // Log Console Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("Log Console").strong());
                egui::ScrollArea::vertical()
                    .max_height(140.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for log in &self.logs {
                            ui.label(egui::RichText::new(log).monospace().small());
                        }
                    });
            });
        });
    }
}

/// Entry point to launch the native eframe desktop window.
pub fn run_gui() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([680.0, 740.0])
            .with_title("DVD Ripper (Movies & TV Series)"),
        ..Default::default()
    };
    eframe::run_native(
        "DVD Ripper",
        options,
        Box::new(|cc| Ok(Box::new(DvdRipperApp::new(cc)))),
    )
}
