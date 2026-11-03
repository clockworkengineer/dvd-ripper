//! Cross-platform optical drive abstraction layer.
//!
//! Provides platform-specific drivers for Windows, Linux, and macOS with unified APIs.

pub mod macos;
pub mod unix;
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows as current;

#[cfg(all(unix, not(target_os = "macos")))]
pub use unix as current;

#[cfg(target_os = "macos")]
pub use macos as current;
