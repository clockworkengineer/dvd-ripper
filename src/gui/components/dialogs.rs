//! Modal dialogs and user assistance overlays.
//! Follows Jakob Nielsen's Heuristic #3 (User Control & Freedom) and #10 (Help & Documentation).

use eframe::egui;

/// Renders a confirmation dialog before cancelling an active rip.
pub fn render_confirm_cancel_dialog(
    ctx: &egui::Context,
    is_open: &mut bool,
    progress_percent: f32,
    on_confirm: impl FnOnce(),
) {
    if !*is_open {
        return;
    }

    egui::Window::new("⚠️ Confirm Operation Cancellation")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!(
                    "Ripping is currently {:.1}% complete.",
                    progress_percent * 100.0
                ))
                .strong(),
            );
            ui.label("Cancelling will terminate FFmpeg and delete incomplete files from the destination.");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("▶ Continue Ripping").clicked() {
                    *is_open = false;
                }

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("⏹ Yes, Cancel Rip").color(super::super::theme::DANGER_COLOR),
                    ))
                    .clicked()
                {
                    *is_open = false;
                    on_confirm();
                }
            });
            ui.add_space(4.0);
        });
}

/// Renders a confirmation dialog before clearing rip history records.
pub fn render_confirm_clear_history_dialog(
    ctx: &egui::Context,
    is_open: &mut bool,
    history_count: usize,
    on_confirm: impl FnOnce(),
) {
    if !*is_open {
        return;
    }

    egui::Window::new("🗑 Confirm Clear History")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!(
                    "Are you sure you want to permanently clear all {} history entries?",
                    history_count
                ))
                .strong(),
            );
            ui.label(
                "This action removes local record logs but does not delete output video files.",
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    *is_open = false;
                }

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("🗑 Clear All Records")
                            .color(super::super::theme::DANGER_COLOR),
                    ))
                    .clicked()
                {
                    *is_open = false;
                    on_confirm();
                }
            });
            ui.add_space(4.0);
        });
}

/// Renders the Interactive Help & Keyboard Shortcuts guide modal.
pub fn render_help_modal(ctx: &egui::Context, is_open: &mut bool) {
    if !*is_open {
        return;
    }

    egui::Window::new("❓ Help & Keyboard Shortcuts Guide")
        .collapsible(false)
        .resizable(true)
        .default_size([540.0, 420.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("DVD Ripper User Guide");
            ui.add_space(6.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("🚀 Quick Start in 3 Easy Steps").strong());
                    ui.label("1. Select Drive: Insert your DVD and verify the drive dropdown displays your disc.");
                    ui.label("2. Identify Content: Enter film name or show title, or click '🔍 Search' to pull OMDb metadata.");
                    ui.label("3. Choose Format & Rip: Select Standard (MP4) or MKV, then click '▶ Start Rip'.");
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("⌨️ Keyboard Accelerators (Shortcuts)").strong());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + R / Enter:").monospace());
                        ui.label("Start Rip (when prerequisites are met)");
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Escape:").monospace());
                        ui.label("Cancel rip prompt or dismiss open dialog");
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + O:").monospace());
                        ui.label("Open destination folder in File Explorer");
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + F:").monospace());
                        ui.label("Open OMDb online metadata search");
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ctrl + B:").monospace());
                        ui.label("Browse custom destination directory");
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("F1:").monospace());
                        ui.label("Open this Help & Keyboard Shortcuts dialog");
                    });
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.label(egui::RichText::new("📖 Plain-English Glossary of Media Terms").strong());
                    ui.label("• Deinterlacing (bwdif/yadif): Removes horizontal 'comb' lines common in broadcast TV recordings and older interlaced DVDs.");
                    ui.label("• Transcoding: Re-encodes MPEG-2 video into high-efficiency H.264/HEVC/AV1, reducing 8GB discs to ~1.5GB with zero perceptible quality loss.");
                    ui.label("• Stream Copy: Losslessly remuxes original MPEG-2 video into MKV without re-encoding, taking only ~3-5 minutes.");
                    ui.label("• Audio Normalization (EBU R128): Adjusts audio loudness to standard levels so quiet dialogue and loud explosions are balanced.");
                    ui.label("• 1:1 ISO Dump: Bit-for-bit disc archive including original DVD menus, extra features, and raw VOB streams.");
                });
            });

            ui.add_space(8.0);
            if ui.button("Close Guide").clicked() {
                *is_open = false;
            }
        });
}

/// Renders the About modal window.
pub fn render_about_modal(ctx: &egui::Context, is_open: &mut bool, ffmpeg_path: &str) {
    if !*is_open {
        return;
    }

    egui::Window::new("ℹ️ About DVD Ripper")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("📀 DVD Ripper Desktop Utility");
            ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).weak());
            ui.add_space(8.0);

            ui.label("A fast, portable DVD backup utility written in Rust.");
            ui.label("Features automated transcoding, TV box set episode sequencing,");
            ui.label("and online IMDb/OMDb metadata matching.");
            ui.add_space(8.0);

            ui.label(egui::RichText::new("License: MIT OR Apache-2.0").small());
            ui.label(
                egui::RichText::new(format!("FFmpeg Path: {}", ffmpeg_path))
                    .monospace()
                    .small(),
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.hyperlink_to(
                    "🌐 GitHub Repository",
                    "https://github.com/clockworkengineer/dvd-ripper",
                );
                ui.separator();
                ui.hyperlink_to("☕ Buy Me a Coffee", "https://buymeacoffee.com/roberttizz1");
            });

            ui.add_space(10.0);
            if ui.button("Close").clicked() {
                *is_open = false;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_render_dialogs() {
        let ctx = egui::Context::default();

        // 1. All closed
        let mut cancel_open = false;
        let mut clear_open = false;
        let mut help_open = false;
        let mut about_open = false;

        let _ = ctx.run(Default::default(), |ctx| {
            render_confirm_cancel_dialog(ctx, &mut cancel_open, 0.45, || {});
            render_confirm_clear_history_dialog(ctx, &mut clear_open, 5, || {});
            render_help_modal(ctx, &mut help_open);
            render_about_modal(ctx, &mut about_open, "ffmpeg");
        });

        // 2. All open
        cancel_open = true;
        clear_open = true;
        help_open = true;
        about_open = true;

        let mut confirmed_cancel = false;
        let mut confirmed_clear = false;

        let _ = ctx.run(Default::default(), |ctx| {
            render_confirm_cancel_dialog(ctx, &mut cancel_open, 0.75, || {
                confirmed_cancel = true;
            });
            render_confirm_clear_history_dialog(ctx, &mut clear_open, 10, || {
                confirmed_clear = true;
            });
            render_help_modal(ctx, &mut help_open);
            render_about_modal(ctx, &mut about_open, "ffmpeg");
        });

        assert!(!confirmed_cancel); // Click not simulated, but UI code executed
        assert!(!confirmed_clear);
    }
}
