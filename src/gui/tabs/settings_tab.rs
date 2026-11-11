//! Global settings, preferences, and hardware configuration tab.
//! Follows Jakob Nielsen's Heuristic #7 (Flexibility & Efficiency of Use) and #10 (Help & Documentation).

use eframe::egui;

/// Renders the global settings tab view.
#[allow(clippy::too_many_arguments)]
pub fn render_settings_tab(
    ui: &mut egui::Ui,
    out_dir: &mut String,
    codec: &mut String,
    profile: &mut String,
    preset: &mut String,
    engine: &mut String,
    handbrake_path: &mut String,
    makemkv_path: &mut String,
    normalize_audio: &mut bool,
    norm_target: &mut f64,
    hwaccel_fallback: &mut bool,
    min_free_gb: &mut u64,
    webhook_url: &mut String,
    no_eject: &mut bool,
    checksum: &mut bool,
    nfo: &mut bool,
    on_browse_out_dir: impl FnOnce(),
    on_reset_defaults: impl FnOnce(),
) {
    ui.group(|ui| {
        ui.heading("⚙️ Application Settings & Preferences");
        ui.label("Configure persistent transcoding defaults, storage limits, and integration hooks.");
        ui.add_space(8.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("🚀 Ripping & Transcoding Engine").strong());
                ui.horizontal(|ui| {
                    ui.label("Active Engine:");
                    egui::ComboBox::from_id_salt("settings_engine_combo")
                        .selected_text(match engine.as_str() {
                            "handbrake" => "HandBrakeCLI (Optical Transcoder)",
                            "makemkv" => "MakeMKV (AACS/BD+/Lossless MKV)",
                            "native" => "Native IFO Cell Extractor (Pure Rust)",
                            "ffmpeg" => "FFmpeg (Standard)",
                            _ => "Auto (Smart Fallback: FFmpeg -> Native -> HandBrake -> MakeMKV)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(engine, "auto".to_string(), "Auto (Smart Fallback)");
                            ui.selectable_value(engine, "ffmpeg".to_string(), "FFmpeg (Standard)");
                            ui.selectable_value(engine, "native".to_string(), "Native IFO Cell Extractor (Pure Rust)");
                            ui.selectable_value(engine, "handbrake".to_string(), "HandBrakeCLI");
                            ui.selectable_value(engine, "makemkv".to_string(), "MakeMKV (makemkvcon)");
                        });
                });

                if engine == "handbrake" || engine == "auto" {
                    ui.horizontal(|ui| {
                        ui.label("HandBrakeCLI Path:");
                        ui.add(egui::TextEdit::singleline(handbrake_path).desired_width(260.0));
                    });
                }

                if engine == "makemkv" || engine == "auto" {
                    ui.horizontal(|ui| {
                        ui.label("MakeMKV (makemkvcon) Path:");
                        ui.add(egui::TextEdit::singleline(makemkv_path).desired_width(260.0));
                    });
                }
            });

            ui.add_space(6.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("📁 Storage & Paths").strong());
                ui.horizontal(|ui| {
                    ui.label("Default Output Directory:");
                    ui.add(egui::TextEdit::singleline(out_dir).desired_width(320.0));
                    if ui.button("📂 Browse...").clicked() {
                        on_browse_out_dir();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Minimum Free Disk Space Guard:");
                    ui.add(egui::DragValue::new(min_free_gb).range(1..=500).suffix(" GB"))
                        .on_hover_text("Aborts transcoding if free storage on target volume falls below this threshold");
                });
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("🎞️ Transcoding & Quality Defaults").strong());
                ui.horizontal(|ui| {
                    ui.label("Default Codec:");
                    egui::ComboBox::from_id_salt("settings_codec_combo")
                        .selected_text(codec.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(codec, "h264".to_string(), "H.264 (AVC)");
                            ui.selectable_value(codec, "hevc".to_string(), "HEVC (H.265)");
                            ui.selectable_value(codec, "av1".to_string(), "AV1 (SVT-AV1)");
                            ui.selectable_value(codec, "copy".to_string(), "Stream Copy (Lossless)");
                        });

                    ui.label("Default Profile:");
                    egui::ComboBox::from_id_salt("settings_profile_combo")
                        .selected_text(profile.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(profile, "standard".to_string(), "Standard");
                            ui.selectable_value(profile, "archival".to_string(), "Archival Lossless MKV");
                            ui.selectable_value(profile, "plex".to_string(), "Plex Direct Play");
                            ui.selectable_value(profile, "mobile".to_string(), "Mobile 720p");
                        });

                    ui.label("FFmpeg Speed Preset:");
                    egui::ComboBox::from_id_salt("settings_preset_combo")
                        .selected_text(preset.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(preset, "ultrafast".to_string(), "ultrafast");
                            ui.selectable_value(preset, "superfast".to_string(), "superfast");
                            ui.selectable_value(preset, "veryfast".to_string(), "veryfast");
                            ui.selectable_value(preset, "fast".to_string(), "fast");
                            ui.selectable_value(preset, "medium".to_string(), "medium");
                        });
                });

                ui.horizontal(|ui| {
                    ui.checkbox(hwaccel_fallback, "⚡ Enable GPU Hardware Acceleration (NVENC/VAAPI/QuickSync)")
                        .on_hover_text("Automatically falls back to software libx264/libx265 if GPU initialization fails");
                });
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("🔊 Audio & Post-Processing").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(normalize_audio, "EBU R128 Loudness Normalization")
                        .on_hover_text("Ensures balanced audio levels across action sequences and whispered dialogue");
                    if *normalize_audio {
                        ui.add(egui::Slider::new(norm_target, -30.0..=-10.0).text("Target LUFS"))
                            .on_hover_text("Broadcast standard is -23 LUFS; streaming platforms use -14 to -16 LUFS");
                    }
                });

                ui.horizontal(|ui| {
                    ui.checkbox(nfo, "📄 Generate Kodi / Jellyfin .nfo Metadata Files");
                    ui.checkbox(checksum, "🔒 Generate SHA-256 Checksums (.sha256)");
                    ui.checkbox(no_eject, "🚫 Do not eject disc upon completion");
                });
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("🔔 Automation & Webhooks").strong());
                ui.horizontal(|ui| {
                    ui.label("Webhook URL:");
                    ui.add(
                        egui::TextEdit::singleline(webhook_url)
                            .hint_text("https://discord.com/api/webhooks/...")
                            .desired_width(340.0),
                    )
                    .on_hover_text("Sends an automated HTTP POST notification when ripping starts, completes, or fails");
                });
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("↺ Reset to Factory Defaults"))
                    .on_hover_text("Restores all settings and transcoding profiles to standard defaults")
                    .clicked()
                {
                    on_reset_defaults();
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_settings_tab_headless() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut out_dir = "C:\\Videos".to_string();
                let mut codec = "h264".to_string();
                let mut profile = "standard".to_string();
                let mut preset = "slow".to_string();
                let mut engine = "auto".to_string();
                let mut handbrake_path = "HandBrakeCLI".to_string();
                let mut makemkv_path = "makemkvcon".to_string();
                let mut normalize_audio = true;
                let mut norm_target = -23.0;
                let mut hwaccel_fallback = true;
                let mut min_free_gb = 10u64;
                let mut webhook_url = "http://example.com/webhook".to_string();
                let mut no_eject = false;
                let mut checksum = true;
                let mut nfo = true;

                render_settings_tab(
                    ui,
                    &mut out_dir,
                    &mut codec,
                    &mut profile,
                    &mut preset,
                    &mut engine,
                    &mut handbrake_path,
                    &mut makemkv_path,
                    &mut normalize_audio,
                    &mut norm_target,
                    &mut hwaccel_fallback,
                    &mut min_free_gb,
                    &mut webhook_url,
                    &mut no_eject,
                    &mut checksum,
                    &mut nfo,
                    || {},
                    || {},
                );
            });
        });
    }
}
