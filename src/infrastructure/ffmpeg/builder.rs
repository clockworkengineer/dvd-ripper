use std::path::Path;
use std::process::Command;
use crate::cli::Args;

/// Builds an FFmpeg Command configured with parameters for the specified input title and target output.
pub fn build_ffmpeg_command(
    args: &Args,
    dvd_path: &Path,
    output_path: &Path,
    title_num: u32,
) -> Command {
    crate::ffmpeg::build_ffmpeg_command(args, dvd_path, output_path, title_num)
}
