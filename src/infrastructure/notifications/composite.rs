use crate::domain::ports::notification::{NotificationEvent, Notifier};
use anyhow::Result;

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
                eprintln!(
                    "[CompositeNotifier Warning] Failed to dispatch notification: {}",
                    e
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DummyNotifier {
        count: Arc<AtomicUsize>,
        should_fail: bool,
    }

    impl Notifier for DummyNotifier {
        fn send(&self, _event: &NotificationEvent) -> Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                anyhow::bail!("Simulated notification failure");
            }
            Ok(())
        }
    }

    #[test]
    fn test_composite_notifier_dispatch_and_add() {
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let mut composite = CompositeNotifier::new(vec![Box::new(DummyNotifier {
            count: count1.clone(),
            should_fail: false,
        })]);

        composite.add(Box::new(DummyNotifier {
            count: count2.clone(),
            should_fail: true, // Should log warning but not fail composite
        }));

        let event = NotificationEvent::RipStarted {
            title: "Test Movie",
            drive: "D:\\",
        };
        let res = composite.send(&event);
        assert!(res.is_ok());
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
