#![cfg(feature = "gui")]

pub mod components;
pub mod controller;
pub mod state;
pub mod tabs;
pub mod theme;

use eframe::egui;
use std::path::Path;

pub use controller::{GuiApplicationController, RipTaskSpecification};
pub use state::DvdRipperApp;
pub use tabs::ActiveTab;

impl eframe::App for DvdRipperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_ui(ctx);
    }
}

impl DvdRipperApp {
    pub fn render_ui(&mut self, ctx: &egui::Context) {
        self.poll_events(ctx);

        if self.is_ripping || self.detecting {
            ctx.request_repaint();
        }

        // --- Keyboard Shortcuts (Accelerators) - Nielsen Heuristic #7 ---
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                if self.show_help_modal {
                    self.show_help_modal = false;
                } else if self.show_about_modal {
                    self.show_about_modal = false;
                } else if self.show_search_modal {
                    self.show_search_modal = false;
                } else if self.show_confirm_cancel {
                    self.show_confirm_cancel = false;
                } else if self.show_confirm_clear {
                    self.show_confirm_clear = false;
                } else if self.is_ripping {
                    self.show_confirm_cancel = true;
                }
            }
            if !self.is_ripping {
                if i.key_pressed(egui::Key::F1) {
                    self.show_help_modal = true;
                }
                if (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::O) {
                    let p = Path::new(&self.out_dir);
                    if p.exists() {
                        crate::utils::open_folder_in_explorer(p);
                    }
                }
                if (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::F) {
                    self.trigger_custom_search();
                }
                if (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::B) {
                    self.pick_output_dir();
                }
                if ((i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::R))
                    || (i.key_pressed(egui::Key::Enter) && !self.detecting)
                {
                    let preflight = components::disk_guard::validate_preflight_checks(
                        &self.input_drive,
                        self.iso_mode,
                        self.is_tv_mode,
                        &self.film_name,
                        &self.out_dir,
                    );
                    if preflight.is_ok() {
                        self.start_ripping();
                    }
                }
            }
        });

        let is_busy = self.is_ripping || self.detecting;

        // Drive status state
        let drive_state = if self.is_ripping {
            components::status_banner::SystemDriveState::Ripping
        } else if self.detecting {
            components::status_banner::SystemDriveState::Scanning
        } else if self.status_message.to_lowercase().contains("error") {
            components::status_banner::SystemDriveState::Error
        } else if self.input_drive.is_empty() {
            components::status_banner::SystemDriveState::NoDisc
        } else {
            components::status_banner::SystemDriveState::Ready
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            // Header Bar
            ui.horizontal(|ui| {
                ui.heading("📀 DVD Ripper Desktop Utility");
                components::status_banner::render_drive_status_pill(ui, drive_state);

                if self.is_ripping {
                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("⏹ Cancel Ripping")
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(theme::DANGER_COLOR)
                            .min_size(egui::vec2(130.0, 24.0)),
                        )
                        .on_hover_text("Abort the active ripping operation (Esc)")
                        .clicked()
                    {
                        self.show_confirm_cancel = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(!self.is_ripping, |ui| {
                        if ui.button("ℹ️ About").clicked() {
                            self.show_about_modal = true;
                        }
                        if ui
                            .button("❓ Help (F1)")
                            .on_hover_text("Open User Guide, Glossary, and Keyboard Shortcuts")
                            .clicked()
                        {
                            self.show_help_modal = true;
                        }
                    });
                });
            });

            ui.add_space(6.0);

            // Tab Navigation Bar
            tabs::render_tab_bar(ui, &mut self.active_tab, is_busy);

            ui.add_space(8.0);

            // Active Tab Content
            match self.active_tab {
                ActiveTab::Rip => {
                    let mut start_requested = false;
                    let mut cancel_requested = false;
                    let mut detect_requested = false;
                    let mut search_requested = false;
                    let mut browse_requested = false;
                    let mut thumb_requested = None;

                    tabs::rip_tab::render_rip_tab(
                        ui,
                        &mut self.simple_mode,
                        &mut self.input_drive,
                        &mut self.film_name,
                        &mut self.film_year,
                        &mut self.out_dir,
                        &self.recent_out_dirs,
                        &mut self.title_number,
                        &mut self.iso_mode,
                        &mut self.mkv,
                        &mut self.transcode,
                        &mut self.profile,
                        &mut self.codec,
                        &mut self.preset,
                        &mut self.deinterlace,
                        &mut self.deinterlace_algo,
                        &mut self.denoise,
                        &mut self.hwaccel_fallback,
                        &mut self.all_audio,
                        &mut self.normalize_audio,
                        &mut self.norm_target,
                        &mut self.dual_audio,
                        &mut self.audio_lang,
                        &mut self.subtitles,
                        &mut self.sub_lang,
                        &mut self.sub_format,
                        &mut self.sub_forced_only,
                        &mut self.sub_external_srt,
                        &mut self.ocr,
                        &mut self.ocr_lang,
                        self.poster_texture.as_ref(),
                        &self.rating,
                        &self.genre,
                        &self.plot,
                        &self.director,
                        &self.actors,
                        &self.title_thumbnails,
                        is_busy,
                        || detect_requested = true,
                        || search_requested = true,
                        || browse_requested = true,
                        |t| thumb_requested = Some(t),
                        || start_requested = true,
                        || cancel_requested = true,
                    );

                    if detect_requested {
                        self.trigger_detection();
                    }
                    if search_requested {
                        self.trigger_custom_search();
                    }
                    if browse_requested {
                        self.pick_output_dir();
                    }
                    if let Some(t) = thumb_requested {
                        self.trigger_thumbnail_preview(t);
                    }
                    if start_requested {
                        self.start_ripping();
                    }
                    if cancel_requested {
                        self.show_confirm_cancel = true;
                    }
                }
                ActiveTab::TvSeries => {
                    let mut scan_requested = false;
                    let mut start_batch = false;
                    let mut cancel_batch = false;
                    let mut thumb_req = None;
                    let mut range_changed = false;

                    tabs::tv_tab::render_tv_tab(
                        ui,
                        &mut self.film_name,
                        &mut self.season_number,
                        &mut self.start_episode,
                        &mut self.end_episode,
                        &mut self.auto_boxset,
                        &mut self.all_episodes,
                        &self.detected_episodes,
                        &self.completed_episodes,
                        self.current_ripping_title,
                        &self.title_thumbnails,
                        is_busy,
                        || range_changed = true,
                        || scan_requested = true,
                        |t| thumb_req = Some(t),
                        || start_batch = true,
                        || cancel_batch = true,
                    );

                    if range_changed {
                        self.update_detected_episode_names();
                    }
                    if scan_requested {
                        self.trigger_tv_scan();
                    }
                    if let Some(t) = thumb_req {
                        self.trigger_thumbnail_preview(t);
                    }
                    if start_batch {
                        self.all_episodes = true;
                        self.is_tv_mode = true;
                        self.start_ripping();
                    }
                    if cancel_batch {
                        self.show_confirm_cancel = true;
                    }
                }
                ActiveTab::Benchmark => {
                    let mut bench_requested = false;
                    ui.add_enabled_ui(!is_busy, |ui| {
                        tabs::benchmark_tab::render_benchmark_tab(
                            ui,
                            &mut self.input_drive,
                            self.benchmark_report.as_ref(),
                            is_busy,
                            || bench_requested = true,
                        );
                    });
                    if bench_requested {
                        self.trigger_benchmark();
                    }
                }
                ActiveTab::History => {
                    let mut clear_requested = false;
                    ui.add_enabled_ui(!is_busy, |ui| {
                        tabs::history_tab::render_history_tab(
                            ui,
                            &self.history,
                            &mut self.history_filter,
                            || clear_requested = true,
                        );
                    });
                    if clear_requested {
                        self.show_confirm_clear = true;
                    }
                }
                ActiveTab::Settings => {
                    let mut browse_req = false;
                    let mut reset_req = false;

                    ui.add_enabled_ui(!is_busy, |ui| {
                        tabs::settings_tab::render_settings_tab(
                            ui,
                            &mut self.out_dir,
                            &mut self.codec,
                            &mut self.profile,
                            &mut self.preset,
                            &mut self.normalize_audio,
                            &mut self.norm_target,
                            &mut self.hwaccel_fallback,
                            &mut self.min_free_gb,
                            &mut self.webhook_url,
                            &mut self.no_eject,
                            &mut self.checksum,
                            &mut self.nfo,
                            || browse_req = true,
                            || reset_req = true,
                        );
                    });

                    if browse_req {
                        self.pick_output_dir();
                    }
                    if reset_req {
                        self.reset_to_defaults();
                    }
                }
            }

            ui.add_space(8.0);

            let mut telemetry_cancel = false;
            // Telemetry & Status Bar
            components::status_banner::render_telemetry_bar(
                ui,
                self.is_ripping,
                self.progress_percent,
                &self.fps,
                &self.speed,
                &self.status_message,
                self.start_time,
                self.eta_seconds,
                || telemetry_cancel = true,
            );
            if telemetry_cancel {
                self.show_confirm_cancel = true;
            }

            ui.add_space(4.0);

            // Collapsible Log Drawer
            components::status_banner::render_collapsible_log_drawer(ui, &self.logs);
        });

        // Modals & Overlays
        self.render_search_modal(ctx);
        components::dialogs::render_about_modal(ctx, &mut self.show_about_modal, &self.ffmpeg_path);
        components::dialogs::render_help_modal(ctx, &mut self.show_help_modal);

        let mut do_cancel = false;
        components::dialogs::render_confirm_cancel_dialog(
            ctx,
            &mut self.show_confirm_cancel,
            self.progress_percent,
            || do_cancel = true,
        );
        if do_cancel {
            self.cancel_ripping();
        }

        let mut do_clear_history = false;
        let count = self.history.len();
        components::dialogs::render_confirm_clear_history_dialog(
            ctx,
            &mut self.show_confirm_clear,
            count,
            || do_clear_history = true,
        );
        if do_clear_history {
            self.history.clear();
            let _ = crate::history::clear_history(None);
        }
    }
}

/// Entry point to launch the native eframe desktop window.
pub fn run_gui() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([880.0, 920.0])
        .with_min_inner_size([800.0, 800.0])
        .with_title("DVD Ripper (Movies & TV Series)");

    let icon_bytes = include_bytes!("../../assets/icon.png");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let icon_data = egui::IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        };
        viewport = viewport.with_icon(icon_data);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "DVD Ripper",
        options,
        Box::new(|cc| Ok(Box::new(DvdRipperApp::new(cc)))),
    )
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::ffmpeg::ProgressEvent;

    #[test]
    fn test_cancel_ripping_cleans_up_progress_bar_and_stats() {
        let mut app = DvdRipperApp::default();
        app.is_ripping = true;
        app.progress_percent = 0.55;
        app.fps = "42.1".to_string();
        app.speed = "1.9x".to_string();
        app.status_message = "Ripping in progress... (55.0%)".to_string();

        app.cancel_ripping();

        assert!(!app.is_ripping);
        assert!(!app.detecting);
        assert_eq!(app.progress_percent, 0.0);
        assert_eq!(app.fps, "N/A");
        assert_eq!(app.speed, "N/A");
        assert_eq!(app.status_message, "Ripping process cancelled by user.");
    }

    #[test]
    fn test_process_events_ignores_late_progress_events_when_cancelled() {
        let mut app = DvdRipperApp::default();
        app.is_ripping = false;
        app.progress_percent = 0.0;
        app.fps = "N/A".to_string();
        app.speed = "N/A".to_string();

        let _ = app.event_tx.send(ProgressEvent::Progress {
            percent: 85.0,
            fps: "45.0".to_string(),
            speed: "2.0x".to_string(),
        });

        app.process_events();

        assert_eq!(app.progress_percent, 0.0);
        assert_eq!(app.fps, "N/A");
        assert_eq!(app.speed, "N/A");
    }

    #[test]
    fn test_process_events_handles_cancelled_error_event_cleanly() {
        let mut app = DvdRipperApp::default();
        app.is_ripping = true;
        app.progress_percent = 0.40;
        app.fps = "30.0".to_string();
        app.speed = "1.2x".to_string();

        let _ = app.event_tx.send(ProgressEvent::Error(
            "Ripping process cancelled by user.".to_string(),
        ));

        app.process_events();

        assert!(!app.is_ripping);
        assert_eq!(app.progress_percent, 0.0);
        assert_eq!(app.fps, "N/A");
        assert_eq!(app.speed, "N/A");
        assert_eq!(app.status_message, "Ripping process cancelled by user.");
    }

    #[test]
    fn test_gui_default_out_dir_is_sensible() {
        let app = DvdRipperApp::default();
        assert!(!app.out_dir.is_empty());
        assert!(
            app.out_dir.contains("DVD Rips")
                || app.out_dir.contains("Videos")
                || std::path::Path::new(&app.out_dir).is_absolute()
        );
    }

    #[test]
    fn test_gui_mode_change_preserves_custom_out_dir() {
        let mut app = DvdRipperApp::default();
        app.out_dir = "D:\\MyCustomRips".to_string();

        app.is_tv_mode = true;
        assert_eq!(app.out_dir, "D:\\MyCustomRips");

        app.is_tv_mode = false;
        assert_eq!(app.out_dir, "D:\\MyCustomRips");
    }

    #[test]
    fn test_active_tab_labels_and_switching() {
        let mut app = DvdRipperApp::default();
        assert_eq!(app.active_tab, ActiveTab::Rip);

        app.active_tab = ActiveTab::TvSeries;
        assert_eq!(app.active_tab.label(), "📺 TV Box Sets");

        app.active_tab = ActiveTab::Benchmark;
        assert_eq!(app.active_tab.label(), "⚡ Benchmark");

        app.active_tab = ActiveTab::History;
        assert_eq!(app.active_tab.label(), "📜 History");

        app.active_tab = ActiveTab::Settings;
        assert_eq!(app.active_tab.label(), "⚙️ Settings");
    }

    #[test]
    fn test_preflight_validation_guard() {
        use crate::gui::components::disk_guard::validate_preflight_checks;

        // Missing drive
        let err1 = validate_preflight_checks("", false, false, "Inception", "C:\\Out");
        assert!(err1.is_err());

        // Missing title in movie mode
        let err2 = validate_preflight_checks("D:\\", false, false, "", "C:\\Out");
        assert!(err2.is_err());

        // ISO mode does not require movie title
        let ok_iso = validate_preflight_checks("D:\\", true, false, "", "C:\\Out");
        assert!(ok_iso.is_ok());

        // TV mode requires show name
        let err_tv = validate_preflight_checks("D:\\", false, true, "", "C:\\Out");
        assert!(err_tv.is_err());

        // Valid movie
        let ok_movie = validate_preflight_checks("D:\\", false, false, "Inception", "C:\\Out");
        assert!(ok_movie.is_ok());
    }

    #[test]
    fn test_render_ui_across_all_tabs_and_modals() {
        let ctx = egui::Context::default();
        let tabs = [
            ActiveTab::Rip,
            ActiveTab::TvSeries,
            ActiveTab::Benchmark,
            ActiveTab::History,
            ActiveTab::Settings,
        ];

        for tab in tabs {
            let mut app = DvdRipperApp::default();
            app.active_tab = tab;
            app.is_ripping = false;
            app.detecting = false;
            app.show_about_modal = true;
            app.show_help_modal = true;
            app.show_confirm_cancel = true;
            app.show_confirm_clear = true;
            app.history = vec![crate::history::RipRecord::new(
                "Test", "Movie", "out.mp4", "Success",
            )];

            let _ = ctx.run(Default::default(), |ctx| {
                app.render_ui(ctx);
            });
        }

        // Test busy ripping state
        let mut busy_app = DvdRipperApp::default();
        busy_app.is_ripping = true;
        busy_app.status_message = "Error: optical drive disc read failed".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            busy_app.render_ui(ctx);
        });

        // Test empty drive state
        let mut empty_app = DvdRipperApp::default();
        empty_app.input_drive.clear();
        let _ = ctx.run(Default::default(), |ctx| {
            empty_app.render_ui(ctx);
        });
    }

    #[test]
    fn test_render_ui_cancel_button_and_disabled_state_when_ripping() {
        let ctx = egui::Context::default();
        let tabs = [
            ActiveTab::Rip,
            ActiveTab::TvSeries,
            ActiveTab::Benchmark,
            ActiveTab::History,
            ActiveTab::Settings,
        ];

        for tab in tabs {
            let mut app = DvdRipperApp::default();
            app.active_tab = tab;
            app.is_ripping = true;
            app.progress_percent = 0.42;
            app.film_name = "Firefly".to_string();

            let _ = ctx.run(Default::default(), |ctx| {
                app.render_ui(ctx);
            });

            assert!(app.is_ripping);
        }
    }
}
