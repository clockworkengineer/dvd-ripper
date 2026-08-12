/**
 * @file cli.rs
 * @brief Command line argument parsing definitions.
 */

use clap::Parser;

/// Command line arguments parsed by clap.
#[derive(Parser, Debug, Clone)]
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

    /// Destination directory for ripped output. Defaults to "Films".
    #[arg(short = 'd', long = "out-dir", default_value = "Films")]
    pub out_dir: String,

    /// Specific DVD title number to rip (e.g. 1, 2). 0 auto-detects the title with the longest duration.
    #[arg(short, long, default_value_t = 0)]
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

    /// Hardware acceleration mode for embedded transcoding (e.g. copy, v4l2m2m, vaapi, nvenc, qsv)
    #[arg(long = "hwaccel", default_value = "copy")]
    pub hwaccel: String,

    /// Force command-line interface mode instead of GUI
    #[arg(long)]
    pub cli: bool,

    /// Run as a headless embedded appliance daemon watching for optical disc insertion
    #[arg(long)]
    pub daemon: bool,

    /// Enable TV series disc ripping mode
    #[arg(long)]
    pub tv: bool,

    /// Season number for TV series mode (e.g. 1 for Season 01)
    #[arg(long, default_value_t = 1)]
    pub season: u32,

    /// Starting episode number for the first detected episode title on disc (default: 1)
    #[arg(long = "start-episode", default_value_t = 1)]
    pub start_episode: u32,

    /// Automatically rip all detected TV episode titles on the disc sequentially
    #[arg(long = "all-episodes")]
    pub all_episodes: bool,
}
