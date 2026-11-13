//! Domain port for optical drive acoustic noise control, speed negotiation, and thermal supervision (SRP).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Acoustic profile for optical drive speed, noise, and laser thermal management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DriveAcousticProfile {
    /// Silent Homelab Mode (~4x DVD speed, ~5,540 KB/s).
    /// Whisper-quiet operation suitable for living rooms and continuous headless servers.
    Silent,
    /// Balanced Standard Mode (~8x DVD speed, ~11,080 KB/s).
    /// Default balance between read throughput and optical drive acoustic noise.
    #[default]
    Balanced,
    /// Turbo Archival Mode (Maximum hardware drive speed, 16x+).
    /// Optimized for fastest possible archival read throughput.
    Turbo,
}

impl DriveAcousticProfile {
    /// Returns the target DVD read speed in KB/s.
    /// (1x DVD = 1,385 KB/s).
    pub fn target_dvd_read_kbps(&self) -> u32 {
        match self {
            Self::Silent => 5_540,      // ~4x
            Self::Balanced => 11_080,   // ~8x
            Self::Turbo => 0xFFFF,      // Maximum drive capability
        }
    }

    /// User-friendly label for GUI and CLI logs.
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Silent => "Silent (4x Whisper-Quiet)",
            Self::Balanced => "Balanced (8x Standard)",
            Self::Turbo => "Turbo (Maximum Speed)",
        }
    }
}

/// Port for negotiating optical drive spindle speed and managing laser thermal rest intervals.
pub trait AcousticSupervisor: Send + Sync {
    /// Issues SCSI / OS commands to throttle optical spindle speed according to chosen profile.
    fn apply_profile(&self, drive_path: &Path, profile: DriveAcousticProfile) -> Result<()>;

    /// Handles laser thermal cool-down pause when repeated read errors occur in damaged sectors.
    /// Returns milliseconds paused (if any).
    fn handle_read_error_thermal_rest(&self, consecutive_errors: u32) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drive_acoustic_profiles() {
        assert_eq!(DriveAcousticProfile::Silent.target_dvd_read_kbps(), 5_540);
        assert_eq!(DriveAcousticProfile::Balanced.target_dvd_read_kbps(), 11_080);
        assert_eq!(DriveAcousticProfile::Turbo.target_dvd_read_kbps(), 0xFFFF);

        assert!(DriveAcousticProfile::Silent.display_label().contains("Silent"));
        assert!(DriveAcousticProfile::Balanced.display_label().contains("Balanced"));
        assert!(DriveAcousticProfile::Turbo.display_label().contains("Turbo"));
    }
}

