use anyhow::Result;
use crate::domain::ports::notification::{NotificationEvent, Notifier};

/// Triggers library refresh scans for Plex, Jellyfin, and Emby upon rip success.
#[derive(Debug, Clone, Default)]
pub struct MediaServerNotifier {
    pub plex_url: Option<String>,
    pub plex_token: Option<String>,
    pub jellyfin_url: Option<String>,
    pub jellyfin_api_key: Option<String>,
    pub emby_url: Option<String>,
    pub emby_api_key: Option<String>,
}

impl MediaServerNotifier {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Notifier for MediaServerNotifier {
    fn send(&self, event: &NotificationEvent) -> Result<()> {
        // Only trigger library scans when a rip completes successfully
        if let NotificationEvent::RipSuccess { .. } = event {
            if let (Some(url), Some(token)) = (&self.plex_url, &self.plex_token) {
                let _ = crate::utils::trigger_plex_library_scan(url, token);
            }
            if let (Some(url), Some(key)) = (&self.jellyfin_url, &self.jellyfin_api_key) {
                let _ = crate::utils::trigger_jellyfin_library_scan(url, key);
            }
            if let (Some(url), Some(key)) = (&self.emby_url, &self.emby_api_key) {
                let _ = crate::utils::trigger_emby_library_scan(url, key);
            }
        }
        Ok(())
    }
}
