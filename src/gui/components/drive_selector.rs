//! Reusable drive selector combo box widget for GUI tabs (DRY).

use eframe::egui;

/// Renders a standardized drive input text box alongside an auto-detecting ComboBox.
pub fn render_drive_selector(
    ui: &mut egui::Ui,
    id_salt: &str,
    input_drive: &mut String,
) {
    ui.label("Drive:");
    ui.text_edit_singleline(input_drive);

    let detected = crate::dvd::detect_dvd_drives();
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(if input_drive.is_empty() {
            "auto"
        } else {
            input_drive.as_str()
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(input_drive, "auto".to_string(), "⚡ Auto-Detect");
            for d in detected {
                ui.selectable_value(input_drive, d.clone(), format!("💿 {}", d));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_drive_selector_headless() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut drive = "D:\\".to_string();
                render_drive_selector(ui, "test_drive_selector", &mut drive);
                assert_eq!(drive, "D:\\");
            });
        });
    }
}

