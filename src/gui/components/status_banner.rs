//! Status pill badge, real-time telemetry, and progress bar widgets.
//! Follows Jakob Nielsen's Heuristic #1 (Visibility of System Status) and #8 (Aesthetic and Minimalist Design).

use eframe::egui;
use std::collections::VecDeque;
use std::time::Instant;

/// Status state of the optical disc and ripping engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemDriveState {
    Ready,
    Scanning,
    Ripping,
    NoDisc,
    Error,
}

impl SystemDriveState {
    pub fn badge(&self) -> (&'static str, egui::Color32) {
        match self {
            Self::Ready => ("🟢 READY", super::super::theme::SUCCESS_COLOR),
            Self::Scanning => ("🟡 SCANNING", super::super::theme::WARNING_COLOR),
            Self::Ripping => ("🔵 RIPPING", super::super::theme::ACCENT_COLOR),
            Self::NoDisc => ("⚪ NO DISC", super::super::theme::MUTED_TEXT),
            Self::Error => ("🔴 ERROR", super::super::theme::DANGER_COLOR),
        }
    }
}

/// Renders the drive status pill badge.
pub fn render_drive_status_pill(ui: &mut egui::Ui, state: SystemDriveState) {
    let (text, color) = state.badge();
    ui.label(
        egui::RichText::new(format!(" {} ", text))
            .strong()
            .small()
            .color(egui::Color32::BLACK)
            .background_color(color),
    );
}

/// Formats seconds into MM:SS format.
pub fn format_mm_ss(total_secs: u64) -> String {
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

/// Renders the complete telemetry card: progress bar, ETA, elapsed time, fps, and speed.
#[allow(clippy::too_many_arguments)]
pub fn render_telemetry_bar(
    ui: &mut egui::Ui,
    is_ripping: bool,
    progress_percent: f32,
    fps: &str,
    speed: &str,
    status_message: &str,
    start_time: Option<Instant>,
    eta_seconds: Option<f64>,
) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Progress & Telemetry").strong());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(start) = start_time
                    && is_ripping
                {
                    let elapsed = start.elapsed().as_secs();
                    ui.label(
                        egui::RichText::new(format!("⏱ Elapsed: {}", format_mm_ss(elapsed)))
                            .small()
                            .color(super::super::theme::MUTED_TEXT),
                    );
                }
            });
        });

        ui.add_space(4.0);

        let max_w = (ui.clip_rect().width() - 40.0).max(180.0);
        let avail_w = (ui.available_width() - 8.0).min(max_w);
        let pct_val = (progress_percent * 100.0).clamp(0.0, 100.0);

        let progress_text = if is_ripping {
            let eta_str = if let Some(eta) = eta_seconds
                && eta > 0.0
            {
                format!(" • ETA: {}", format_mm_ss(eta as u64))
            } else {
                String::new()
            };
            if progress_percent > 0.0 {
                format!("{:.1}%{}", pct_val, eta_str)
            } else {
                format!("Ripping... (0.0%){}", eta_str)
            }
        } else if progress_percent >= 1.0 {
            "100% (Completed)".to_string()
        } else if status_message.to_lowercase().contains("cancel") {
            "Cancelled (0%)".to_string()
        } else if progress_percent <= 0.0 {
            "Ready".to_string()
        } else {
            format!("{:.1}%", pct_val)
        };

        let progress_bar = egui::ProgressBar::new(progress_percent)
            .text(progress_text)
            .animate(is_ripping)
            .desired_width(avail_w);
        ui.add(progress_bar);

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(format!("FPS: {}", fps));
            ui.separator();
            ui.label(format!("Speed: {}", speed));
            ui.separator();
            ui.add(
                egui::Label::new(egui::RichText::new(status_message).small().color(
                    if status_message.to_lowercase().contains("error") {
                        super::super::theme::DANGER_COLOR
                    } else {
                        super::super::theme::MUTED_TEXT
                    },
                ))
                .truncate(),
            );
        });
    });
}

/// Renders a collapsible bottom drawer for logs.
pub fn render_collapsible_log_drawer(ui: &mut egui::Ui, logs: &VecDeque<String>) {
    egui::CollapsingHeader::new(format!("Console Logs ({})", logs.len()))
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("collapsible_log_console_scroll")
                .max_height(100.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for log in logs {
                        ui.label(egui::RichText::new(log).monospace().small());
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_mm_ss() {
        assert_eq!(format_mm_ss(0), "00:00");
        assert_eq!(format_mm_ss(59), "00:59");
        assert_eq!(format_mm_ss(60), "01:00");
        assert_eq!(format_mm_ss(3665), "61:05");
    }

    #[test]
    fn test_system_drive_state_badges() {
        let (txt, _) = SystemDriveState::Ready.badge();
        assert!(txt.contains("READY"));

        let (txt, _) = SystemDriveState::Scanning.badge();
        assert!(txt.contains("SCANNING"));

        let (txt, _) = SystemDriveState::Ripping.badge();
        assert!(txt.contains("RIPPING"));

        let (txt, _) = SystemDriveState::NoDisc.badge();
        assert!(txt.contains("NO DISC"));

        let (txt, _) = SystemDriveState::Error.badge();
        assert!(txt.contains("ERROR"));

        let cloned = SystemDriveState::Ready;
        assert_eq!(SystemDriveState::Ready, cloned);
        assert_eq!(
            format!("{:?}", SystemDriveState::Ready),
            format!("{:?}", cloned)
        );
    }

    #[test]
    fn test_headless_render_status_banner_and_telemetry() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_drive_status_pill(ui, SystemDriveState::Ready);
                render_drive_status_pill(ui, SystemDriveState::Scanning);
                render_drive_status_pill(ui, SystemDriveState::Ripping);
                render_drive_status_pill(ui, SystemDriveState::NoDisc);
                render_drive_status_pill(ui, SystemDriveState::Error);

                // Telemetry during ripping
                render_telemetry_bar(
                    ui,
                    true,
                    0.5,
                    "30.0",
                    "1.5x",
                    "Ripping Title #1...",
                    Some(Instant::now()),
                    Some(120.0),
                );

                // Telemetry completed
                render_telemetry_bar(
                    ui,
                    false,
                    1.0,
                    "N/A",
                    "N/A",
                    "Ripping completed successfully.",
                    None,
                    None,
                );

                // Telemetry cancelled
                render_telemetry_bar(
                    ui,
                    false,
                    0.0,
                    "N/A",
                    "N/A",
                    "Process cancelled",
                    None,
                    None,
                );

                // Telemetry error
                render_telemetry_bar(
                    ui,
                    false,
                    0.0,
                    "N/A",
                    "N/A",
                    "Error: drive failed",
                    None,
                    None,
                );

                let mut logs = VecDeque::new();
                logs.push_back("Log line 1".to_string());
                logs.push_back("Log line 2".to_string());
                render_collapsible_log_drawer(ui, &logs);
            });
        });
    }
}
