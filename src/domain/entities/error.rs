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

        let drive_not_found = DvdError::DriveNotFound("Z:\\".to_string());
        assert_eq!(drive_not_found.to_string(), "Optical drive not found: Z:\\");

        let no_disc = DvdError::NoDiscDetected("D:\\".to_string());
        assert_eq!(no_disc.to_string(), "No optical disc media detected in drive 'D:\\'");

        let copy_prot = DvdError::CopyProtectionTrap {
            title: 3,
            message: "Bad sector detected".to_string(),
        };
        assert_eq!(
            copy_prot.to_string(),
            "Copy protection trap or bad sectors on Title #3: Bad sector detected"
        );

        let transcode_failed = DvdError::TranscodeFailed("FFmpeg exit code 1".to_string());
        assert_eq!(
            transcode_failed.to_string(),
            "Transcoding failure: FFmpeg exit code 1"
        );

        let meta_not_found = DvdError::MetadataNotFound("The Matrix 1999".to_string());
        assert_eq!(
            meta_not_found.to_string(),
            "Metadata not found for query: The Matrix 1999"
        );

        let cloned = err.clone();
        assert_eq!(err, cloned);
        assert_eq!(format!("{:?}", err), format!("{:?}", cloned));
        let std_err: &dyn std::error::Error = &err;
        assert!(std_err.source().is_none());
    }
}
