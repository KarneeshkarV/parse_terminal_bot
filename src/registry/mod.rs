pub mod pane;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::types::{PaneEvent, PaneId};
use pane::{PaneEntry, PaneState};

/// Thread-safe registry of all monitored panes.
#[derive(Clone)]
pub struct PaneRegistry {
    inner: Arc<RwLock<HashMap<PaneId, PaneEntry>>>,
    replay_cap: usize,
}

impl PaneRegistry {
    pub fn new(replay_cap: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            replay_cap,
        }
    }

    /// Register a pane (idempotent — returns existing stream_id if already present).
    pub fn register(&self, pane_id: &PaneId, label: Option<String>) -> String {
        let mut map = self.inner.write();
        if let Some(entry) = map.get(pane_id) {
            return entry.stream_id.clone();
        }
        let stream_id = Uuid::new_v4().to_string();
        let entry = PaneEntry::new(stream_id.clone(), label, self.replay_cap);
        map.insert(pane_id.clone(), entry);
        stream_id
    }

    pub fn unregister(&self, pane_id: &PaneId) {
        let mut map = self.inner.write();
        if let Some(mut entry) = map.remove(pane_id) {
            // Signal reader task to stop
            if let Some(tx) = entry.shutdown_tx.take() {
                let _ = tx.send(());
            }
        }
    }

    pub fn set_attached(&self, pane_id: &PaneId, shutdown_tx: tokio::sync::oneshot::Sender<()>) {
        let mut map = self.inner.write();
        if let Some(entry) = map.get_mut(pane_id) {
            entry.state = PaneState::Attached;
            entry.shutdown_tx = Some(shutdown_tx);
        }
    }

    pub fn set_detached(&self, pane_id: &PaneId) {
        let mut map = self.inner.write();
        if let Some(entry) = map.get_mut(pane_id) {
            entry.state = PaneState::Detached;
            entry.shutdown_tx = None;
        }
    }

    pub fn push_line(&self, pane_id: &PaneId, event: PaneEvent) {
        let mut map = self.inner.write();
        if let Some(entry) = map.get_mut(pane_id) {
            entry.push_line(event);
        }
    }

    pub fn snapshot(&self, pane_id: &PaneId) -> Option<(Vec<PaneEvent>, u64)> {
        let map = self.inner.read();
        map.get(pane_id).map(|e| (e.snapshot(), e.line_count))
    }

    pub fn list(&self) -> Vec<PaneInfo> {
        let map = self.inner.read();
        map.iter()
            .map(|(id, entry)| PaneInfo {
                pane_id: id.clone(),
                stream_id: entry.stream_id.clone(),
                label: entry.label.clone(),
                state: entry.state,
                line_count: entry.line_count,
            })
            .collect()
    }

    pub fn contains(&self, pane_id: &PaneId) -> bool {
        self.inner.read().contains_key(pane_id)
    }

    pub fn is_attached(&self, pane_id: &PaneId) -> bool {
        self.inner
            .read()
            .get(pane_id)
            .map(|e| e.state == PaneState::Attached)
            .unwrap_or(false)
    }

    pub fn stream_id(&self, pane_id: &PaneId) -> Option<String> {
        self.inner.read().get(pane_id).map(|e| e.stream_id.clone())
    }
}

#[derive(serde::Serialize, Clone)]
pub struct PaneInfo {
    pub pane_id: PaneId,
    pub stream_id: String,
    pub label: Option<String>,
    pub state: PaneState,
    pub line_count: u64,
}

impl serde::Serialize for PaneState {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PaneState::Attached => s.serialize_str("attached"),
            PaneState::Detached => s.serialize_str("detached"),
        }
    }
}
