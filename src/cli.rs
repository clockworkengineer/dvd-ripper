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

    /// Optional MQTT broker address (e.g. 192.168.1.50:1883) for Home Assistant smart home telemetry
    #[arg(long = "mqtt-broker")]
    pub mqtt_broker: Option<String>,

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

    /// Include all audio tracks from DVD title in output
    #[arg(long = "all-audio")]
    pub all_audio: bool,

    /// Preferred audio track language code (e.g. eng, fre, spa)
    #[arg(long = "audio-lang")]
    pub audio_lang: Option<String>,

    /// Extract subtitle tracks from DVD title into output container
    #[arg(long)]
    pub subtitles: bool,

    /// Preferred subtitle track language code (e.g. eng, fre, spa)
    #[arg(long = "sub-lang")]
    pub sub_lang: Option<String>,

    /// Webhook URL (e.g. Discord, Slack, Ntfy, Telegram) for HTTP status POST notifications
    #[arg(long = "webhook-url")]
    pub webhook_url: Option<String>,

    /// Do not overwrite existing destination files (auto-append incremental numeric suffix)
    #[arg(long = "no-overwrite")]
    pub no_overwrite: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            input: "D:\\".to_string(),
            output: None,
            out_dir: "Films".to_string(),
            title: 0,
            transcode: false,
            preset: "veryfast".to_string(),
            ffmpeg: "ffmpeg".to_string(),
            hwaccel: "copy".to_string(),
            cli: false,
            daemon: false,
            mqtt_broker: None,
            tv: false,
            season: 1,
            start_episode: 1,
            all_episodes: false,
            all_audio: false,
            audio_lang: None,
            subtitles: false,
            sub_lang: None,
            webhook_url: None,
            no_overwrite: false,
        }
    }
}

impl Args {
    pub fn new_movie(
        input: String,
        out_dir: String,
        title: u32,
        transcode: bool,
        preset: String,
        ffmpeg: String,
    ) -> Self {
        Self {
            input,
            out_dir,
            title,
            transcode,
            preset,
            ffmpeg,
            ..Default::default()
        }
    }

    pub fn new_tv(
        input: String,
        out_dir: String,
        title: u32,
        season: u32,
        start_ep: u32,
        transcode: bool,
        preset: String,
        ffmpeg: String,
    ) -> Self {
        Self {
            input,
            out_dir,
            title,
            tv: true,
            season,
            start_episode: start_ep,
            transcode,
            preset,
            ffmpeg,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_defaults() {
        let args = Args::default();
        assert_eq!(args.input, "D:\\");
        assert_eq!(args.out_dir, "Films");
        assert_eq!(args.title, 0);
        assert!(!args.transcode);
        assert_eq!(args.preset, "veryfast");
        assert!(!args.tv);
        assert!(!args.all_audio);
        assert!(!args.subtitles);
        assert!(!args.no_overwrite);
    }

    #[test]
    fn test_args_new_movie() {
        let args = Args::new_movie(
            "E:\\".to_string(),
            "MoviesOut".to_string(),
            2,
            true,
            "fast".to_string(),
            "custom_ffmpeg".to_string(),
        );

        assert_eq!(args.input, "E:\\");
        assert_eq!(args.out_dir, "MoviesOut");
        assert_eq!(args.title, 2);
        assert!(args.transcode);
        assert_eq!(args.preset, "fast");
        assert_eq!(args.ffmpeg, "custom_ffmpeg");
        assert!(!args.tv);
    }

    #[test]
    fn test_args_new_tv() {
        let args = Args::new_tv(
            "F:\\".to_string(),
            "TVOut".to_string(),
            1,
            3,
            5,
            false,
            "medium".to_string(),
            "ffmpeg".to_string(),
        );

        assert_eq!(args.input, "F:\\");
        assert_eq!(args.out_dir, "TVOut");
        assert_eq!(args.title, 1);
        assert!(args.tv);
        assert_eq!(args.season, 3);
        assert_eq!(args.start_episode, 5);
        assert!(!args.transcode);
    }
}
