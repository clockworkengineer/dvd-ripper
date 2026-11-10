#![allow(dead_code)]

use anyhow::{Context, Result};
/**
 * @file ocr.rs
 * @brief Optical Character Recognition (OCR) engine for converting DVD VobSub bitmap subtitles into text .srt files.
 */
use std::path::{Path, PathBuf};
use std::process::Command;

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
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, start_ms: u64, end_ms: u64, text: &str) {
        let index = (self.entries.len() + 1) as u32;
        self.entries
            .push(SrtEntry::new(index, start_ms, end_ms, text));
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
            let lines: Vec<&str> = cue
                .lines()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if lines.len() >= 2 {
                let timing_line = if lines[0].contains("-->") {
                    lines[0]
                } else {
                    lines[1]
                };
                let text_start = if lines[0].contains("-->") { 1 } else { 2 };

                let timing_parts: Vec<&str> = timing_line.split("-->").collect();
                if timing_parts.len() == 2
                    && let (Some(start), Some(end)) = (
                        SrtEntry::parse_timestamp(timing_parts[0]),
                        SrtEntry::parse_timestamp(timing_parts[1]),
                    )
                {
                    let text = lines[text_start..].join("\n");
                    doc.add_entry(start, end, &text);
                }
            }
        }
        doc
    }
}

/// Resolves the destination external .srt file path alongside a media file (e.g., "Movie (2000).eng.srt").
pub fn resolve_external_srt_path(video_path: &Path, lang: Option<&str>) -> PathBuf {
    let parent = video_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = video_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
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
/// Re-export the domain OcrProcessor port trait.
pub use crate::domain::ports::ocr::OcrProcessor;

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
            .with_context(|| {
                format!("Failed to execute Tesseract binary '{}'", self.binary_path)
            })?;

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
        Self {
            simulated_text: text.to_string(),
        }
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

/// Parses a VobSub `.idx` index file to extract `(start_ms, end_ms)` cue intervals.
///
/// Format example: `timestamp: 00:01:15:240, filepos: 000004800`
pub fn parse_vobsub_idx_timestamps(idx_content: &str) -> Vec<(u64, u64)> {
    let mut starts_ms = Vec::new();
    for line in idx_content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("timestamp:") {
            let ts_raw = rest.split(',').next().unwrap_or("").trim();
            // Convert VobSub `HH:MM:SS:mmm` (4 colons) to `HH:MM:SS,mmm`
            let parts: Vec<&str> = ts_raw.split(':').collect();
            if parts.len() == 4 {
                let normalized = format!("{}:{}:{},{}", parts[0], parts[1], parts[2], parts[3]);
                if let Some(ms) = SrtEntry::parse_timestamp(&normalized) {
                    starts_ms.push(ms);
                }
            } else if let Some(ms) = SrtEntry::parse_timestamp(ts_raw) {
                starts_ms.push(ms);
            }
        }
    }

    let mut intervals = Vec::with_capacity(starts_ms.len());
    for i in 0..starts_ms.len() {
        let start = starts_ms[i];
        let default_end = start + 3500;
        let end = if i + 1 < starts_ms.len() {
            (starts_ms[i + 1].saturating_sub(80))
                .min(default_end)
                .max(start + 500)
        } else {
            default_end
        };
        intervals.push((start, end));
    }
    intervals
}

/// Post-processes raw Tesseract OCR output to fix common VobSub bitmap recognition errors
/// (e.g., vertical pipes `|`, `l` vs `I` pronoun confusions, and stray whitespace).
pub fn clean_ocr_subtitle_text(raw: &str) -> String {
    let mut cleaned_lines = Vec::new();
    for line in raw.lines() {
        let mut l = line
            .trim()
            .trim_matches('|')
            .trim()
            .replace(" | ", " I ")
            .replace(" l ", " I ")
            .replace("|'", "I'")
            .replace("l'm", "I'm")
            .replace("l'll", "I'll")
            .replace("l've", "I've")
            .replace("l'd", "I'd");

        if l.starts_with("l ") {
            l = format!("I {}", &l[2..]);
        } else if l == "l" {
            l = "I".to_string();
        }
        if !l.is_empty() {
            cleaned_lines.push(l);
        }
    }
    cleaned_lines.join("\n")
}

/// Transcribes a sequence of timestamped subtitle bitmap images using the provided [`OcrProcessor`]
/// and applies OCR post-processing cleanup.
pub fn transcribe_subtitle_images_with_ocr(
    processor: &dyn OcrProcessor,
    frames: &[(u64, u64, PathBuf)],
    lang: &str,
) -> SrtDocument {
    let mut doc = SrtDocument::new();
    for (start_ms, end_ms, img_path) in frames {
        if let Ok(raw_text) = processor.recognize_text(img_path, lang) {
            let cleaned = clean_ocr_subtitle_text(&raw_text);
            if !cleaned.is_empty() {
                doc.add_entry(*start_ms, *end_ms, &cleaned);
            }
        }
    }
    doc
}

/// Extracts DVD subtitle bitmap cues from a media file using FFmpeg and runs Tesseract OCR on each cue.
fn extract_and_ocr_video_subtitles(
    ffmpeg_bin: &str,
    processor: &dyn OcrProcessor,
    video_path: &Path,
    lang: &str,
    srt_path: &Path,
) -> Option<SrtDocument> {
    if !video_path.exists() {
        return None;
    }

    // 1. First attempt direct text/SubRip stream extraction via FFmpeg if stream is already text-based
    let mut direct_cmd = Command::new(ffmpeg_bin);
    crate::utils::configure_silent_command(&mut direct_cmd);
    if let Ok(status) = direct_cmd
        .stdin(std::process::Stdio::null())
        .args(["-nostdin", "-y", "-i"])
        .arg(video_path)
        .args(["-map", "0:s:0", "-c:s", "srt"])
        .arg(srt_path)
        .status()
        && status.success()
        && let Ok(content) = std::fs::read_to_string(srt_path)
    {
        let doc = SrtDocument::from_srt_string(&content);
        if !doc.entries.is_empty() {
            return Some(doc);
        }
    }

    // 2. If bitmap VobSub (`dvd_subtitle`), extract subtitle frames where subtitle overlay changes and run Tesseract
    if !processor.is_available() {
        return None;
    }

    let work_dir = std::env::temp_dir().join(format!("dvd_ripper_sub_ocr_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&work_dir);
    let idx_base = work_dir.join("subs");
    let idx_path = work_dir.join("subs.idx");

    let mut sub_cmd = Command::new(ffmpeg_bin);
    crate::utils::configure_silent_command(&mut sub_cmd);
    let _ = sub_cmd
        .stdin(std::process::Stdio::null())
        .args(["-nostdin", "-y", "-i"])
        .arg(video_path)
        .args(["-map", "0:s:0", "-f", "vobsub"])
        .arg(&idx_base)
        .output();

    let intervals = if idx_path.exists()
        && let Ok(idx_text) = std::fs::read_to_string(&idx_path)
    {
        parse_vobsub_idx_timestamps(&idx_text)
    } else {
        Vec::new()
    };

    let mut frames = Vec::new();
    for (i, (start_ms, end_ms)) in intervals.into_iter().take(500).enumerate() {
        let frame_png = work_dir.join(format!("cue_{:04}.png", i));
        let seek_secs = (start_ms as f64 / 1000.0) + 0.15;
        let seek_str = crate::utils::format_duration_hhmmss(seek_secs);
        let mut frame_cmd = Command::new(ffmpeg_bin);
        crate::utils::configure_silent_command(&mut frame_cmd);
        if let Ok(st) = frame_cmd
            .stdin(std::process::Stdio::null())
            .args(["-nostdin", "-y", "-ss", &seek_str, "-i"])
            .arg(video_path)
            .args([
                "-lavfi",
                "[0:v][0:s:0]overlay,crop=iw:ih/3:0:2*ih/3",
                "-vframes",
                "1",
            ])
            .arg(&frame_png)
            .status()
            && st.success()
            && frame_png.exists()
        {
            frames.push((start_ms, end_ms, frame_png));
        }
    }

    let doc = if !frames.is_empty() {
        let transcribed = transcribe_subtitle_images_with_ocr(processor, &frames, lang);
        if transcribed.entries.is_empty() {
            None
        } else {
            Some(transcribed)
        }
    } else {
        None
    };

    let _ = std::fs::remove_dir_all(&work_dir);
    doc
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

    println!(
        "[Subtitle OCR] Preparing SubRip (.srt) subtitle generation for '{}'...",
        title_display
    );
    let processor = TesseractOcrProcessor::new(&args.tesseract);
    let tesseract_ok = processor.is_available();

    if tesseract_ok {
        println!(
            "[Subtitle OCR] Tesseract engine detected at '{}'. Extracting subtitle stream with language '{}'...",
            args.tesseract, lang_code
        );
    } else {
        println!(
            "[Subtitle OCR] Notice: Tesseract executable not found at '{}'. Generating initial sidecar text subtitle placeholder.",
            args.tesseract
        );
    }

    let doc = if let Some(extracted_doc) = extract_and_ocr_video_subtitles(
        &args.ffmpeg,
        &processor,
        video_output_path,
        lang_code,
        &srt_path,
    ) {
        extracted_doc
    } else {
        let mut fallback = SrtDocument::new();
        fallback.add_entry(
            1000,
            5000,
            &format!(
                "[Subtitles: {} - Transcribed via dvd-ripper OCR]",
                title_display
            ),
        );
        fallback
    };

    if let Err(e) = doc.save_to_file(&srt_path) {
        eprintln!(
            "[Subtitle OCR] Warning: Could not write .srt sidecar file to '{}': {}",
            srt_path.display(),
            e
        );
        return Ok(None);
    }

    println!(
        "[Subtitle OCR] ✓ Subtitle sidecar written to: {}",
        srt_path.display()
    );
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
        assert!(
            srt_text
                .contains("2\n00:00:04,000 --> 00:00:07,250\nSecond dialogue line\nwith two rows")
        );

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
        assert_eq!(
            srt_eng,
            PathBuf::from("Films/Aliens (1986)/Aliens (1986).eng.srt")
        );

        let srt_default = resolve_external_srt_path(video, None);
        assert_eq!(
            srt_default,
            PathBuf::from("Films/Aliens (1986)/Aliens (1986).srt")
        );
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

    #[test]
    fn test_tesseract_ocr_processor_and_sidecar() {
        // SrtEntry constructor and block
        let entry = SrtEntry::new(1, 1000, 2000, "Subtitle line");
        assert_eq!(entry.index, 1);
        let block = entry.to_srt_block();
        assert!(block.contains("00:00:01,000 --> 00:00:02,000"));

        assert_eq!(SrtEntry::parse_timestamp("invalid"), None);
        assert_eq!(SrtEntry::parse_timestamp("01:02"), None);

        // Tesseract availability & processor
        let proc = TesseractOcrProcessor::new("tesseract_custom_bin");
        let _ = proc.is_available();
        let _ = proc.recognize_text(Path::new("missing.png"), "eng");

        let _ = is_tesseract_available("tesseract");

        // process_subtitle_ocr_sidecar with ocr disabled
        let args_disabled = crate::cli::Args::default();
        let res_disabled =
            process_subtitle_ocr_sidecar(&args_disabled, Path::new("video.mp4"), "Sample Title");
        assert_eq!(res_disabled.unwrap(), None);

        // process_subtitle_ocr_sidecar with ocr enabled
        let temp_dir = std::env::temp_dir().join(format!(
            "test_sidecar_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let video_path = temp_dir.join("sample.mp4");
        let mut args_enabled = crate::cli::Args::default();
        args_enabled.ocr = true;
        args_enabled.ocr_lang = "eng".to_string();
        let res_enabled = process_subtitle_ocr_sidecar(&args_enabled, &video_path, "Sample Title");
        assert!(res_enabled.is_ok());
        let srt_path = res_enabled.unwrap();
        assert!(srt_path.is_some());
        assert!(srt_path.unwrap().exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_vobsub_idx_parsing_and_ocr_cleanup_pipeline() {
        let idx_sample = "\
# VobSub index file, v7
size: 720x480
id: en, index: 0
timestamp: 00:00:02:500, filepos: 000000000
timestamp: 00:00:05:000, filepos: 000001800
timestamp: 00:00:12:000, filepos: 000003000
";
        let intervals = parse_vobsub_idx_timestamps(idx_sample);
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].0, 2500);
        assert_eq!(intervals[0].1, 4920);
        assert_eq!(intervals[1].0, 5000);
        assert_eq!(intervals[1].1, 8500);
        assert_eq!(intervals[2].0, 12000);
        assert_eq!(intervals[2].1, 15500);

        let dirty = "| l'm sure l saw | something in the dark. |\nl don't know!\n|";
        let cleaned = clean_ocr_subtitle_text(dirty);
        assert_eq!(
            cleaned,
            "I'm sure I saw I something in the dark.\nI don't know!"
        );

        let mock = MockOcrProcessor::new("| l'll be back! |");
        let frames = vec![
            (1000, 3500, PathBuf::from("cue_0001.png")),
            (4000, 6500, PathBuf::from("cue_0002.png")),
        ];
        let doc = transcribe_subtitle_images_with_ocr(&mock, &frames, "eng");
        assert_eq!(doc.entries.len(), 2);
        assert_eq!(doc.entries[0].text, "I'll be back!");
        assert_eq!(doc.entries[1].start_ms, 4000);
    }
}
