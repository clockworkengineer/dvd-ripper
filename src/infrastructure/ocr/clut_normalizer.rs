//! Subtitle Palette (CLUT) Normalizer and Advanced OCR Text Post-Processor.
//!
//! Fixes corrupted, illegible, or low-contrast DVD subtitle color lookup tables
//! in VobSub `.idx` headers, and cleans subtle OCR misrecognitions.

/// Broadcast standard high-contrast 16-color RGB palette for DVD subtitle overlays.
/// Color 0: Black background / outline
/// Color 1: Bright White text body
/// Color 2: Deep Black outer contour border
/// Color 3: Neutral Gray anti-aliasing blend
pub const BROADCAST_STANDARD_PALETTE_HEX: &str =
    "000000, ffffff, 101010, 808080, \
     202020, e0e0e0, 404040, a0a0a0, \
     181818, d8d8d8, 303030, 909090, \
     282828, c8c8c8, 505050, b0b0b0";

/// Normalizes the `palette: ...` directive within a VobSub `.idx` index file.
///
/// If the palette is missing, unreadable, or exhibits pathological low-contrast
/// mastering (e.g. all zeroes, neon greens, or invisible gray-on-gray), replaces
/// it with standard broadcast high-contrast colors (white text, black outline).
#[must_use]
pub fn normalize_vobsub_idx_palette(idx_content: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut palette_found = false;

    for line in idx_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("palette:") {
            palette_found = true;
            // Check if existing palette is pathological
            let hex_values = trimmed.trim_start_matches("palette:").trim();
            if is_palette_pathological(hex_values) {
                lines.push(format!("palette: {}", BROADCAST_STANDARD_PALETTE_HEX));
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    if !palette_found {
        lines.push(format!("palette: {}", BROADCAST_STANDARD_PALETTE_HEX));
    }

    lines.join("\n")
}

/// Checks whether an existing hex palette string is defective or unreadable.
#[must_use]
pub fn is_palette_pathological(palette_str: &str) -> bool {
    let colors: Vec<&str> = palette_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if colors.len() < 4 {
        return true;
    }

    // Check 1: All identical (e.g. all black 000000 or all zeroes)
    let first = colors[0];
    if colors.iter().all(|&c| c.eq_ignore_ascii_case(first)) {
        return true;
    }

    // Check 2: All values lack high luminance contrast (e.g. all dark < 0x30 or all light > 0xD0)
    let luminance_values: Vec<u32> = colors
        .iter()
        .filter_map(|c| u32::from_str_radix(c, 16).ok())
        .map(|rgb| {
            let r = (rgb >> 16) & 0xFF;
            let g = (rgb >> 8) & 0xFF;
            let b = rgb & 0xFF;
            (r * 299 + g * 587 + b * 114) / 1000
        })
        .collect();

    if luminance_values.is_empty() {
        return true;
    }

    let min_lum = luminance_values.iter().min().copied().unwrap_or(0);
    let max_lum = luminance_values.iter().max().copied().unwrap_or(255);

    // If max contrast between brightest and darkest color is less than 60, subtitles are unreadable
    (max_lum - min_lum) < 60
}

/// Enhanced OCR text cleanup resolving common optical scanning ambiguities.
#[must_use]
pub fn clean_advanced_ocr_text(raw_text: &str) -> String {
    let mut cleaned_lines = Vec::new();

    for line in raw_text.lines() {
        let mut text = line.trim().to_string();

        // 1. Basic OCR pronoun fixes from existing pipeline
        text = text
            .replace(" | ", " I ")
            .replace(" l ", " I ")
            .replace("|'", "I'")
            .replace("l'm", "I'm")
            .replace("l'll", "I'll")
            .replace("l've", "I've")
            .replace("l'd", "I'd");

        if text.starts_with("l ") {
            text = format!("I {}", &text[2..]);
        } else if text == "l" {
            text = "I".to_string();
        }

        // 2. Unicode quotation mark normalization
        text = text
            .replace(['\u{201C}', '\u{201D}'], "\"")
            .replace(['\u{2018}', '\u{2019}'], "'");

        // 3. Em-dash and double hyphen normalization
        text = text.replace("--", "—").replace("— ", "—");

        // 4. Common OCR scan artifact fixes
        // Fix standalone digits mistakenly read in words (e.g. "0f" -> "of")
        text = text.replace(" 0f ", " of ").replace(" 0n ", " on ");

        // Strip stray leading/trailing pipes and tildes
        text = text.trim_matches(|c: char| c == '|' || c == '~' || c == '_').trim().to_string();

        if !text.is_empty() {
            cleaned_lines.push(text);
        }
    }

    cleaned_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_palette_pathological_all_black() {
        let black_palette = "000000, 000000, 000000, 000000";
        assert!(is_palette_pathological(black_palette));
    }

    #[test]
    fn test_is_palette_pathological_low_contrast() {
        // Dark grays: 101010, 151515, 202020 (lum diff < 20)
        let low_contrast = "101010, 151515, 181818, 202020";
        assert!(is_palette_pathological(low_contrast));
    }

    #[test]
    fn test_is_palette_healthy() {
        let healthy = "000000, ffffff, 101010, 808080";
        assert!(!is_palette_pathological(healthy));
    }

    #[test]
    fn test_normalize_vobsub_idx_palette_replaces_broken() {
        let broken_idx = r#"# VobSub index file
size: 720x480
palette: 000000, 000000, 000000, 000000
id: en, index: 0
"#;

        let normalized = normalize_vobsub_idx_palette(broken_idx);
        assert!(normalized.contains("ffffff"));
        assert!(!normalized.contains("palette: 000000, 000000, 000000, 000000"));
    }

    #[test]
    fn test_clean_advanced_ocr_text_quotes_and_pronouns() {
        let raw = "“l'm going to the store, 0f course.” |";
        let cleaned = clean_advanced_ocr_text(raw);
        assert_eq!(cleaned, "\"I'm going to the store, of course.\"");
    }

    #[test]
    fn test_clean_advanced_ocr_dashes() {
        let raw = "Wait--did you hear that?";
        let cleaned = clean_advanced_ocr_text(raw);
        assert_eq!(cleaned, "Wait—did you hear that?");
    }
}

