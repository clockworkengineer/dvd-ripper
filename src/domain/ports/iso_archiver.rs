//! Domain port for 1:1 byte-for-byte archival ISO disc imaging (DIP/LSP).

use crate::domain::ports::transcoder::ProgressObserver;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Dependency Inversion (DIP/SOLID): Port contract for 1:1 byte-for-byte archival ISO disc imaging.
pub trait IsoArchiver: Send + Sync {
    /// Copies raw sectors or directory structures from `source_dvd` into an ISO-9660 file at `output_path`.
    fn dump_disc_to_iso(
        &self,
        source_dvd: &Path,
        output_path: &Path,
        volume_label: Option<&str>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockIsoArchiver;

    impl IsoArchiver for MockIsoArchiver {
        fn dump_disc_to_iso(
            &self,
            _source_dvd: &Path,
            _output_path: &Path,
            _volume_label: Option<&str>,
            observer: Option<&dyn ProgressObserver>,
            _cancel_flag: Option<Arc<AtomicBool>>,
        ) -> Result<u64> {
            if let Some(obs) = observer {
                obs.on_progress(100.0, "0", "10.0 MB/s");
            }
            Ok(4_700_000_000)
        }
    }

    #[test]
    fn test_mock_iso_archiver_contract() {
        let archiver = MockIsoArchiver;
        let bytes = archiver
            .dump_disc_to_iso(Path::new("D:\\"), Path::new("out.iso"), Some("DISC"), None, None)
            .unwrap();
        assert_eq!(bytes, 4_700_000_000);
    }
}

