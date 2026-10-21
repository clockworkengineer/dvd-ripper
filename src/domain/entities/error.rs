use std::fmt;

/// Domain-specific errors representing failures in optical drives, transcoding, storage, and metadata.
#[derive(Debug, Clone, PartialEq)]
pub enum DvdError {
    /// Optical drive device or path was not found.
    DriveNotFound(String),
    /// Drive was detected but no optical disc media is currently inserted.
    NoDiscDetected(String),
    /// Available storage capacity does not meet minimum safety threshold.
    InsufficientDiskSpace {
        required_gb: u64,
        available_gb: f64,
        path: String,
    },
    /// Optical copy protection (CSS, ARccOS, RipGuard) or dirty disc sector trap encountered.
    CopyProtectionTrap { title: u32, message: String },
    /// FFmpeg transcoding or demuxing process exited with an error.
    TranscodeFailed(String),
    /// Online metadata lookup failed or returned no valid candidate.
    MetadataNotFound(String),
    /// Operation was cancelled by user request or cancellation token.
    Cancelled,
}

impl fmt::Display for DvdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DriveNotFound(path) => write!(f, "Optical drive not found: {}", path),
            Self::NoDiscDetected(path) => {
                write!(f, "No optical disc media detected in drive '{}'", path)
            }
            Self::InsufficientDiskSpace {
                required_gb,
                available_gb,
                path,
            } => write!(
                f,
                "Disk Space Safeguard: Only {:.2} GB available in '{}', but minimum threshold is {} GB",
                available_gb, path, required_gb
            ),
            Self::CopyProtectionTrap { title, message } => write!(
                f,
                "Copy protection trap or bad sectors on Title #{}: {}",
                title, message
            ),
            Self::TranscodeFailed(msg) => write!(f, "Transcoding failure: {}", msg),
            Self::MetadataNotFound(query) => write!(f, "Metadata not found for query: {}", query),
            Self::Cancelled => write!(f, "Ripping process cancelled by user"),
        }
    }
}

impl std::error::Error for DvdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvd_error_display() {
        let err = DvdError::InsufficientDiskSpace {
            required_gb: 10,
            available_gb: 4.5,
            path: "D:\\Rips".to_string(),
        };
        assert!(err.to_string().contains("10 GB"));
        assert!(err.to_string().contains("4.50 GB"));

        let cancelled = DvdError::Cancelled;
        assert_eq!(cancelled.to_string(), "Ripping process cancelled by user");
    }
}
