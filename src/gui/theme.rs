//! Design tokens, color palette, and styling constants for the DVD Ripper GUI.
//! Follows Jakob Nielsen's Heuristic #4 (Consistency and Standards) and #8 (Aesthetic and Minimalist Design).

use eframe::egui::Color32;

pub const ACCENT_COLOR: Color32 = Color32::from_rgb(56, 189, 248); // #38bdf8 - Active blue/cyan
pub const SUCCESS_COLOR: Color32 = Color32::from_rgb(34, 197, 94); // #22c55e - Completed / healthy green
pub const WARNING_COLOR: Color32 = Color32::from_rgb(245, 158, 11); // #f59e0b - Scanning / caution amber
pub const DANGER_COLOR: Color32 = Color32::from_rgb(239, 68, 68); // #ef4444 - Error / cancelled red
pub const MUTED_TEXT: Color32 = Color32::from_rgb(148, 163, 184); // #94a3b8 - Secondary slate
pub const CARD_BG: Color32 = Color32::from_rgb(30, 41, 59); // #1e293b - Slate card background
pub const BORDER_COLOR: Color32 = Color32::from_rgb(51, 65, 85); // #334155 - Subtle border

pub const PRIMARY_BTN_SIZE: [f32; 2] = [160.0, 32.0];
pub const SECONDARY_BTN_SIZE: [f32; 2] = [120.0, 28.0];
pub const SMALL_BTN_SIZE: [f32; 2] = [28.0, 24.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_constants() {
        assert_ne!(ACCENT_COLOR, DANGER_COLOR);
        assert_ne!(SUCCESS_COLOR, WARNING_COLOR);
        assert_eq!(PRIMARY_BTN_SIZE, [160.0, 32.0]);
        assert_eq!(SECONDARY_BTN_SIZE, [120.0, 28.0]);
        assert_eq!(SMALL_BTN_SIZE, [28.0, 24.0]);
    }
}
