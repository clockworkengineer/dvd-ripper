//! TV Series season box set multi-episode batch ripper tab.
//! Follows Jakob Nielsen's Heuristic #2 (Match Real World), #6 (Recognition), and #7 (Efficiency).

use crate::domain::entities::disc::TvEpisodeInfo;
use eframe::egui;
use std::collections::HashMap;

/// Renders the TV Series batch extraction tab.
#[allow(clippy::too_many_arguments)]
pub fn render_tv_tab(
    ui: &mut egui::Ui,
    show_name: &mut String,
    season_number: &mut u32,
    start_episode: &mut u32,
    auto_boxset: &mut bool,
    all_episodes: &mut bool,
    detected_episodes: &[TvEpisodeInfo],
    title_thumbnails: &HashMap<u32, (Vec<u8>, Option<egui::TextureHandle>)>,
    is_busy: bool,
    on_scan_disc: impl FnOnce(),
    mut on_preview_thumb: impl FnMut(u32),
    on_start_batch_rip: impl FnOnce(),
) {
    ui.group(|ui| {
        ui.heading("📺 TV Series Box Set Extraction");
        ui.label("Automatically discovers episodic titles, strips menu loops, and sequences multi-disc box sets.");
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("1. Series Metadata & Season Identification").strong());
            ui.horizontal(|ui| {
                ui.label("Show Title:");
                ui.add(egui::TextEdit::singleline(show_name).hint_text("e.g. The Office").desired_width(220.0));

                ui.label("Season #:");
                ui.add(egui::DragValue::new(season_number).range(1..=99));

                ui.label("Start Episode #:");
                ui.add(egui::DragValue::new(start_episode).range(1..=99));
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.checkbox(auto_boxset, "📦 Auto BoxSet Mode")
                    .on_hover_text("Automatically remembers the last ripped episode number and increments across discs");

                ui.checkbox(all_episodes, "Extract All Discovered Episodes")
                    .on_hover_text("Rips every detected episodic title sequentially without manual confirmation");

                if ui
                    .add_enabled(!is_busy, egui::Button::new("🔍 Scan Disc for Episodes"))
                    .on_hover_text("Probes disc titles and clusters episodes by median duration")
                    .clicked()
                {
                    on_scan_disc();
                }
            });
        });

        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new(format!("2. Discovered Episode Tracks ({})", detected_episodes.len())).strong());

            if detected_episodes.is_empty() {
                ui.label(
                    egui::RichText::new("No episodic tracks identified yet. Click 'Scan Disc for Episodes' above.")
                        .weak()
                        .italics()
                        .small(),
                );
            } else {
                let mut requested_thumb = None;
                egui::ScrollArea::vertical()
                    .id_salt("tv_tab_episodes_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for ep in detected_episodes {
                            ui.horizontal(|ui| {
                                if let Some((_, Some(texture))) = title_thumbnails.get(&ep.title_num) {
                                    ui.add(egui::Image::new(texture).max_height(24.0).rounding(2.0));
                                } else if ui.small_button("📸").on_hover_text("Capture visual thumbnail preview").clicked() {
                                    requested_thumb = Some(ep.title_num);
                                }

                                ui.label(
                                    egui::RichText::new(format!(
                                        "• {} (Title #{}, {:.0} mins)",
                                        ep.formatted_name,
                                        ep.title_num,
                                        ep.duration_secs / 60.0
                                    ))
                                    .small(),
                                );
                            });
                        }
                    });

                if let Some(t) = requested_thumb {
                    on_preview_thumb(t);
                }
            }
        });

        ui.add_space(10.0);

        let can_rip = !is_busy && !show_name.trim().is_empty() && !detected_episodes.is_empty();
        let mut rip_btn = ui.add_enabled(
            can_rip,
            egui::Button::new(
                egui::RichText::new("▶ Batch Rip All Episodes")
                    .strong()
                    .color(if can_rip { super::super::theme::ACCENT_COLOR } else { super::super::theme::MUTED_TEXT }),
            )
            .min_size(egui::vec2(200.0, 32.0)),
        );

        if !can_rip {
            if show_name.trim().is_empty() {
                rip_btn = rip_btn.on_hover_text("⚠️ Enter a show title before batch ripping");
            } else if detected_episodes.is_empty() {
                rip_btn = rip_btn.on_hover_text("⚠️ Click 'Scan Disc for Episodes' to discover titles first");
            }
        }

        if can_rip && rip_btn.clicked() {
            on_start_batch_rip();
        }
    });
}
