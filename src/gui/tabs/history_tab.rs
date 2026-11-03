//! Persistent ripping activity history tab.
//! Follows Jakob Nielsen's Heuristic #3 (User Control & Freedom) and #6 (Recognition Rather than Recall).

use crate::history::RipRecord;
use eframe::egui;

/// Renders the persistent history tab view.
pub fn render_history_tab(
    ui: &mut egui::Ui,
    history: &[RipRecord],
    filter_query: &mut String,
    on_request_clear: impl FnOnce(),
) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading(format!("📜 Ripping History Log ({})", history.len()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !history.is_empty()
                    && ui
                        .add(egui::Button::new(
                            egui::RichText::new("🗑 Clear History")
                                .color(super::super::theme::DANGER_COLOR),
                        ))
                        .on_hover_text("Safely clear all local ripping activity history records")
                        .clicked()
                {
                    on_request_clear();
                }
            });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("🔍 Filter Records:");
            ui.add(
                egui::TextEdit::singleline(filter_query)
                    .hint_text("Search by title, status, or path...")
                    .desired_width(260.0),
            );
            if !filter_query.is_empty() && ui.small_button("✖").clicked() {
                filter_query.clear();
            }
        });

        ui.add_space(8.0);

        if history.is_empty() {
            ui.label(
                egui::RichText::new("No past rip records found.")
                    .weak()
                    .italics(),
            );
        } else {
            let query = filter_query.trim().to_lowercase();
            let filtered: Vec<&RipRecord> = history
                .iter()
                .filter(|rec| {
                    if query.is_empty() {
                        true
                    } else {
                        rec.title.to_lowercase().contains(&query)
                            || rec.status.to_lowercase().contains(&query)
                            || rec.output_path.to_lowercase().contains(&query)
                    }
                })
                .collect();

            if filtered.is_empty() {
                ui.label(
                    egui::RichText::new("No records match the current filter query.")
                        .weak()
                        .italics(),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("history_tab_scroll")
                    .max_height(350.0)
                    .show(ui, |ui| {
                        for rec in filtered {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&rec.timestamp)
                                        .weak()
                                        .monospace()
                                        .small(),
                                );

                                let tag_color = if rec.media_type == "TV Series" {
                                    egui::Color32::from_rgb(100, 180, 240)
                                } else {
                                    egui::Color32::from_rgb(240, 180, 100)
                                };
                                ui.label(
                                    egui::RichText::new(format!("[{}]", rec.media_type))
                                        .color(tag_color)
                                        .small(),
                                );

                                let status_color = if rec.status == "Success" {
                                    super::super::theme::SUCCESS_COLOR
                                } else if rec.status.to_lowercase().contains("cancel") {
                                    super::super::theme::WARNING_COLOR
                                } else {
                                    super::super::theme::DANGER_COLOR
                                };
                                ui.label(
                                    egui::RichText::new(&rec.status)
                                        .color(status_color)
                                        .strong()
                                        .small(),
                                );

                                ui.label(egui::RichText::new(&rec.title).strong().small());

                                if ui
                                    .small_button("↗ Open")
                                    .on_hover_text("Open enclosing folder in file manager")
                                    .clicked()
                                {
                                    let p = std::path::Path::new(&rec.output_path);
                                    if p.exists() {
                                        crate::utils::open_folder_in_explorer(p);
                                    } else if let Some(parent) = p.parent()
                                        && parent.exists()
                                    {
                                        crate::utils::open_folder_in_explorer(parent);
                                    }
                                }

                                ui.label(
                                    egui::RichText::new(&rec.output_path)
                                        .weak()
                                        .monospace()
                                        .small(),
                                );
                            });
                        }
                    });
            }
        }
    });
}
