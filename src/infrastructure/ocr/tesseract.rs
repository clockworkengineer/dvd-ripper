use std::path::Path;
use anyhow::Result;
use crate::ocr::{is_tesseract_available, OcrProcessor};

/// Tesseract-backed OCR processor implementing domain character recognition.
#[derive(Debug, Clone)]
pub struct TesseractOcrService {
    pub binary_path: String,
}

impl TesseractOcrService {
    pub fn new(binary_path: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
        }
    }
}

impl OcrProcessor for TesseractOcrService {
    fn is_available(&self) -> bool {
        is_tesseract_available(&self.binary_path)
    }

    fn recognize_text(&self, image_path: &Path, lang: &str) -> Result<String> {
        let processor = crate::ocr::TesseractOcrProcessor::new(&self.binary_path);
        processor.recognize_text(image_path, lang)
    }
}
