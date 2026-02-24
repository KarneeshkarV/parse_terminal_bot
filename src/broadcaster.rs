use crate::types::PaneEvent;
use tokio::sync::broadcast;

/// A thin wrapper around tokio::sync::broadcast for fan-out delivery.
#[derive(Clone)]
pub struct Broadcaster {
    tx: broadcast::Sender<PaneEvent>,
}

impl Broadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn send(&self, event: PaneEvent) {
        // Ignore send errors — no subscribers is fine
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PaneEvent> {
        self.tx.subscribe()
    }
}
