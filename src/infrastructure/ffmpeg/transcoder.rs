use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use anyhow::Result;
use crate::domain::ports::transcoder::{ProgressObserver, TranscodeJob, Transcoder};

/// Concrete FFmpeg subprocess transcoder implementing the domain Transcoder port.
#[derive(Debug, Clone)]
pub struct FfmpegTranscoder {
    pub ffmpeg_path: String,
    pub default_args: crate::cli::Args,
}

impl FfmpegTranscoder {
    pub fn new(ffmpeg_path: impl Into<String>) -> Self {
        use clap::Parser;
        let default_args = crate::cli::Args::parse_from(["dvd-ripper"]);
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            default_args,
        }
    }

    pub fn with_args(ffmpeg_path: impl Into<String>, args: crate::cli::Args) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            default_args: args,
        }
    }
}

impl Transcoder for FfmpegTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let mut args = self.default_args.clone();
        if job.title_number > 0 {
            args.title = job.title_number;
        }

        if let Some(obs) = observer {
            let (tx, rx) = std::sync::mpsc::channel();
            let obs_thread = std::thread::spawn(move || {
                let mut captured = Vec::new();
                while let Ok(event) = rx.recv() {
                    captured.push(event);
                }
                captured
            });

            let res = crate::ffmpeg::run_ffmpeg_with_channel(
                &args,
                job.source_dvd,
                job.output_path,
                job.display_title,
                job.expected_duration_secs,
                Some(tx),
                None,
                cancel_flag,
                false,
            );

            if let Ok(events) = obs_thread.join() {
                for event in events {
                    match event {
                        crate::ffmpeg::ProgressEvent::Progress { percent, fps, speed } => {
                            obs.on_progress(percent, &fps, &speed);
                        }
                        crate::ffmpeg::ProgressEvent::Log(line) => {
                            obs.on_log(&line);
                        }
                        _ => {}
                    }
                }
            }

            res
        } else {
            crate::ffmpeg::run_ffmpeg_with_channel(
                &args,
                job.source_dvd,
                job.output_path,
                job.display_title,
                job.expected_duration_secs,
                None,
                None,
                cancel_flag,
                false,
            )
        }
    }
}
