/**
 * @file cli.rs
 * @brief Command line argument parsing definitions.
 */

use clap::Parser;

/// Command line arguments parsed by clap.
#[derive(Parser, Debug)]
#[command(
    name = "dvd-ripper",
    version,
    about = "Rips a DVD title using FFmpeg's dvdvideo demuxer and creates an MPEG/MPEG-4 file"
)]
pub struct Args {
    /// DVD drive letter or root path (e.g., D: or D:\)
    #[arg(default_value = "D:\\")]
    pub input: String,

    /// Output file path. Defaults to output.mp4 (or output.mpg for copy). Overridden if film details are auto-detected.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Specific DVD title number to rip (e.g. 1). 0 defaults to auto-select Title 1.
    #[arg(short, long, default_value_t = 1)]
    pub title: u32,

    /// Re-encode the video/audio instead of doing a fast lossless stream copy
    #[arg(long)]
    pub transcode: bool,

    /// FFmpeg preset for H.264 encoding (e.g. veryfast, superfast, ultrafast, fast, medium)
    #[arg(long, default_value = "veryfast")]
    pub preset: String,

    /// Custom path to FFmpeg executable
    #[arg(long, default_value = "ffmpeg")]
    pub ffmpeg: String,
}
