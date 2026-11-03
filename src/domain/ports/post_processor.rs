//! Domain port abstraction for post-transcoding pipeline processing.

use anyhow::Result;
use std::path::PathBuf;

/// Contextual metadata and configuration passed to post-processors upon rip completion.
#[derive(Debug, Clone, Default)]
pub struct PostProcessContext {
    pub output_file: Option<PathBuf>,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub media_type: String,
    pub poster_bytes: Option<Vec<u8>>,
    pub dvd_path: PathBuf,
    pub nfo_enabled: bool,
    pub nfo_tags: Option<String>,
    pub checksum_enabled: bool,
    pub audit_log_path: Option<PathBuf>,
    pub auto_cleanup_days: Option<u64>,
    pub post_script: Option<String>,
    pub media_servers_enabled: bool,
    pub jellyfin_url: Option<String>,
    pub jellyfin_key: Option<String>,
    pub plex_url: Option<String>,
    pub plex_token: Option<String>,
    pub emby_url: Option<String>,
    pub emby_key: Option<String>,
    pub no_eject: bool,
    pub eject_autoclose_secs: Option<u64>,
    pub spindown: bool,
}

impl PostProcessContext {
    pub fn new(
        output_file: Option<PathBuf>,
        title: impl Into<String>,
        dvd_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            output_file,
            title: title.into(),
            dvd_path: dvd_path.into(),
            media_type: "Movie".to_string(),
            ..Default::default()
        }
    }

    pub fn from_args(
        args: &crate::cli::Args,
        output_file: Option<PathBuf>,
        title: &str,
        year: Option<u32>,
        dvd_path: PathBuf,
        poster_bytes: Option<Vec<u8>>,
    ) -> Self {
        let media_type = if args.tv { "TV Series" } else { "Movie" };
        Self {
            output_file,
            title: title.to_string(),
            year,
            season: if args.tv { Some(args.season) } else { None },
            episode: if args.tv {
                Some(args.start_episode)
            } else {
                None
            },
            media_type: media_type.to_string(),
            poster_bytes,
            dvd_path,
            nfo_enabled: args.nfo,
            nfo_tags: args.tags.clone(),
            checksum_enabled: args.checksum,
            audit_log_path: args.audit_log.as_ref().map(PathBuf::from),
            auto_cleanup_days: args.auto_cleanup_days,
            post_script: args.post_script.clone(),
            media_servers_enabled: args.jellyfin_url.is_some()
                || args.plex_url.is_some()
                || args.emby_url.is_some(),
            jellyfin_url: args.jellyfin_url.clone(),
            jellyfin_key: args.jellyfin_key.clone(),
            plex_url: args.plex_url.clone(),
            plex_token: args.plex_token.clone(),
            emby_url: args.emby_url.clone(),
            emby_key: args.emby_key.clone(),
            no_eject: args.no_eject,
            eject_autoclose_secs: args.eject_autoclose,
            spindown: args.spindown,
        }
    }
}

/// Open/Closed Principle (OCP/SOLID): Pluggable post-processing action executed after rip completion.
pub trait PostProcessor: Send + Sync {
    /// Executes the post-processing action against the completed rip context.
    ///
    /// # Errors
    /// Returns an error if the post-processing action fails critically.
    fn process(&self, ctx: &PostProcessContext) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPostProcessor {
        called: std::sync::atomic::AtomicBool,
    }

    impl PostProcessor for MockPostProcessor {
        fn process(&self, _ctx: &PostProcessContext) -> Result<()> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_mock_post_processor() {
        let proc = MockPostProcessor {
            called: std::sync::atomic::AtomicBool::new(false),
        };
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Inception",
            PathBuf::from("D:\\"),
        );
        proc.process(&ctx).unwrap();
        assert!(proc.called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
