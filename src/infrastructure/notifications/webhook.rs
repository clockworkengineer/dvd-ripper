use crate::domain::ports::notification::{NotificationEvent, Notifier};
use anyhow::Result;

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
            NotificationEvent::RipStarted { title, drive } => (
                *title,
                "Started",
                format!("Began ripping '{}' from drive {}", title, drive),
            ),
            NotificationEvent::RipProgress { title, percent } => {
                (*title, "Progress", format!("{:.1}% complete", percent))
            }
            NotificationEvent::RipSuccess {
                title,
                output_file,
                duration_secs,
            } => (
                *title,
                "Success",
                format!(
                    "Successfully ripped to '{}' ({:.1}s)",
                    output_file, duration_secs
                ),
            ),
            NotificationEvent::RipFailed {
                title,
                error_message,
            } => (*title, "Failed", format!("Rip failed: {}", error_message)),
            NotificationEvent::RipCancelled { title } => (
                *title,
                "Cancelled",
                "Rip job was cancelled by user".to_string(),
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_webhook_notifier_all_events() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/webhook", port);

        let handle = thread::spawn(move || {
            for _ in 0..5 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let resp = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        });

        let notifier = WebhookNotifier::new(&url, Some("secret123".to_string()));
        assert_eq!(notifier.webhook_url, url);
        assert_eq!(notifier.webhook_secret.as_deref(), Some("secret123"));

        let events = [
            NotificationEvent::RipStarted {
                title: "Movie 1",
                drive: "D:\\",
            },
            NotificationEvent::RipProgress {
                title: "Movie 1",
                percent: 50.0,
            },
            NotificationEvent::RipSuccess {
                title: "Movie 1",
                output_file: "out.mp4",
                duration_secs: 120.0,
            },
            NotificationEvent::RipFailed {
                title: "Movie 1",
                error_message: "Transcode failed",
            },
            NotificationEvent::RipCancelled { title: "Movie 1" },
        ];

        for ev in &events {
            let res = notifier.send(ev);
            assert!(res.is_ok());
        }

        let _ = handle.join();
        let cloned = notifier.clone();
        assert_eq!(format!("{:?}", notifier), format!("{:?}", cloned));
    }
}
