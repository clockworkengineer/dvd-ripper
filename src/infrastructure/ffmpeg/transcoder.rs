use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Concrete FFmpeg subprocess transcoder implementing the domain Transcoder port.
#[derive(Debug, Clone)]
pub struct FfmpegTranscoder {
    pub ffmpeg_path: String,
    pub options: TranscodeOptions,
}

impl FfmpegTranscoder {
    /// Creates a new FfmpegTranscoder with default transcoding options.
    ///
    /// # Parameters
    /// - `ffmpeg_path`: Path or executable name for the FFmpeg binary.
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::infrastructure::ffmpeg::FfmpegTranscoder;
    ///
    /// let transcoder = FfmpegTranscoder::new("ffmpeg");
    /// assert_eq!(transcoder.ffmpeg_path, "ffmpeg");
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn new(ffmpeg_path: impl Into<String>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            options: TranscodeOptions::default(),
        }
    }

    /// Creates a new FfmpegTranscoder with typed [`TranscodeOptions`].
    ///
    /// # Examples
    ///
    /// ```
    /// use dvd_ripper::infrastructure::ffmpeg::FfmpegTranscoder;
    /// use dvd_ripper::domain::ports::TranscodeOptions;
    ///
    /// let options = TranscodeOptions {
    ///     codec: "hevc".to_string(),
    ///     ..Default::default()
    /// };
    /// let transcoder = FfmpegTranscoder::with_options("ffmpeg", options);
    /// assert_eq!(transcoder.options.codec, "hevc");
    /// ```
    ///
    /// # Panics
    /// Never panics.
    pub fn with_options(ffmpeg_path: impl Into<String>, options: TranscodeOptions) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            options,
        }
    }

    /// Creates an FfmpegTranscoder with legacy CLI Args for backward compatibility.
    pub fn with_args(ffmpeg_path: impl Into<String>, args: crate::cli::Args) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
            options: (&args).into(),
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
        self.transcode_chapter_range(job, None, observer, cancel_flag)
    }

    fn transcode_chapter_range(
        &self,
        job: &TranscodeJob,
        chapter_range: Option<(u32, u32)>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let mut args = crate::cli::Args::from(&self.options);
        args.ffmpeg = self.ffmpeg_path.clone();
        if job.title_number > 0 {
            args.title = job.title_number;
        }
        if let Some((start_ch, end_ch)) = chapter_range {
            args.start_chapter = Some(start_ch);
            args.end_chapter = Some(end_ch);
        }

        if let Some(obs) = observer {
            let (tx, rx) = std::sync::mpsc::channel();

            let args_clone = args.clone();
            let source_dvd = job.source_dvd.to_path_buf();
            let output_path = job.output_path.to_path_buf();
            let display_title = job.display_title.to_string();
            let expected_duration_secs = job.expected_duration_secs;
            let cancel_clone = cancel_flag;

            let worker = std::thread::spawn(move || {
                crate::ffmpeg::run_ffmpeg_with_channel(
                    &args_clone,
                    &source_dvd,
                    &output_path,
                    &display_title,
                    expected_duration_secs,
                    Some(tx),
                    None,
                    cancel_clone,
                    false,
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
                Err(_) => Err(anyhow::anyhow!("FFmpeg transcoding thread panicked")),
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_transcoder_constructors() {
        let transcoder_default = FfmpegTranscoder::new("ffmpeg_custom");
        assert_eq!(transcoder_default.ffmpeg_path, "ffmpeg_custom");
        assert_eq!(transcoder_default.options.codec, "h264");

        let custom_opts = TranscodeOptions {
            codec: "hevc".to_string(),
            preset: "fast".to_string(),
            ..Default::default()
        };
        let transcoder_custom = FfmpegTranscoder::with_options("ffmpeg", custom_opts);
        assert_eq!(transcoder_custom.options.codec, "hevc");
        assert_eq!(transcoder_custom.options.preset, "fast");

        let args = crate::cli::Args::default();
        let transcoder_args = FfmpegTranscoder::with_args("ffmpeg", args);
        assert_eq!(transcoder_args.ffmpeg_path, "ffmpeg");

        let cloned = transcoder_custom.clone();
        assert_eq!(format!("{:?}", transcoder_custom), format!("{:?}", cloned));
    }

    struct TestObserver;
    impl ProgressObserver for TestObserver {
        fn on_progress(&self, _percent: f64, _fps: &str, _speed: &str) {}
        fn on_log(&self, _line: &str) {}
    }

    #[test]
    fn test_ffmpeg_transcoder_transcode_execution() {
        let transcoder = FfmpegTranscoder::new("ffmpeg");

        let job = TranscodeJob {
            source_dvd: std::path::Path::new("Z:\\DefinitelyNonExistentDVD"),
            title_number: 1,
            output_path: std::path::Path::new("test_out.mp4"),
            display_title: "Test Job",
            expected_duration_secs: Some(3600.0),
        };

        // 1. Without observer
        let res_no_obs = transcoder.transcode(&job, None, None);
        assert!(res_no_obs.is_err());

        // 2. With observer
        let obs = TestObserver;
        let res_obs = transcoder.transcode(&job, Some(&obs), None);
        assert!(res_obs.is_err());
    }
}
