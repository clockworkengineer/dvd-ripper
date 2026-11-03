//! Single-title movie and 1:1 ISO dump extraction tab.
//! Supports Simple Mode vs Advanced Mode progressive disclosure.
//! Follows Jakob Nielsen's Heuristics #1, #2, #5, #7, #8.

use eframe::egui;
use std::collections::HashMap;

/// Renders the primary Rip Disc tab.
#[allow(clippy::too_many_arguments)]
pub fn render_rip_tab(
    ui: &mut egui::Ui,
    simple_mode: &mut bool,
    input_drive: &mut String,
    film_name: &mut String,
    film_year: &mut String,
    out_dir: &mut String,
    recent_out_dirs: &[String],
    title_number: &mut u32,
    iso_mode: &mut bool,
    mkv: &mut bool,
    transcode: &mut bool,
    profile: &mut String,
    codec: &mut String,
    preset: &mut String,
    deinterlace: &mut bool,
    deinterlace_algo: &mut String,
    denoise: &mut bool,
    hwaccel_fallback: &mut bool,
    all_audio: &mut bool,
    normalize_audio: &mut bool,
    norm_target: &mut f64,
    dual_audio: &mut bool,
    audio_lang: &mut String,
    subtitles: &mut bool,
    sub_lang: &mut String,
    sub_format: &mut String,
    sub_forced_only: &mut bool,
    sub_external_srt: &mut bool,
    ocr: &mut bool,
    ocr_lang: &mut String,
    poster_texture: Option<&egui::TextureHandle>,
    rating: &str,
    genre: &str,
    plot: &str,
    director: &str,
    actors: &str,
    title_thumbnails: &HashMap<u32, (Vec<u8>, Option<egui::TextureHandle>)>,
    is_busy: bool,
    on_detect_drive: impl FnOnce(),
    on_search_metadata: impl FnOnce(),
    on_browse_out_dir: impl FnOnce(),
    mut on_preview_thumb: impl FnMut(u32),
    on_start_rip: impl FnOnce(),
    on_cancel_rip: impl FnOnce(),
) {
    ui.horizontal(|ui| {
        ui.heading("🎬 Disc Extraction");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mode_btn_text = if *simple_mode {
                "Switch to 🛠️ Advanced Mode"
            } else {
                "Switch to ✨ Simple Mode"
            };
            if ui
                .button(egui::RichText::new(mode_btn_text).small())
                .clicked()
            {
                *simple_mode = !*simple_mode;
            }
        });
    });

    ui.add_space(6.0);

    // 1. Drive & Media Identification Card
    ui.group(|ui| {
        ui.label(egui::RichText::new("1. Optical Drive & Disc Source").strong());
        ui.horizontal(|ui| {
            ui.label("Drive:");
            ui.text_edit_singleline(input_drive);

            let detected = crate::dvd::detect_dvd_drives();
            egui::ComboBox::from_id_salt("rip_tab_drive_combo")
                .selected_text(if input_drive.is_empty() { "auto" } else { input_drive.as_str() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(input_drive, "auto".to_string(), "⚡ Auto-Detect");
                    for d in detected {
                        ui.selectable_value(input_drive, d.clone(), format!("💿 {}", d));
                    }
                });

            if ui
                .add_enabled(!is_busy, egui::Button::new("🔍 Detect DVD"))
                .on_hover_text("Inspect optical drive for inserted disc and query volume label")
                .clicked()
            {
                on_detect_drive();
            }

            ui.separator();
            ui.checkbox(iso_mode, "📀 1:1 Archival ISO Dump")
                .on_hover_text("Creates an exact bit-for-bit ISO copy without transcoding, preserving full menus and bonus content");
        });
    });

    ui.add_space(6.0);

    // 2. Film Metadata Card
    if !*iso_mode {
        ui.group(|ui| {
            ui.label(egui::RichText::new("2. Movie Identification & Artwork").strong());

            ui.horizontal(|ui| {
                if let Some(texture) = poster_texture {
                    ui.add(egui::Image::new(texture).max_height(100.0).rounding(4.0));
                }

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Movie Title:");
                        ui.add(egui::TextEdit::singleline(film_name).hint_text("e.g. Inception").desired_width(220.0));

                        ui.label("Year:");
                        ui.add(egui::TextEdit::singleline(film_year).hint_text("2010").desired_width(45.0));

                        if ui
                            .add_enabled(!is_busy, egui::Button::new("🔍 Search OMDb"))
                            .on_hover_text("Search OMDb / IMDb online for high-res posters, plot, genre, and exact release year")
                            .clicked()
                        {
                            on_search_metadata();
                        }
                    });

                    if !rating.is_empty() || !genre.is_empty() {
                        ui.horizontal(|ui| {
                            if !rating.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("⭐ {}/10", rating))
                                        .strong()
                                        .color(egui::Color32::from_rgb(245, 175, 25)),
                                );
                            }
                            if !genre.is_empty() {
                                ui.label(egui::RichText::new(genre).weak().small());
                            }
                        });
                    }

                    if !plot.is_empty() {
                        ui.label(egui::RichText::new(plot).italics().small());
                    }

                    if !director.is_empty() || !actors.is_empty() {
                        let mut info = Vec::new();
                        if !director.is_empty() {
                            info.push(format!("Director: {}", director));
                        }
                        if !actors.is_empty() {
                            info.push(format!("Cast: {}", actors));
                        }
                        ui.label(egui::RichText::new(info.join(" | ")).weak().small());
                    }
                });
            });
        });

        ui.add_space(6.0);
    }

    // 3. Destination Directory Card
    ui.group(|ui| {
        ui.label(egui::RichText::new("3. Destination Output Directory").strong());
        ui.horizontal(|ui| {
            ui.label("Folder:");
            ui.add(egui::TextEdit::singleline(out_dir).desired_width(300.0));

            if !recent_out_dirs.is_empty() {
                egui::ComboBox::from_id_salt("recent_dirs_combo")
                    .selected_text("Recent Folders ▾")
                    .show_ui(ui, |ui| {
                        for dir in recent_out_dirs {
                            if ui.selectable_label(out_dir == dir, dir).clicked() {
                                *out_dir = dir.clone();
                            }
                        }
                    });
            }

            if ui
                .button("📂 Browse...")
                .on_hover_text("Select custom destination folder")
                .clicked()
            {
                on_browse_out_dir();
            }

            if ui
                .button("↗ Open")
                .on_hover_text("Open current destination directory in File Explorer")
                .clicked()
            {
                let p = std::path::Path::new(out_dir.as_str());
                if p.exists() {
                    crate::utils::open_folder_in_explorer(p);
                }
            }
        });

        super::super::components::disk_guard::render_disk_space_gauge(ui, out_dir, 10);
    });

    ui.add_space(6.0);

    // 4. Quality & Transcoding Options Card
    if !*iso_mode {
        ui.group(|ui| {
            ui.label(egui::RichText::new("4. Quality & Video Profile").strong());

            ui.horizontal(|ui| {
                ui.label("Quality Profile:");
                egui::ComboBox::from_id_salt("rip_profile_combo")
                    .selected_text(profile.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            profile,
                            "standard".to_string(),
                            "Standard (Recommended - balanced MP4)",
                        );
                        ui.selectable_value(
                            profile,
                            "archival".to_string(),
                            "Archival Lossless (Remuxed MKV)",
                        );
                        ui.selectable_value(
                            profile,
                            "plex".to_string(),
                            "Plex Direct Play (Universal compatibility)",
                        );
                        ui.selectable_value(
                            profile,
                            "mobile".to_string(),
                            "Mobile 720p (Compact file size)",
                        );
                    });

                if *simple_mode {
                    ui.checkbox(mkv, "Use .mkv format");
                }
            });

            if !*simple_mode {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.checkbox(transcode, "Re-encode Video Streams");
                    ui.checkbox(mkv, "Matroska Container (.mkv)");

                    ui.label("Codec:");
                    egui::ComboBox::from_id_salt("rip_codec_combo")
                        .selected_text(codec.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(codec, "h264".to_string(), "H.264 (AVC)");
                            ui.selectable_value(codec, "hevc".to_string(), "HEVC (H.265)");
                            ui.selectable_value(codec, "av1".to_string(), "AV1 (SVT-AV1)");
                            ui.selectable_value(
                                codec,
                                "copy".to_string(),
                                "Stream Copy (Passthrough)",
                            );
                        });

                    ui.label("Speed:");
                    egui::ComboBox::from_id_salt("rip_preset_combo")
                        .selected_text(preset.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(preset, "ultrafast".to_string(), "ultrafast");
                            ui.selectable_value(preset, "veryfast".to_string(), "veryfast");
                            ui.selectable_value(preset, "fast".to_string(), "fast");
                            ui.selectable_value(preset, "medium".to_string(), "medium");
                        });
                });

                ui.horizontal(|ui| {
                    ui.checkbox(deinterlace, "🧹 Motion-Adaptive Deinterlacing");
                    if *deinterlace {
                        egui::ComboBox::from_id_salt("rip_deint_algo")
                            .selected_text(deinterlace_algo.as_str())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    deinterlace_algo,
                                    "bwdif".to_string(),
                                    "bwdif (Highest quality)",
                                );
                                ui.selectable_value(
                                    deinterlace_algo,
                                    "yadif".to_string(),
                                    "yadif (Fast standard)",
                                );
                                ui.selectable_value(
                                    deinterlace_algo,
                                    "w3fdif".to_string(),
                                    "w3fdif (Simple)",
                                );
                            });
                    }

                    ui.checkbox(denoise, "✨ Noise Reduction (hqdn3d)");
                    ui.checkbox(hwaccel_fallback, "⚡ GPU Acceleration");
                });

                ui.horizontal(|ui| {
                    ui.checkbox(all_audio, "All Audio Tracks");
                    ui.checkbox(normalize_audio, "🔊 Normalize Volume");
                    if *normalize_audio {
                        ui.add(egui::Slider::new(norm_target, -30.0..=-10.0).text("LUFS"));
                    }
                    ui.checkbox(dual_audio, "📻 Dual Audio");
                    ui.label("Lang:");
                    ui.add(
                        egui::TextEdit::singleline(audio_lang)
                            .hint_text("eng")
                            .desired_width(35.0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.checkbox(subtitles, "Extract Subtitles");
                    ui.checkbox(sub_forced_only, "Forced Only");
                    ui.checkbox(sub_external_srt, "External .srt");
                    ui.checkbox(ocr, "🔤 OCR (.srt)");
                    if *ocr {
                        ui.label("OCR Lang:");
                        ui.add(
                            egui::TextEdit::singleline(ocr_lang)
                                .hint_text("eng")
                                .desired_width(35.0),
                        );
                    }
                    ui.label("Sub Lang:");
                    ui.add(
                        egui::TextEdit::singleline(sub_lang)
                            .hint_text("eng")
                            .desired_width(35.0),
                    );
                    ui.label("Format:");
                    egui::ComboBox::from_id_salt("rip_sub_fmt")
                        .selected_text(if *sub_format == "subrip" {
                            "Text (.srt)"
                        } else {
                            "Bitmap (dvdsub)"
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                sub_format,
                                "dvdsub".to_string(),
                                "Bitmap (dvdsub)",
                            );
                            ui.selectable_value(sub_format, "subrip".to_string(), "Text (.srt)");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Title Selection #:");
                    ui.add(egui::DragValue::new(title_number).range(0..=99));
                    if *title_number == 0 {
                        ui.label(
                            egui::RichText::new("(Auto-rank feature film)")
                                .weak()
                                .small(),
                        );
                    }

                    let active = if *title_number == 0 { 1 } else { *title_number };
                    if let Some((_, Some(texture))) = title_thumbnails.get(&active) {
                        ui.add(egui::Image::new(texture).max_height(28.0).rounding(2.0))
                            .on_hover_text(format!("Title #{} video preview snapshot", active));
                    } else if ui
                        .small_button("📸 Preview")
                        .on_hover_text("Capture video preview thumbnail from this title")
                        .clicked()
                    {
                        on_preview_thumb(*title_number);
                    }
                });
            }
        });

        ui.add_space(8.0);
    }

    // 5. Execution Controls
    ui.horizontal(|ui| {
        let preflight = super::super::components::disk_guard::validate_preflight_checks(
            input_drive,
            *iso_mode,
            false,
            film_name,
            out_dir,
        );

        if !is_busy {
            let can_start = preflight.is_ok();
            let btn_text = if *iso_mode {
                "📀 Dump 1:1 ISO Image"
            } else {
                "▶ Start Rip"
            };
            let start_btn = ui.add_enabled(
                can_start,
                egui::Button::new(egui::RichText::new(btn_text).strong().color(if can_start {
                    super::super::theme::ACCENT_COLOR
                } else {
                    super::super::theme::MUTED_TEXT
                }))
                .min_size(egui::vec2(160.0, 32.0)),
            );

            if !can_start {
                if let Err(reasons) = preflight {
                    start_btn.on_hover_text(format!(
                        "⚠️ Prerequisites incomplete:\n• {}",
                        reasons.join("\n• ")
                    ));
                }
            } else if start_btn.clicked() {
                on_start_rip();
            }
        } else if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("⏹ Cancel Rip")
                        .strong()
                        .color(super::super::theme::DANGER_COLOR),
                )
                .min_size(egui::vec2(140.0, 32.0)),
            )
            .clicked()
        {
            on_cancel_rip();
        }
    });
}
