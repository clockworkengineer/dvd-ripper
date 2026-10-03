use anyhow::Result;

/// Canonical event types emitted during ripping lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEvent<'a> {
    RipStarted { title: &'a str, drive: &'a str },
    RipProgress { title: &'a str, percent: f64 },
    RipSuccess { title: &'a str, output_file: &'a str, duration_secs: f64 },
    RipFailed { title: &'a str, error_message: &'a str },
    RipCancelled { title: &'a str },
}

/// Abstraction for outward event notifications (Webhook, MQTT, Media Server).
pub trait Notifier: Send + Sync {
    fn send(&self, event: &NotificationEvent) -> Result<()>;
}
