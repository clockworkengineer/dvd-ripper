use crate::domain::ports::notification::{NotificationEvent, Notifier};
use anyhow::Result;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_server_notifier_lifecycle_and_non_success_events() {
        let notifier = MediaServerNotifier::new();
        assert!(notifier.plex_url.is_none());
        assert!(notifier.jellyfin_url.is_none());
        assert!(notifier.emby_url.is_none());

        let event = NotificationEvent::RipStarted {
            title: "Test",
            drive: "D:\\",
        };
        assert!(notifier.send(&event).is_ok());

        let cloned = notifier.clone();
        assert_eq!(format!("{:?}", notifier), format!("{:?}", cloned));
    }

    #[test]
    fn test_media_server_notifier_with_urls() {
        let notifier = MediaServerNotifier {
            plex_url: Some("http://127.0.0.1:32400".to_string()),
            plex_token: Some("token123".to_string()),
            jellyfin_url: Some("http://127.0.0.1:8096".to_string()),
            jellyfin_api_key: Some("key123".to_string()),
            emby_url: Some("http://127.0.0.1:8096".to_string()),
            emby_api_key: Some("key456".to_string()),
        };

        let success_event = NotificationEvent::RipSuccess {
            title: "Test Movie",
            output_file: "test.mp4",
            duration_secs: 120.0,
        };
        // Calling send should trigger utils scans (which fail harmlessly or succeed) and return Ok(())
        let res = notifier.send(&success_event);
        assert!(res.is_ok());
    }
}
