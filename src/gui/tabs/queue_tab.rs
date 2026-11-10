//! Decoupled Batch Job Queue & Fast-Spooling Tab.
//! Allows users to enqueue multiple optical discs, spool discs to NVMe/SSD so the tray frees up in minutes,
//! and reorder or manage background transcoding jobs.

use crate::queue::JobItem;
use eframe::egui;

/// Renders the batch job queue management tab.
#[allow(clippy::too_many_arguments)]
pub fn render_queue_tab(
    ui: &mut egui::Ui,
    jobs: &[JobItem],
    spool_mode: &mut bool,
    is_busy: bool,
    on_enqueue_current: impl FnOnce(),
    mut on_move_up: impl FnMut(&str),
    mut on_move_down: impl FnMut(&str),
    mut on_remove_job: impl FnMut(&str),
    on_clear_queue: impl FnOnce(),
) {
    ui.add_enabled_ui(!is_busy, |ui| {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading(format!("📋 Batch Job Queue ({})", jobs.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !jobs.is_empty()
                        && ui
                            .button(
                                egui::RichText::new("🗑 Clear Queue")
                                    .color(super::super::theme::DANGER_COLOR),
                            )
                            .on_hover_text("Remove all queued jobs")
                            .clicked()
                    {
                        on_clear_queue();
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Decouple fast optical disc reading (~5 mins) from CPU/GPU encoding so you can feed multiple discs back-to-back.",
                )
                .weak()
                .small(),
            );

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new("➕ Add Current Disc Configuration to Queue").strong())
                    .on_hover_text("Enqueue the currently configured Movie or TV Series disc")
                    .clicked()
                {
                    on_enqueue_current();
                }

                ui.separator();
                ui.checkbox(
                    spool_mode,
                    "⚡ Fast-Spool to SSD & Eject Immediately (Spool-Then-Encode)",
                )
                .on_hover_text(
                    "Dumps raw optical sectors to a temporary local spool folder at 16x speed and ejects the disc immediately before background transcoding",
                );
            });
        });

        ui.add_space(8.0);

        ui.group(|ui| {
            if jobs.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No jobs currently queued. Configure a disc in the '🎬 Rip Disc' or '📺 TV Box Sets' tab and click '➕ Add Current Disc Configuration to Queue'.",
                    )
                    .weak()
                    .italics(),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("queue_tab_scroll")
                    .max_height(340.0)
                    .show(ui, |ui| {
                        for (idx, job) in jobs.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("#{}", idx + 1))
                                        .monospace()
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(format!("[{}]", job.media_type))
                                        .color(super::super::theme::ACCENT_COLOR)
                                        .small(),
                                );
                                ui.label(egui::RichText::new(&job.title).strong());
                                ui.label(
                                    egui::RichText::new(format!("({})", job.drive))
                                        .weak()
                                        .small(),
                                );
                                ui.label(
                                    egui::RichText::new(&job.status)
                                        .color(egui::Color32::from_rgb(0, 200, 120))
                                        .small(),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("✖")
                                            .on_hover_text("Remove job from queue")
                                            .clicked()
                                        {
                                            on_remove_job(&job.id);
                                        }
                                        if ui
                                            .add_enabled(idx + 1 < jobs.len(), egui::Button::new("⬇").small())
                                            .on_hover_text("Move Down in priority")
                                            .clicked()
                                        {
                                            on_move_down(&job.id);
                                        }
                                        if ui
                                            .add_enabled(idx > 0, egui::Button::new("⬆").small())
                                            .on_hover_text("Move Up in priority")
                                            .clicked()
                                        {
                                            on_move_up(&job.id);
                                        }
                                    },
                                );
                            });
                            ui.separator();
                        }
                    });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_queue_tab_headless() {
        let ctx = egui::Context::default();
        let mut spool = true;
        let jobs = vec![
            JobItem {
                id: "job_1".to_string(),
                title: "The Matrix (1999)".to_string(),
                media_type: "Movie".to_string(),
                drive: "D:\\".to_string(),
                status: "Spooled (Ready to Encode)".to_string(),
                timestamp: "2026-11-10 12:00:00".to_string(),
                spool_path: Some("C:\\Out\\.spool\\job_1\\spooled_disc.iso".to_string()),
                output_path: None,
                progress: 0.5,
                priority: 1,
            },
            JobItem {
                id: "job_2".to_string(),
                title: "Aliens (1986)".to_string(),
                media_type: "Movie".to_string(),
                drive: "D:\\".to_string(),
                status: "Queued".to_string(),
                timestamp: "2026-11-10 12:05:00".to_string(),
                spool_path: None,
                output_path: None,
                progress: 0.0,
                priority: 1,
            },
        ];

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_queue_tab(
                    ui,
                    &[],
                    &mut spool,
                    false,
                    || {},
                    |_| {},
                    |_| {},
                    |_| {},
                    || {},
                );
                render_queue_tab(
                    ui,
                    &jobs,
                    &mut spool,
                    false,
                    || {},
                    |_| {},
                    |_| {},
                    |_| {},
                    || {},
                );
            });
        });
    }
}
