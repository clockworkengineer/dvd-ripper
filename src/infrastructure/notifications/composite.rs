use anyhow::Result;
use crate::domain::ports::notification::{NotificationEvent, Notifier};

/// Composite notifier that dispatches events to multiple registered Notifier implementations (Composite Pattern).
pub struct CompositeNotifier {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl CompositeNotifier {
    pub fn new(notifiers: Vec<Box<dyn Notifier>>) -> Self {
        Self { notifiers }
    }

    pub fn add(&mut self, notifier: Box<dyn Notifier>) {
        self.notifiers.push(notifier);
    }
}

impl Notifier for CompositeNotifier {
    fn send(&self, event: &NotificationEvent) -> Result<()> {
        for n in &self.notifiers {
            if let Err(e) = n.send(event) {
                eprintln!("[CompositeNotifier Warning] Failed to dispatch notification: {}", e);
            }
        }
        Ok(())
    }
}
