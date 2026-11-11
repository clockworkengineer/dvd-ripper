//! MakeMKV CLI (`makemkvcon`) Transcoder adapter implementing the domain `Transcoder` port.
//!
//! Executes `makemkvcon` in robot mode (`-r --progress=-same`) for lossless `.mkv` remuxing
//! with hardware-level optical decryption (AACS, BD+, CSS, structural obfuscation).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, Transcoder,
};
use crate::infrastructure::ffmpeg::ChildGuard;

/// Transcoder implementation powered by MakeMKV's `makemkvcon`.
pub struct MakeMkvTranscoder {
    makemkv_path: String,
}

impl MakeMkvTranscoder {
    /// Creates a new `MakeMkvTranscoder`.
    pub fn new(makemkv_path: impl Into<String>) -> Self {
        Self {
            makemkv_path: makemkv_path.into(),
        }
    }

    /// Checks whether `makemkvcon` is available in PATH or at the specified executable path.
    pub fn is_available(path: &str) -> bool {
        let mut cmd = Command::new(path);
        crate::utils::configure_silent_command(&mut cmd);
        cmd.arg("-r")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success() || s.code() == Some(1))
            .unwrap_or(false)
    }

    /// Parses MakeMKV robot progress line:
    /// `PRGV:current,total,max` e.g. `PRGV:32768,65536,65536` -> 50.0%
    pub fn parse_progress_line(line: &str) -> Option<f64> {
        if !line.starts_with("PRGV:") {
            return None;
        }
        let parts: Vec<&str> = line[5..].split(',').collect();
        if parts.len() >= 2 {
            let current = parts[0].trim().parse::<f64>().ok()?;
            let total = parts[1].trim().parse::<f64>().ok()?;
            if total > 0.0 {
                return Some(((current / total) * 100.0).clamp(0.0, 99.9));
            }
        }
        None
    }
}

impl Transcoder for MakeMkvTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let out_dir = job.output_path.parent().unwrap_or(Path::new("."));
        if !out_dir.exists() {
            std::fs::create_dir_all(out_dir)?;
        }

        // makemkvcon mkv disc:<id> or file:<path> <title_index> <dest_dir>
        let mut cmd = Command::new(&self.makemkv_path);
        crate::utils::configure_silent_command(&mut cmd);

        cmd.arg("-r")
            .arg("--progress=-same")
            .arg("mkv");

        let src_str = job.source_dvd.to_string_lossy();
        if src_str.ends_with(".iso") || src_str.ends_with(".ISO") {
            cmd.arg(format!("iso:{}", src_str));
        } else if src_str.contains("VIDEO_TS") || src_str.contains("video_ts") {
            cmd.arg(format!("file:{}", src_str));
        } else {
            // Assume drive path, e.g. D: or /dev/sr0
            cmd.arg("disc:0");
        }

        // Title index in makemkv is 0-based for specific title or "all"
        let title_arg = if job.title_number > 0 {
            (job.title_number - 1).to_string()
        } else {
            "all".to_string()
        };
        cmd.arg(title_arg);
        cmd.arg(out_dir);

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[MakeMKV Engine] Ripping title to directory '{}'...",
                out_dir.display()
            ));
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let raw_child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn makemkvcon ('{}'). Ensure MakeMKV is installed.",
                self.makemkv_path
            )
        })?;
        let mut child = ChildGuard::new(raw_child);

        let stdout = child
            .as_mut()
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture makemkvcon stdout"))?;
        let reader = BufReader::new(stdout);

        for line_res in reader.lines() {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                return Err(anyhow!("MakeMKV operation cancelled by user."));
            }

            if let Ok(line) = line_res {
                if let Some(pct) = Self::parse_progress_line(&line) {
                    if let Some(obs) = observer {
                        obs.on_progress(pct, "N/A", "Ripping");
                    }
                } else if line.starts_with("MSG:")
                    && let Some(obs) = observer
                {
                    obs.on_log(&format!("[MakeMKV] {}", line));
                }
            }
        }

        let status = child
            .as_mut()
            .wait()
            .context("Failed to wait on makemkvcon process")?;
        child.disarm();

        if !status.success() {
            return Err(anyhow!(
                "makemkvcon exited with non-zero code: {:?}",
                status.code()
            ));
        }

        // MakeMKV outputs title00.mkv (or similar). If job.output_path was specified,
        // find newly created .mkv in out_dir and rename to match job.output_path if needed.
        if let Ok(entries) = std::fs::read_dir(out_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().and_then(|e| e.to_str()) == Some("mkv")
                    && path != job.output_path
                    && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                    && (file_name.starts_with("title") || file_name.starts_with("t0"))
                {
                    let _ = std::fs::rename(&path, job.output_path);
                    break;
                }
            }
        }

        if let Some(obs) = observer {
            obs.on_progress(100.0, "N/A", "Done");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_makemkv_progress() {
        let line = "PRGV:32768,65536,65536";
        let pct = MakeMkvTranscoder::parse_progress_line(line);
        assert!(pct.is_some());
        assert!((pct.unwrap() - 50.0).abs() < 0.1);

        // Invalid cases
        assert!(MakeMkvTranscoder::parse_progress_line("MSG:5004,0,0").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:abc,def").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:10,0").is_none());
    }

    #[test]
    fn test_makemkv_is_available_nonexistent() {
        assert!(!MakeMkvTranscoder::is_available("nonexistent_makemkvcon_xyz_1234"));
    }

    #[test]
    fn test_makemkv_transcode_nonexistent_binary() {
        let transcoder = MakeMkvTranscoder::new("nonexistent_makemkvcon_xyz_1234");
        let tmp = std::env::temp_dir().join(format!("mmkv_test_{}", std::process::id()));
        let out_file = tmp.join("out.mkv");
        let job = TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: &out_file,
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        };

        let res = transcoder.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Failed to spawn makemkvcon"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_makemkv_transcode_iso_source_arg() {
        let transcoder = MakeMkvTranscoder::new("nonexistent_makemkvcon_xyz_1234");
        let tmp = std::env::temp_dir().join(format!("mmkv_iso_test_{}", std::process::id()));
        let out_file = tmp.join("out.mkv");
        let job = TranscodeJob {
            source_dvd: Path::new("C:\\movies\\sample.iso"),
            output_path: &out_file,
            title_number: 0,
            display_title: "Main",
            expected_duration_secs: None,
        };

        let res = transcoder.transcode(&job, None, None);
        assert!(res.is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
