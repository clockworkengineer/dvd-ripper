use crate::domain::ports::ocr::OcrProcessor;
use crate::ocr::is_tesseract_available;
use anyhow::Result;
use std::path::Path;

/// Tesseract-backed OCR processor implementing domain character recognition.
#[derive(Debug, Clone)]
pub struct TesseractOcrService {
    pub binary_path: String,
}

impl Default for TesseractOcrService {
    fn default() -> Self {
        Self::new("tesseract")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tesseract_ocr_service() {
        let service = TesseractOcrService::default();
        assert_eq!(service.binary_path, "tesseract");

        let custom = TesseractOcrService::new("nonexistent-tesseract-bin");
        assert_eq!(custom.binary_path, "nonexistent-tesseract-bin");
        assert!(!custom.is_available());

        let res = custom.recognize_text(Path::new("dummy.png"), "eng");
        assert!(res.is_err());

        let cloned = custom.clone();
        assert_eq!(format!("{:?}", custom), format!("{:?}", cloned));
    }
}
