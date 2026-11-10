//! TV Series season box set multi-episode batch ripper tab.
//! Follows Jakob Nielsen's Heuristic #2 (Match Real World), #6 (Recognition), and #7 (Efficiency).

use crate::domain::entities::disc::TvEpisodeInfo;
use eframe::egui;
use std::collections::{HashMap, HashSet};

/// Renders the TV Series batch extraction tab.
#[allow(clippy::too_many_arguments)]
pub fn render_tv_tab(
    ui: &mut egui::Ui,
    show_name: &mut String,
    season_number: &mut u32,
    start_episode: &mut u32,
    end_episode: &mut Option<u32>,
    auto_boxset: &mut bool,
    all_episodes: &mut bool,
    detected_episodes: &[TvEpisodeInfo],
    completed_episodes: &HashSet<u32>,
    current_ripping_title: Option<u32>,
    title_thumbnails: &HashMap<u32, (Vec<u8>, Option<egui::TextureHandle>)>,
    is_busy: bool,
    on_episode_range_changed: impl FnOnce(),
    on_scan_disc: impl FnOnce(),
    mut on_preview_thumb: impl FnMut(u32),
    on_start_batch_rip: impl FnOnce(),
    on_cancel_rip: impl FnOnce(),
) {
    let mut range_changed = false;

    ui.group(|ui| {
        ui.heading("📺 TV Series Box Set Extraction");
        ui.label("Automatically discovers episodic titles, strips menu loops, and sequences multi-disc box sets.");
        ui.add_space(8.0);

        let effective_end = end_episode.unwrap_or_else(|| {
            if !detected_episodes.is_empty() {
                *start_episode + detected_episodes.len() as u32 - 1
            } else {
                *start_episode
            }
        });

        let included_count = detected_episodes
            .iter()
            .filter(|ep| ep.episode_num >= *start_episode && ep.episode_num <= effective_end)
            .count();

        ui.add_enabled_ui(!is_busy, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("1. Series Metadata & Episode Range").strong());
                ui.horizontal(|ui| {
                    ui.label("Show Title:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(show_name)
                                .hint_text("e.g. The Office")
                                .desired_width(180.0),
                        )
                        .changed()
                    {
                        range_changed = true;
                    }

                    ui.label("Season #:");
                    if ui
                        .add(egui::DragValue::new(season_number).range(1..=99))
                        .changed()
                    {
                        range_changed = true;
                    }

                    ui.label("Start Ep #:");
                    if ui
                        .add(egui::DragValue::new(start_episode).range(1..=99))
                        .on_hover_text("Starting episode number for this disc")
                        .changed()
                    {
                        *end_episode = None;
                        range_changed = true;
                    }

                    ui.label("End Ep #:");
                    let default_end = if !detected_episodes.is_empty() {
                        *start_episode + detected_episodes.len() as u32 - 1
                    } else {
                        *start_episode
                    };
                    let mut current_end = end_episode.unwrap_or(default_end);
                    if ui.add(egui::DragValue::new(&mut current_end).range(*start_episode..=99)).on_hover_text("Ending episode number to extract from this disc").changed() {
                        *end_episode = Some(current_end);
                        range_changed = true;
                    }

                    let effective_end = end_episode.unwrap_or(default_end);
                    let count = if effective_end >= *start_episode {
                        (effective_end - *start_episode) + 1
                    } else {
                        1
                    };

                    ui.label(
                        egui::RichText::new(format!("(S{:02}E{:02}..S{:02}E{:02} • {} eps)", season_number, start_episode, season_number, effective_end, count))
                            .small()
                            .strong()
                            .color(super::super::theme::ACCENT_COLOR),
                    );
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

            let done_count = detected_episodes
                .iter()
                .filter(|ep| completed_episodes.contains(&ep.title_num))
                .count();

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("2. Discovered Episode Tracks ({})", detected_episodes.len())).strong());
                    if !detected_episodes.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("• Selected: {} / {}", included_count, detected_episodes.len()))
                                .small()
                                .color(if included_count > 0 { super::super::theme::ACCENT_COLOR } else { egui::Color32::RED }),
                        );
                        if done_count > 0 {
                            ui.label(
                                egui::RichText::new(format!("• Done: {} / {}", done_count, detected_episodes.len()))
                                    .small()
                                    .strong()
                                    .color(egui::Color32::from_rgb(0, 220, 100)),
                            );
                        }
                    }
                });

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
                                let is_done = completed_episodes.contains(&ep.title_num);
                                let is_currently_ripping = is_busy && current_ripping_title == Some(ep.title_num);
                                let is_included = ep.episode_num >= *start_episode && ep.episode_num <= effective_end;

                                ui.horizontal(|ui| {
                                    if is_done {
                                        ui.label(
                                            egui::RichText::new("✅ Done")
                                                .color(egui::Color32::from_rgb(0, 220, 100))
                                                .strong(),
                                        )
                                        .on_hover_text("Episode successfully ripped");
                                    } else if is_currently_ripping {
                                        ui.label(
                                            egui::RichText::new("⏳ Ripping...")
                                                .color(super::super::theme::ACCENT_COLOR)
                                                .strong(),
                                        )
                                        .on_hover_text("Currently extracting and encoding this episode");
                                    } else if is_included {
                                        ui.label(
                                            egui::RichText::new("✓ Queued")
                                                .color(egui::Color32::from_rgb(100, 180, 255))
                                                .strong(),
                                        )
                                        .on_hover_text("Queued to rip in this batch");
                                    } else {
                                        ui.label(
                                            egui::RichText::new("✗ Excluded")
                                                .color(egui::Color32::from_rgb(160, 160, 160))
                                                .weak(),
                                        )
                                        .on_hover_text("Excluded (outside Start-End range)");
                                    }

                                    if let Some((_, Some(texture))) = title_thumbnails.get(&ep.title_num) {
                                        ui.add(egui::Image::new(texture).max_height(24.0).rounding(2.0));
                                    } else if ui.small_button("📸").on_hover_text("Capture visual thumbnail preview").clicked() {
                                        requested_thumb = Some(ep.title_num);
                                    }

                                    let text_style = if is_done {
                                        egui::RichText::new(format!(
                                            "• {} (Title #{}, {:.0} mins) [Done]",
                                            ep.formatted_name,
                                            ep.title_num,
                                            ep.duration_secs / 60.0
                                        ))
                                        .small()
                                        .color(egui::Color32::from_rgb(0, 220, 100))
                                        .strong()
                                    } else if is_currently_ripping {
                                        egui::RichText::new(format!(
                                            "• {} (Title #{}, {:.0} mins) [Ripping...]",
                                            ep.formatted_name,
                                            ep.title_num,
                                            ep.duration_secs / 60.0
                                        ))
                                        .small()
                                        .color(super::super::theme::ACCENT_COLOR)
                                        .strong()
                                    } else if is_included {
                                        egui::RichText::new(format!(
                                            "• {} (Title #{}, {:.0} mins)",
                                            ep.formatted_name,
                                            ep.title_num,
                                            ep.duration_secs / 60.0
                                        ))
                                        .small()
                                    } else {
                                        egui::RichText::new(format!(
                                            "• {} (Title #{}, {:.0} mins) [Excluded]",
                                            ep.formatted_name,
                                            ep.title_num,
                                            ep.duration_secs / 60.0
                                        ))
                                        .small()
                                        .strikethrough()
                                        .color(egui::Color32::from_rgb(140, 140, 140))
                                    };

                                    ui.label(text_style);
                                });
                            }
                        });

                    if let Some(t) = requested_thumb {
                        on_preview_thumb(t);
                    }
                }
            });
        });

        ui.add_space(10.0);

        if is_busy {
            let cancel_btn = ui.add(
                egui::Button::new(
                    egui::RichText::new("⏹ Cancel Ripping")
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(super::super::theme::DANGER_COLOR)
                .min_size(egui::vec2(220.0, 34.0)),
            )
            .on_hover_text("Abort the active batch ripping operation (Esc)");
            if cancel_btn.clicked() {
                on_cancel_rip();
            }
        } else {
            let can_rip = !show_name.trim().is_empty() && included_count > 0;
            let btn_text = if included_count > 0 && included_count < detected_episodes.len() {
                format!("▶ Batch Rip Episodes (S{:02}E{:02}..S{:02}E{:02} • {} tracks)", season_number, start_episode, season_number, effective_end, included_count)
            } else {
                format!("▶ Batch Rip All Episodes ({} tracks)", detected_episodes.len())
            };

            let mut rip_btn = ui.add_enabled(
                can_rip,
                egui::Button::new(
                    egui::RichText::new(btn_text)
                        .strong()
                        .color(if can_rip { super::super::theme::ACCENT_COLOR } else { super::super::theme::MUTED_TEXT }),
                )
                .min_size(egui::vec2(220.0, 32.0)),
            );

            if !can_rip {
                if show_name.trim().is_empty() {
                    rip_btn = rip_btn.on_hover_text("⚠️ Enter a show title before batch ripping");
                } else if detected_episodes.is_empty() {
                    rip_btn = rip_btn.on_hover_text("⚠️ Click 'Scan Disc for Episodes' to discover titles first");
                } else if included_count == 0 {
                    rip_btn = rip_btn.on_hover_text("⚠️ No episodes within the specified Start-End episode range");
                }
            }

            if can_rip && rip_btn.clicked() {
                on_start_batch_rip();
            }
        }

        if range_changed {
            on_episode_range_changed();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_tv_tab_headless() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut show_name = "Lost".to_string();
                let mut season = 1;
                let mut start_ep = 1;
                let mut end_ep = Some(4);
                let mut auto_boxset = true;
                let mut all_episodes = true;
                let title_thumbnails = HashMap::new();
                let mut completed = HashSet::new();
                completed.insert(1);

                // 1. Empty episodes
                render_tv_tab(
                    ui,
                    &mut show_name,
                    &mut season,
                    &mut start_ep,
                    &mut end_ep,
                    &mut auto_boxset,
                    &mut all_episodes,
                    &[],
                    &completed,
                    None,
                    &title_thumbnails,
                    false,
                    || {},
                    || {},
                    |_| {},
                    || {},
                    || {},
                );

                // 2. With detected episodes
                let episodes = vec![
                    TvEpisodeInfo {
                        title_num: 1,
                        episode_num: 1,
                        duration_secs: 2700.0,
                        formatted_name: "Pilot 1".to_string(),
                    },
                    TvEpisodeInfo {
                        title_num: 2,
                        episode_num: 2,
                        duration_secs: 2600.0,
                        formatted_name: "Pilot 2".to_string(),
                    },
                ];

                render_tv_tab(
                    ui,
                    &mut show_name,
                    &mut season,
                    &mut start_ep,
                    &mut end_ep,
                    &mut auto_boxset,
                    &mut all_episodes,
                    &episodes,
                    &completed,
                    Some(2),
                    &title_thumbnails,
                    false,
                    || {},
                    || {},
                    |_| {},
                    || {},
                    || {},
                );

                // 3. Busy state with empty show title
                let mut empty_show = String::new();
                render_tv_tab(
                    ui,
                    &mut empty_show,
                    &mut season,
                    &mut start_ep,
                    &mut end_ep,
                    &mut auto_boxset,
                    &mut all_episodes,
                    &[],
                    &completed,
                    None,
                    &title_thumbnails,
                    true,
                    || {},
                    || {},
                    |_| {},
                    || {},
                    || {},
                );
            });
        });
    }
}
