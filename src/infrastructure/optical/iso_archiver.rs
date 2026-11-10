//! Concrete `IsoArchiver` adapter bridging `crate::iso::dump_disc_to_iso` to the domain port (DIP/LSP).

use crate::domain::ports::iso_archiver::IsoArchiver;
use crate::domain::ports::transcoder::ProgressObserver;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Production optical sector-by-sector ISO archiver with multi-pass `ddrescue` bad-sector recovery.
#[derive(Debug, Default, Clone)]
pub struct SystemIsoArchiver;

impl SystemIsoArchiver {
    pub fn new() -> Self {
        Self
    }
}

impl IsoArchiver for SystemIsoArchiver {
    fn dump_disc_to_iso(
        &self,
        source_dvd: &Path,
        output_path: &Path,
        volume_label: Option<&str>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<u64> {
        if let Some(obs) = observer {
            let (tx, rx) = std::sync::mpsc::channel();
            let src_str = source_dvd.to_string_lossy().to_string();
            let out_path = output_path.to_path_buf();
            let label = volume_label.map(|s| s.to_string());
            let cancel_clone = cancel_flag.clone();

            let worker = std::thread::spawn(move || {
                crate::iso::dump_disc_to_iso(
                    &src_str,
                    &out_path,
                    label.as_deref(),
                    cancel_clone.as_ref(),
                    Some(&tx),
                )
            });

            while let Ok(event) = rx.recv() {
                match event {
                    crate::ffmpeg::ProgressEvent::Progress {
                        percent,
                        fps,
                        speed,
                    } => {
                        obs.on_progress(percent, &fps, &speed);
                    }
                    crate::ffmpeg::ProgressEvent::Log(line) => {
                        obs.on_log(&line);
                    }
                    _ => {}
                }
            }

            match worker.join() {
                Ok(res) => res,
                Err(_) => Err(anyhow::anyhow!("ISO dump worker thread panicked")),
            }
        } else {
            let src_str = source_dvd.to_string_lossy();
            crate::iso::dump_disc_to_iso(
                &src_str,
                output_path,
                volume_label,
                cancel_flag.as_ref(),
                None,
            )
        }
    }
}

