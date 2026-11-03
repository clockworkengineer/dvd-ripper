//! Optical drive read performance and speed benchmarking tab.
//! Follows Jakob Nielsen's Heuristic #1 (Visibility of System Status) and #8 (Minimalist Design).

use eframe::egui;

/// Renders the drive benchmark tab view.
pub fn render_benchmark_tab(
    ui: &mut egui::Ui,
    input_drive: &mut String,
    benchmark_report: Option<&crate::dvd::DriveBenchmarkReport>,
    is_busy: bool,
    on_run_benchmark: impl FnOnce(),
) {
    ui.group(|ui| {
        ui.heading("⚡ Optical Drive Read Benchmark");
        ui.label("Measures raw optical sector throughput to test drive read stability and mechanical health.");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Target Drive:");
            ui.text_edit_singleline(input_drive);

            let detected = crate::dvd::detect_dvd_drives();
            egui::ComboBox::from_id_salt("benchmark_drive_select")
                .selected_text(if input_drive.is_empty() { "auto" } else { input_drive.as_str() })
                .show_ui(ui, |ui| {
                    ui.selectable_value(input_drive, "auto".to_string(), "⚡ Auto-Detect");
                    for d in detected {
                        ui.selectable_value(input_drive, d.clone(), format!("💿 {}", d));
                    }
                });

            if ui
                .add_enabled(!is_busy, egui::Button::new(
                    egui::RichText::new("▶ Run 10s Benchmark").strong()
                ))
                .on_hover_text("Execute a 10-second sequential read benchmark against the loaded DVD")
                .clicked()
            {
                on_run_benchmark();
            }
        });

        ui.add_space(12.0);

        if let Some(report) = benchmark_report {
            ui.group(|ui| {
                ui.label(egui::RichText::new("📊 Benchmark Diagnostic Report").strong());
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Average Read Speed:");
                    ui.label(
                        egui::RichText::new(format!("{:.2} MB/s", report.read_speed_mbps))
                            .strong()
                            .color(super::super::theme::ACCENT_COLOR),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Drive Speed Rating:");
                    ui.label(
                        egui::RichText::new(&report.rating_summary)
                            .strong()
                            .color(super::super::theme::SUCCESS_COLOR),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Demux Throughput:");
                    ui.label(format!("{:.2} MB/s ({} FPS)", report.demux_speed_mbps, report.fps));
                });

                ui.horizontal(|ui| {
                    ui.label("Total Data Transferred:");
                    ui.label(format!("{:.1} MB", report.read_bytes as f64 / (1024.0 * 1024.0)));
                });

                ui.horizontal(|ui| {
                    ui.label("Benchmark Duration:");
                    ui.label(format!("{} seconds", report.test_duration_secs));
                });
            });
        } else {
            ui.label(egui::RichText::new("No benchmark has been run in this session. Insert a disc and click Run.").italics().weak());
        }
    });
}
