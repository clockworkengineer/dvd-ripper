//! Proactive disk space monitoring and pre-flight validation guards.
//! Follows Jakob Nielsen's Heuristic #5 (Error Prevention) and #9 (Help Users Recognize and Recover from Errors).

use eframe::egui;
use std::path::Path;

/// Renders a live free disk space gauge for the destination directory.
pub fn render_disk_space_gauge(ui: &mut egui::Ui, out_dir: &str, required_gb: u64) {
    let p = Path::new(out_dir);
    let target = if p.exists() {
        p
    } else {
        p.parent().unwrap_or(Path::new("."))
    };

    let free_gb = crate::utils::check_disk_space_guard(target, 0)
        .ok()
        .map(|_| {
            #[cfg(windows)]
            {
                let mut free_bytes = 0u64;
                let mut total_bytes = 0u64;
                let target_wide: Vec<u16> = target
                    .to_string_lossy()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                unsafe {
                    let _ = windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                        windows::core::PCWSTR(target_wide.as_ptr()),
                        Some(&mut free_bytes),
                        Some(&mut total_bytes),
                        None,
                    );
                }
                free_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            }
            #[cfg(not(windows))]
            {
                // Fallback for unix
                50.0
            }
        });

    if let Some(gb) = free_gb {
        let (color, icon) = if gb < (required_gb as f64).max(5.0) {
            (super::super::theme::DANGER_COLOR, "⚠️")
        } else if gb < (required_gb as f64 * 2.0).max(15.0) {
            (super::super::theme::WARNING_COLOR, "ℹ️")
        } else {
            (super::super::theme::SUCCESS_COLOR, "💾")
        };

        ui.label(
            egui::RichText::new(format!(
                "{} Free Disk Space: {:.1} GB ({} GB estimated needed)",
                icon, gb, required_gb
            ))
            .small()
            .color(color),
        );
    }
}

/// Evaluates pre-flight prerequisites before initiating a rip.
pub fn validate_preflight_checks(
    drive: &str,
    is_iso: bool,
    is_tv: bool,
    title: &str,
    out_dir: &str,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if drive.trim().is_empty() {
        errors.push(
            "No DVD input drive selected. Choose 'auto' or a valid optical drive.".to_string(),
        );
    }

    if !is_iso && title.trim().is_empty() {
        if is_tv {
            errors.push("TV Show Name cannot be empty. Please enter series title.".to_string());
        } else {
            errors.push("Movie Title cannot be empty. Enter a title or search OMDb.".to_string());
        }
    }

    if out_dir.trim().is_empty() {
        errors.push("Destination folder path is required.".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
