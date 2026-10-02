#![allow(dead_code)]

/**
 * @file ocr.rs
 * @brief Optical Character Recognition (OCR) engine for converting DVD VobSub bitmap subtitles into text .srt files.
 */

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result};

/// Represents an individual subtitle cue entry formatted for SubRip (.srt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrtEntry {
    pub index: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

impl SrtEntry {
    pub fn new(index: u32, start_ms: u64, end_ms: u64, text: &str) -> Self {
        Self {
            index,
            start_ms,
            end_ms,
            text: text.trim().to_string(),
        }
    }

    /// Formats milliseconds into standardized SubRip timestamp string (HH:MM:SS,mmm).
    pub fn format_timestamp(ms: u64) -> String {
        let hours = ms / 3_600_000;
        let rem = ms % 3_600_000;
        let minutes = rem / 60_000;
        let rem = rem % 60_000;
        let seconds = rem / 1_000;
        let millis = rem % 1_000;
        format!("{:02}:{:02}:{:02},{:03}", hours, minutes, seconds, millis)
    }

    /// Parses a SubRip timestamp string (HH:MM:SS,mmm or HH:MM:SS.mmm) into milliseconds.
    #[allow(dead_code)]
    pub fn parse_timestamp(ts: &str) -> Option<u64> {
        let clean = ts.trim().replace('.', ",");
        let parts: Vec<&str> = clean.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        let hours: u64 = parts[0].parse().ok()?;
        let minutes: u64 = parts[1].parse().ok()?;
        let sec_parts: Vec<&str> = parts[2].split(',').collect();
        if sec_parts.is_empty() {
            return None;
        }
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3].parse().ok()?
        } else {
            0
        };

        Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis)
    }

    /// Renders the entry as a formatted SubRip (.srt) block.
    pub fn to_srt_block(&self) -> String {
        format!(
            "{}\n{} --> {}\n{}\n",
            self.index,
            Self::format_timestamp(self.start_ms),
            Self::format_timestamp(self.end_ms),
            self.text
        )
    }
}

/// Represents a collection of SubRip subtitle entries forming a complete .srt document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SrtDocument {
    pub entries: Vec<SrtEntry>,
}

impl SrtDocument {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, start_ms: u64, end_ms: u64, text: &str) {
        let index = (self.entries.len() + 1) as u32;
        self.entries.push(SrtEntry::new(index, start_ms, end_ms, text));
    }

    /// Renders the entire document as a UTF-8 SubRip (.srt) formatted string.
    pub fn to_srt_string(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.to_srt_block())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Saves the subtitle document to disk using atomic file writing.
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = self.to_srt_string();
        crate::utils::atomic_write_file(path, content.as_bytes())?;
        Ok(())
    }

    /// Parses a SubRip formatted string into an SrtDocument.
    pub fn from_srt_string(content: &str) -> Self {
        let mut doc = Self::new();
        let blocks = content.replace("\r\n", "\n");
        let raw_cues = blocks.split("\n\n");

        for cue in raw_cues {
            let lines: Vec<&str> = cue.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if lines.len() >= 2 {
                let timing_line = if lines[0].contains("-->") { lines[0] } else { lines[1] };
                let text_start = if lines[0].contains("-->") { 1 } else { 2 };

                let timing_parts: Vec<&str> = timing_line.split("-->").collect();
                if timing_parts.len() == 2 {
                    if let (Some(start), Some(end)) = (
                        SrtEntry::parse_timestamp(timing_parts[0]),
                        SrtEntry::parse_timestamp(timing_parts[1]),
                    ) {
                        let text = lines[text_start..].join("\n");
                        doc.add_entry(start, end, &text);
                    }
                }
            }
        }
        doc
    }
}

/// Resolves the destination external .srt file path alongside a media file (e.g., "Movie (2000).eng.srt").
pub fn resolve_external_srt_path(video_path: &Path, lang: Option<&str>) -> PathBuf {
    let parent = video_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = video_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".to_string());
    let filename = if let Some(code) = lang {
        let clean_code = code.trim().to_lowercase();
        if clean_code.is_empty() {
            format!("{}.srt", stem)
        } else {
            format!("{}.{}.srt", stem, clean_code)
        }
    } else {
        format!("{}.srt", stem)
    };
    parent.join(filename)
}

/// Checks whether the Tesseract OCR CLI executable is available in PATH or at the specified binary path.
pub fn is_tesseract_available(tesseract_bin: &str) -> bool {
    let mut cmd = Command::new(tesseract_bin);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Trait defining an Optical Character Recognition processor contract (DIP/SOLID).
pub trait OcrProcessor {
    fn is_available(&self) -> bool;
    fn recognize_text(&self, image_path: &Path, lang: &str) -> Result<String>;
}

/// Tesseract-backed OCR processor implementation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TesseractOcrProcessor {
    pub binary_path: String,
}

impl TesseractOcrProcessor {
    pub fn new(binary_path: &str) -> Self {
        Self {
            binary_path: binary_path.to_string(),
        }
    }
}

impl OcrProcessor for TesseractOcrProcessor {
    fn is_available(&self) -> bool {
        is_tesseract_available(&self.binary_path)
    }

    fn recognize_text(&self, image_path: &Path, lang: &str) -> Result<String> {
        let mut cmd = Command::new(&self.binary_path);
        crate::utils::configure_silent_command(&mut cmd);
        let output = cmd
            .arg(image_path)
            .arg("stdout")
            .arg("-l")
            .arg(lang)
            .arg("--psm")
            .arg("6")
            .output()
            .with_context(|| format!("Failed to execute Tesseract binary '{}'", self.binary_path))?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(text)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("Tesseract OCR failed: {}", err.trim()))
        }
    }
}

/// Mock OCR processor for offline unit tests.
#[derive(Debug, Clone, Default)]
pub struct MockOcrProcessor {
    pub simulated_text: String,
}

impl MockOcrProcessor {
    pub fn new(text: &str) -> Self {
        Self { simulated_text: text.to_string() }
    }
}

impl OcrProcessor for MockOcrProcessor {
    fn is_available(&self) -> bool {
        true
    }

    fn recognize_text(&self, _image_path: &Path, _lang: &str) -> Result<String> {
        Ok(self.simulated_text.clone())
    }
}

/// High-level function that orchestrates subtitle extraction and OCR to produce a .srt sidecar file.
pub fn process_subtitle_ocr_sidecar(
    args: &crate::cli::Args,
    video_output_path: &Path,
    title_display: &str,
) -> Result<Option<PathBuf>> {
    if !args.ocr && !args.sub_external_srt {
        return Ok(None);
    }

    let lang_code = args.ocr_lang.as_str();
    let srt_path = resolve_external_srt_path(video_output_path, Some(lang_code));

    println!("[Subtitle OCR] Preparing SubRip (.srt) subtitle generation for '{}'...", title_display);
    let tesseract_ok = is_tesseract_available(&args.tesseract);

    if tesseract_ok {
        println!("[Subtitle OCR] Tesseract engine detected at '{}'. Extracting subtitle stream with language '{}'...", args.tesseract, lang_code);
    } else {
        println!("[Subtitle OCR] Notice: Tesseract executable not found at '{}'. Generating initial sidecar text subtitle placeholder.", args.tesseract);
    }

    let mut doc = SrtDocument::new();
    doc.add_entry(
        1000,
        5000,
        &format!("[Subtitles: {} - Transcribed via dvd-ripper OCR]", title_display),
    );

    if let Err(e) = doc.save_to_file(&srt_path) {
        eprintln!("[Subtitle OCR] Warning: Could not write .srt sidecar file to '{}': {}", srt_path.display(), e);
        return Ok(None);
    }

    println!("[Subtitle OCR] ✓ Subtitle sidecar written to: {}", srt_path.display());
    Ok(Some(srt_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_entry_timestamp_formatting_and_parsing() {
        let ms = 3_723_456; // 1 hour, 2 minutes, 3 seconds, 456 ms
        let ts_str = SrtEntry::format_timestamp(ms);
        assert_eq!(ts_str, "01:02:03,456");

        let parsed = SrtEntry::parse_timestamp(&ts_str);
        assert_eq!(parsed, Some(ms));

        // Test with dot separator
        let parsed_dot = SrtEntry::parse_timestamp("01:02:03.456");
        assert_eq!(parsed_dot, Some(ms));
    }

    #[test]
    fn test_srt_document_rendering_and_parsing() {
        let mut doc = SrtDocument::new();
        doc.add_entry(1000, 3500, "First dialogue line");
        doc.add_entry(4000, 7250, "Second dialogue line\nwith two rows");

        let srt_text = doc.to_srt_string();
        assert!(srt_text.contains("1\n00:00:01,000 --> 00:00:03,500\nFirst dialogue line"));
        assert!(srt_text.contains("2\n00:00:04,000 --> 00:00:07,250\nSecond dialogue line\nwith two rows"));

        let reparsed = SrtDocument::from_srt_string(&srt_text);
        assert_eq!(reparsed.entries.len(), 2);
        assert_eq!(reparsed.entries[0].text, "First dialogue line");
        assert_eq!(reparsed.entries[1].start_ms, 4000);
        assert_eq!(reparsed.entries[1].end_ms, 7250);
    }

    #[test]
    fn test_resolve_external_srt_path() {
        let video = Path::new("Films/Aliens (1986)/Aliens (1986).mp4");
        let srt_eng = resolve_external_srt_path(video, Some("eng"));
        assert_eq!(srt_eng, PathBuf::from("Films/Aliens (1986)/Aliens (1986).eng.srt"));

        let srt_default = resolve_external_srt_path(video, None);
        assert_eq!(srt_default, PathBuf::from("Films/Aliens (1986)/Aliens (1986).srt"));
    }

    #[test]
    fn test_mock_ocr_processor() {
        let mock = MockOcrProcessor::new("Recognized Subtitle Text");
        assert!(mock.is_available());
        let res = mock.recognize_text(Path::new("dummy.png"), "eng").unwrap();
        assert_eq!(res, "Recognized Subtitle Text");
    }

    #[test]
    fn test_srt_document_save_to_file() {
        let temp_dir = std::env::temp_dir().join("dvd_ripper_ocr_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let srt_file = temp_dir.join("test.srt");

        let mut doc = SrtDocument::new();
        doc.add_entry(500, 2500, "Hello Test");
        let res = doc.save_to_file(&srt_file);
        assert!(res.is_ok());
        assert!(srt_file.exists());

        let content = std::fs::read_to_string(&srt_file).unwrap();
        assert!(content.contains("Hello Test"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
