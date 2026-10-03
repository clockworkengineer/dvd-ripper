//! # Infrastructure Layer
//!
//! Contains concrete adapters that implement the abstract port interfaces defined in the domain layer.
//!
//! Submodules:
//! - [`ffmpeg`]: FFmpeg command building, child process management, and progress parsing.
//! - [`metadata`]: HTTP clients for OMDb and TMDb online metadata queries with local caching.
//! - [`notifications`]: Binary MQTT 3.1.1 packet generation and HTTP webhook dispatchers.
//! - [`ocr`]: Tesseract OCR runner for extracting SubRip text subtitles from DVD bitmap overlays.
//! - [`optical`]: OS-specific optical drive readers and ISO-9660 sector inspectors.
//! - [`storage`]: Enterprise disk space guard checks and JSON history repository persistence.
//! - [`system`]: OS-native file dialog services and desktop explorer triggers.

pub mod ffmpeg;
pub mod metadata;
pub mod notifications;
pub mod ocr;
pub mod optical;
pub mod storage;
pub mod system;
