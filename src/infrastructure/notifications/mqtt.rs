use crate::domain::ports::notification::{NotificationEvent, Notifier};
use anyhow::Result;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_mqtt_notifier_send_all_events() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let broker = format!("127.0.0.1:{}", port);

        // Spawn mock broker responder thread
        let handle = thread::spawn(move || {
            for _ in 0..5 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                }
            }
        });

        let notifier = MqttNotifier::new(&broker, "test/dvd");
        assert_eq!(notifier.broker, broker);
        assert_eq!(notifier.topic_prefix, "test/dvd");

        let events = [
            NotificationEvent::RipStarted {
                title: "Movie 1",
                drive: "D:\\",
            },
            NotificationEvent::RipProgress {
                title: "Movie 1",
                percent: 45.0,
            },
            NotificationEvent::RipSuccess {
                title: "Movie 1",
                output_file: "out.mp4",
                duration_secs: 100.0,
            },
            NotificationEvent::RipFailed {
                title: "Movie 1",
                error_message: "Error",
            },
            NotificationEvent::RipCancelled { title: "Movie 1" },
        ];

        for ev in &events {
            let _ = notifier.send(ev);
        }

        let _ = handle.join();
        let cloned = notifier.clone();
        assert_eq!(format!("{:?}", notifier), format!("{:?}", cloned));
    }
}
