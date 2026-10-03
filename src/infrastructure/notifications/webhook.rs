use anyhow::Result;
use crate::domain::ports::notification::{NotificationEvent, Notifier};

/// Dispatches JSON notifications via HTTP webhook (Discord, Slack, Ntfy, generic POST).
#[derive(Debug, Clone)]
pub struct WebhookNotifier {
    pub webhook_url: String,
    pub webhook_secret: Option<String>,
}

impl WebhookNotifier {
    pub fn new(webhook_url: impl Into<String>, webhook_secret: Option<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            webhook_secret,
        }
    }
}

impl Notifier for WebhookNotifier {
    fn send(&self, event: &NotificationEvent) -> Result<()> {
        let (title, status, message) = match event {
            NotificationEvent::RipStarted { title, drive } => {
                (*title, "Started", format!("Began ripping '{}' from drive {}", title, drive))
            }
            NotificationEvent::RipProgress { title, percent } => {
                (*title, "Progress", format!("{:.1}% complete", percent))
            }
            NotificationEvent::RipSuccess { title, output_file, duration_secs } => {
                (*title, "Success", format!("Successfully ripped to '{}' ({:.1}s)", output_file, duration_secs))
            }
            NotificationEvent::RipFailed { title, error_message } => {
                (*title, "Failed", format!("Rip failed: {}", error_message))
            }
            NotificationEvent::RipCancelled { title } => {
                (*title, "Cancelled", "Rip job was cancelled by user".to_string())
            }
        };

        crate::mqtt::send_webhook_notification(
            &self.webhook_url,
            title,
            status,
            &message,
            self.webhook_secret.as_deref(),
        )
    }
}
