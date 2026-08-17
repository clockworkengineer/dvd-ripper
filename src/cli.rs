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
    /// DVD drive letter, device path, or 'auto' for automatic cross-platform optical drive detection (e.g., auto, D:, /dev/sr0)
    #[arg(default_value = "auto")]
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

    /// Output container in Matroska (.mkv) format to preserve raw DVD bitmap subtitles losslessly
    #[arg(long)]
    pub mkv: bool,

    /// Video codec for transcoding (e.g. h264, hevc, av1, copy)
    #[arg(long, default_value = "h264")]
    pub codec: String,

    /// Transcoding preset profile (e.g. standard, archival, plex, mobile)
    #[arg(long, default_value = "standard")]
    pub profile: String,

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

    /// Normalize audio loudness across streams using EBU R128 (-filter:a loudnorm)
    #[arg(long = "normalize-audio")]
    pub normalize_audio: bool,

    /// Generate dual audio streams (Track 1: Stereo AAC normalized, Track 2: 5.1 Surround Passthrough)
    #[arg(long = "dual-audio")]
    pub dual_audio: bool,

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

    /// Optional post-processing script executable path to run upon rip completion
    #[arg(long = "post-script")]
    pub post_script: Option<String>,

    /// Optional API key parameter to secure REST API endpoints with Bearer token authentication
    #[arg(long = "api-key")]
    pub api_key: Option<String>,

    /// Search query term to query IMDb/OMDb metadata candidates
    #[arg(short = 's', long = "search")]
    pub search: Option<String>,

    /// Select specific IMDb ID directly (e.g. tt0090605)
    #[arg(long = "imdb-id")]
    pub imdb_id: Option<String>,

    /// Select 1-based index candidate directly from search results
    #[arg(long = "select-index")]
    pub select_index: Option<usize>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            input: "D:\\".to_string(),
            output: None,
            out_dir: "Films".to_string(),
            title: 0,
            transcode: false,
            mkv: false,
            codec: "h264".to_string(),
            profile: "standard".to_string(),
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
            normalize_audio: false,
            dual_audio: false,
            audio_lang: None,
            subtitles: false,
            sub_lang: None,
            webhook_url: None,
            no_overwrite: false,
            post_script: None,
            api_key: None,
            search: None,
            imdb_id: None,
            select_index: None,
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

    #[test]
    fn test_args_search_flags() {
        let mut args = Args::default();
        assert!(args.search.is_none());
        assert!(args.imdb_id.is_none());
        assert!(args.select_index.is_none());

        args.search = Some("Kill Bill".to_string());
        args.imdb_id = Some("tt0266697".to_string());
        args.select_index = Some(1);

        assert_eq!(args.search.as_deref(), Some("Kill Bill"));
        assert_eq!(args.imdb_id.as_deref(), Some("tt0266697"));
        assert_eq!(args.select_index, Some(1));
    }
}
