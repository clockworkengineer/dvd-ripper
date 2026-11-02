use anyhow::Result;
use std::path::Path;

/// Hardware and process abstraction for Optical Character Recognition (OCR) engines.
pub trait OcrProcessor: Send + Sync {
    /// Determines whether the underlying OCR binary (e.g. Tesseract) is installed and operational.
    fn is_available(&self) -> bool;

    /// Extracts clean UTF-8 text from an image containing subtitle bitmap glyphs.
    fn recognize_text(&self, image_path: &Path, lang: &str) -> Result<String>;
}
