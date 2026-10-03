use anyhow::Result;
use crate::domain::ports::notification::{NotificationEvent, Notifier};

/// Dispatches notifications over MQTT 3.1.1 byte packets.
#[derive(Debug, Clone)]
pub struct MqttNotifier {
    pub broker: String,
    pub topic_prefix: String,
}

impl MqttNotifier {
    pub fn new(broker: impl Into<String>, topic_prefix: impl Into<String>) -> Self {
        Self {
            broker: broker.into(),
            topic_prefix: topic_prefix.into(),
        }
    }
}

impl Notifier for MqttNotifier {
    fn send(&self, event: &NotificationEvent) -> Result<()> {
        let (title, status, progress) = match event {
            NotificationEvent::RipStarted { title, .. } => (*title, "ripping", 0.0),
            NotificationEvent::RipProgress { title, percent } => (*title, "ripping", *percent),
            NotificationEvent::RipSuccess { title, .. } => (*title, "completed", 100.0),
            NotificationEvent::RipFailed { title, .. } => (*title, "failed", 0.0),
            NotificationEvent::RipCancelled { title } => (*title, "idle", 0.0),
        };

        crate::mqtt::publish_mqtt_status(&self.broker, title, status, progress)
    }
}
