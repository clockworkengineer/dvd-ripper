use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use anyhow::Result;
use crate::cli::Args;
use crate::ffmpeg::ProgressEvent;

/// Executes an FFmpeg transcode process with live stderr progress parsing and graceful cancellation support.
pub fn run_ffmpeg_process(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    display_title: &str,
    expected_runtime_secs: Option<f64>,
    tx: Option<Sender<ProgressEvent>>,
    cancel_rx: Option<Receiver<()>>,
    cancel_flag: Option<Arc<AtomicBool>>,
    is_batch: bool,
) -> Result<()> {
    crate::ffmpeg::run_ffmpeg_with_channel(
        args,
        dvd_path,
        absolute_output,
        display_title,
        expected_runtime_secs,
        tx,
        cancel_rx,
        cancel_flag,
        is_batch,
    )
}
