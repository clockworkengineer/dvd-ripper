//! HandBrakeCLI Transcoder adapter implementing the domain `Transcoder` port.
//!
//! Spawns `HandBrakeCLI` for advanced DVD and Blu-ray transcoding with native `libdvdnav`
//! menu navigation, decomb filtering, hardware acceleration, and real-time progress parsing.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
};
use crate::infrastructure::ffmpeg::ChildGuard;

/// Transcoder implementation powered by `HandBrakeCLI`.
pub struct HandBrakeTranscoder {
    handbrake_path: String,
    options: TranscodeOptions,
}

impl HandBrakeTranscoder {
    /// Creates a new `HandBrakeTranscoder`.
    pub fn new(handbrake_path: impl Into<String>, options: TranscodeOptions) -> Self {
        Self {
            handbrake_path: handbrake_path.into(),
            options,
        }
    }

    /// Checks whether `HandBrakeCLI` is available in PATH or at the specified executable path.
    pub fn is_available(path: &str) -> bool {
        let mut cmd = Command::new(path);
        crate::utils::configure_silent_command(&mut cmd);
        cmd.arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Builds the `Command` for executing HandBrakeCLI based on job and options.
    pub fn build_command(
        &self,
        job: &TranscodeJob,
        chapter_range: Option<(u32, u32)>,
    ) -> Command {
        let mut cmd = Command::new(&self.handbrake_path);
        crate::utils::configure_silent_command(&mut cmd);

        cmd.arg("-i").arg(job.source_dvd);
        cmd.arg("-o").arg(job.output_path);

        // Title selection: HandBrake uses 1-based title index. 0 auto-detects main feature in HandBrake.
        if job.title_number > 0 {
            cmd.arg("-t").arg(job.title_number.to_string());
        } else {
            cmd.arg("--main-feature");
        }

        // Chapters
        if let Some((start, end)) = chapter_range {
            cmd.arg("-c").arg(format!("{}-{}", start, end));
        }

        // Codec selection
        let codec = self.options.codec.to_lowercase();
        let encoder = match codec.as_str() {
            "hevc" | "h265" | "x265" => "x265",
            "av1" => "svt_av1",
            "mpeg2" | "mpeg2video" => "mpeg2",
            _ => "x264",
        };
        cmd.arg("-e").arg(encoder);

        // Quality (CRF)
        if let Some(crf) = self.options.crf {
            cmd.arg("-q").arg(crf.to_string());
        } else {
            cmd.arg("-q").arg("20");
        }

        // Preset
        let preset = match self.options.preset.to_lowercase().as_str() {
            "ultrafast" | "superfast" | "veryfast" => "veryfast",
            "faster" | "fast" => "fast",
            "medium" => "medium",
            "slow" | "slower" | "veryslow" => "slow",
            _ => "fast",
        };
        cmd.arg("--encoder-preset").arg(preset);

        // Audio configuration
        if self.options.all_audio {
            cmd.arg("--all-audio");
        } else {
            cmd.arg("-a").arg("1");
        }
        cmd.arg("-E").arg("copy:aac,copy:ac3,aac");

        // Subtitles
        if self.options.subtitles {
            cmd.arg("--all-subtitles");
        }

        // Deinterlace / decomb
        if self.options.deinterlace {
            cmd.arg("--comb-detect").arg("--decomb");
        }

        // Keep chapters
        cmd.arg("--markers");

        cmd
    }

    /// Parses HandBrake progress output line:
    /// `Encoding: task 1 of 1, 45.23 % (120.5 fps, avg 115.2 fps, ETA 00h05m12s)`
    pub fn parse_progress_line(line: &str) -> Option<(f64, String, String)> {
        if !line.contains("Encoding:") {
            return None;
        }

        let pct_part = line.split('%').next()?;
        let pct_str = pct_part.split(',').next_back()?.trim();
        let percent = pct_str.parse::<f64>().ok()?;

        let mut fps = "N/A".to_string();
        if let Some(fps_idx) = line.find("fps") {
            let before = &line[..fps_idx];
            if let Some(start_paren) = before.rfind('(') {
                fps = before[start_paren + 1..].trim().to_string();
            } else if let Some(last_space) = before.trim().rfind(' ') {
                fps = before.trim()[last_space + 1..].trim().to_string();
            }
        }

        let mut speed = "N/A".to_string();
        if let Some(eta_idx) = line.find("ETA") {
            let after = &line[eta_idx + 3..];
            let eta = after.trim_matches(|c: char| c == ' ' || c == ')' || c == ',');
            speed = format!("ETA {}", eta);
        }

        Some((percent, fps, speed))
    }
}

impl Transcoder for HandBrakeTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        self.transcode_chapter_range(job, None, observer, cancel_flag)
    }

    fn transcode_chapter_range(
        &self,
        job: &TranscodeJob,
        chapter_range: Option<(u32, u32)>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let mut cmd = self.build_command(job, chapter_range);

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[HandBrake Engine] Starting encode for '{}' -> {}",
                job.display_title,
                job.output_path.display()
            ));
        }

        if job.output_path.exists() {
            let _ = std::fs::remove_file(job.output_path);
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let raw_child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn HandBrakeCLI ('{}'). Ensure it is installed and in PATH.",
                self.handbrake_path
            )
        })?;
        let mut child = ChildGuard::new(raw_child);

        let stdout = child
            .as_mut()
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture HandBrakeCLI stdout"))?;
        let reader = BufReader::new(stdout);

        for line_res in reader.lines() {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                if job.output_path.exists() {
                    let _ = std::fs::remove_file(job.output_path);
                }
                return Err(anyhow!("HandBrake transcoding cancelled by user."));
            }

            if let Ok(line) = line_res {
                if let Some((pct, fps, speed)) = Self::parse_progress_line(&line) {
                    if let Some(obs) = observer {
                        obs.on_progress(pct, &fps, &speed);
                    }
                } else if let Some(obs) = observer
                    && (line.contains("ERROR") || line.contains("Error"))
                {
                    obs.on_log(&format!("[HandBrake] {}", line));
                }
            }
        }

        let status = child
            .as_mut()
            .wait()
            .context("Failed to wait on HandBrakeCLI process")?;
        child.disarm();

        if !status.success() {
            return Err(anyhow!(
                "HandBrakeCLI exited with non-zero status code: {:?}",
                status.code()
            ));
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
    use std::path::Path;

    #[test]
    fn test_parse_handbrake_progress() {
        let line = "Encoding: task 1 of 1, 45.23 % (120.50 fps, avg 115.20 fps, ETA 00h05m12s)";
        let parsed = HandBrakeTranscoder::parse_progress_line(line);
        assert!(parsed.is_some());
        let (pct, fps, speed) = parsed.unwrap();
        assert!((pct - 45.23).abs() < 0.001);
        assert_eq!(fps, "120.50");
        assert!(speed.contains("00h05m12s"));

        // Non-progress line
        assert!(HandBrakeTranscoder::parse_progress_line("Scanning title 1...").is_none());
        assert!(HandBrakeTranscoder::parse_progress_line("Encoding:").is_none());
    }

    #[test]
    fn test_handbrake_is_available_nonexistent() {
        assert!(!HandBrakeTranscoder::is_available("nonexistent_handbrake_xyz_1234"));
    }

    #[test]
    fn test_handbrake_build_command_permutations() {
        let options_h265 = TranscodeOptions {
            codec: "hevc".to_string(),
            crf: Some(18),
            preset: "ultrafast".to_string(),
            deinterlace: true,
            all_audio: true,
            subtitles: true,
            ..Default::default()
        };
        let hb = HandBrakeTranscoder::new("HandBrakeCLI", options_h265);
        let job = TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: Path::new("out.mkv"),
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        };

        let cmd = hb.build_command(&job, Some((1, 4)));
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"-e".to_string()));
        assert!(args.contains(&"x265".to_string()));
        assert!(args.contains(&"-q".to_string()));
        assert!(args.contains(&"18".to_string()));
        assert!(args.contains(&"--encoder-preset".to_string()));
        assert!(args.contains(&"veryfast".to_string()));
        assert!(args.contains(&"--comb-detect".to_string()));
        assert!(args.contains(&"--decomb".to_string()));
        assert!(args.contains(&"--all-audio".to_string()));
        assert!(args.contains(&"--all-subtitles".to_string()));
        assert!(args.contains(&"-c".to_string()));
        assert!(args.contains(&"1-4".to_string()));
        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"1".to_string()));

        // Test title 0 -> main-feature, av1 codec, slow preset
        let options_av1 = TranscodeOptions {
            codec: "av1".to_string(),
            crf: None,
            preset: "veryslow".to_string(),
            deinterlace: false,
            all_audio: false,
            subtitles: false,
            ..Default::default()
        };
        let hb_av1 = HandBrakeTranscoder::new("HandBrakeCLI", options_av1);
        let job_main = TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: Path::new("out.mkv"),
            title_number: 0,
            display_title: "Main",
            expected_duration_secs: None,
        };
        let cmd_main = hb_av1.build_command(&job_main, None);
        let args_main: Vec<String> = cmd_main
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args_main.contains(&"--main-feature".to_string()));
        assert!(args_main.contains(&"svt_av1".to_string()));
        assert!(args_main.contains(&"slow".to_string()));
        assert!(args_main.contains(&"-a".to_string()));

        // Test mpeg2 codec
        let options_mpeg2 = TranscodeOptions {
            codec: "mpeg2".to_string(),
            crf: None,
            preset: "faster".to_string(),
            deinterlace: false,
            all_audio: false,
            subtitles: false,
            ..Default::default()
        };
        let hb_mpeg2 = HandBrakeTranscoder::new("HandBrakeCLI", options_mpeg2);
        let cmd_mpeg2 = hb_mpeg2.build_command(&job_main, None);
        let args_mpeg2: Vec<String> = cmd_mpeg2
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args_mpeg2.contains(&"mpeg2".to_string()));
        assert!(args_mpeg2.contains(&"fast".to_string()));
    }

    #[test]
    fn test_handbrake_transcode_nonexistent_binary() {
        let hb = HandBrakeTranscoder::new("nonexistent_hb_bin_xyz", TranscodeOptions::default());
        let job = TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: Path::new("out.mkv"),
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        };
        let res = hb.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Failed to spawn HandBrakeCLI"));
    }
}
