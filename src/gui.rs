/**
 * @file gui.rs
 * @brief Portable graphical user interface built with eframe/egui.
 */

use std::sync::mpsc::{channel, Receiver, Sender};
use eframe::egui;

use crate::cli::Args;
use crate::dvd::{get_volume_label, normalize_dvd_path};
use crate::ffmpeg::{resolve_output_path, run_ffmpeg_with_channel, ProgressEvent};
use crate::imdb::lookup_film_details;
use crate::utils::sanitize_filename;

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

    // Movie metadata fields
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
        }
    }
}

impl DvdRipperApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        // Automatically attempt DVD volume label detection on startup
        app.trigger_detection();
        app
    }

    fn trigger_detection(&mut self) {
        if self.detecting || self.is_ripping {
            return;
        }

        self.detecting = true;
        self.detect_status = "Detecting DVD volume label & metadata...".to_string();

        let drive = self.input_drive.clone();
        let tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let dvd_path = normalize_dvd_path(&drive);
            if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                let _ = tx.send(ProgressEvent::Log(format!("Detected Volume Label: {}", label)));
                match lookup_film_details(&label) {
                    Ok(meta) => {
                        let clean = sanitize_filename(&meta.title);
                        let year_str = meta.year.map(|y| y.to_string()).unwrap_or_default();
                        let runtime_desc = meta
                            .runtime_secs
                            .map(|r| format!(" [Runtime: {:.0}m]", r / 60.0))
                            .unwrap_or_default();
                        let _ = tx.send(ProgressEvent::Log(format!(
                            "Auto-detected Film: {} ({}){}",
                            clean, year_str, runtime_desc
                        )));
                        let _ = tx.send(ProgressEvent::Metadata(meta));
                    }
                    Err(e) => {
                        let _ = tx.send(ProgressEvent::Log(format!("Lookup warning: {}", e)));
                        let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
                    }
                }
            } else {
                let _ = tx.send(ProgressEvent::Log("Could not read volume label.".to_string()));
                let _ = tx.send(ProgressEvent::Log("META_FAIL".to_string()));
            }
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
        self.status_message = "Starting FFmpeg process...".to_string();

        let (cancel_tx, cancel_rx) = channel();
        self.cancel_tx = Some(cancel_tx);

        let film_name_opt = if self.film_name.trim().is_empty() {
            None
        } else {
            Some(self.film_name.trim().to_string())
        };

        let film_year_opt = self.film_year.trim().parse::<u32>().ok();

        let args = Args {
            input: drive.clone(),
            output: None,
            out_dir: self.out_dir.clone(),
            title: self.title_number,
            transcode: self.transcode,
            preset: self.preset.clone(),
            ffmpeg: self.ffmpeg_path.clone(),
            cli: false,
        };

        let tx = self.event_tx.clone();
        let expected_runtime = self.expected_runtime_secs;

        std::thread::spawn(move || {
            let res = resolve_output_path(&args, film_name_opt.as_deref(), film_year_opt);
            match res {
                Ok(abs_out) => {
                    let display_title = film_name_opt.unwrap_or_else(|| "Unknown DVD Title".to_string());
                    if let Err(e) = run_ffmpeg_with_channel(
                        &args,
                        &dvd_path,
                        &abs_out,
                        &display_title,
                        expected_runtime,
                        Some(tx.clone()),
                        Some(cancel_rx),
                    ) {
                        let _ = tx.send(ProgressEvent::Error(format!("Ripping error: {}", e)));
                    }
                }
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Error(format!("Path resolution error: {}", e)));
                }
            }
        });
    }

    fn cancel_ripping(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
            self.status_message = "Cancelling ripping process...".to_string();
        }
    }

    fn poll_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ProgressEvent::Log(line) => {
                    if line == "META_FAIL" {
                        self.detecting = false;
                        self.detect_status = "Detection finished (no metadata matched).".to_string();
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
                    self.detecting = false;
                    self.detect_status = "Detection complete.".to_string();
                }
                ProgressEvent::Progress { percent, fps, speed } => {
                    self.progress_percent = (percent as f32) / 100.0;
                    self.fps = fps;
                    self.speed = speed;
                    self.status_message = format!(
                        "Ripping in progress... ({:.1}%)",
                        percent
                    );
                }
                ProgressEvent::Success(path) => {
                    self.is_ripping = false;
                    self.progress_percent = 1.0;
                    self.status_message = format!("Success! Saved to {}", path.display());
                    self.cancel_tx = None;
                }
                ProgressEvent::Error(msg) => {
                    self.is_ripping = false;
                    self.status_message = format!("Failed: {}", msg);
                    self.cancel_tx = None;
                }
            }
        }

        // Lazy load texture on GUI thread when raw bytes are available
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

            // Metadata & Config Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("2. Film Metadata & Output Settings").strong());

                ui.horizontal(|ui| {
                    if let Some(ref texture) = self.poster_texture {
                        ui.add(egui::Image::new(texture).max_height(140.0).rounding(6.0));
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Film Title:");
                            ui.text_edit_singleline(&mut self.film_name);
                            ui.label("Year:");
                            ui.add(egui::TextEdit::singleline(&mut self.film_year).desired_width(60.0));
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

                        ui.horizontal(|ui| {
                            ui.label("Output Directory:");
                            ui.text_edit_singleline(&mut self.out_dir);
                            ui.label("Title #:");
                            ui.add(egui::DragValue::new(&mut self.title_number).range(0..=99));
                            if self.title_number == 0 {
                                ui.label(egui::RichText::new("(0 = Auto Main)").weak());
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
                    if !self.is_ripping {
                        if ui.add_sized([120.0, 32.0], egui::Button::new("▶ Start Rip")).clicked() {
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
            .with_inner_size([620.0, 680.0])
            .with_title("DVD Ripper"),
        ..Default::default()
    };
    eframe::run_native(
        "DVD Ripper",
        options,
        Box::new(|cc| Ok(Box::new(DvdRipperApp::new(cc)))),
    )
}
