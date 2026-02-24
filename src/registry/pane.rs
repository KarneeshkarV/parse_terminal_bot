use crate::types::{PaneEvent, StreamId};
use std::collections::VecDeque;
use tokio::sync::oneshot;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PaneState {
    Attached,
    Detached,
}

pub struct PaneEntry {
    pub stream_id: StreamId,
    pub label: Option<String>,
    pub state: PaneState,
    pub line_count: u64,
    /// Bounded replay buffer
    pub replay: VecDeque<PaneEvent>,
    pub replay_cap: usize,
    /// Signal sender to stop the pipe reader task
    pub shutdown_tx: Option<oneshot::Sender<()>>,
}

impl PaneEntry {
    pub fn new(stream_id: StreamId, label: Option<String>, replay_cap: usize) -> Self {
        Self {
            stream_id,
            label,
            state: PaneState::Detached,
            line_count: 0,
            replay: VecDeque::with_capacity(replay_cap),
            replay_cap,
            shutdown_tx: None,
        }
    }

    pub fn push_line(&mut self, event: PaneEvent) {
        if self.replay.len() >= self.replay_cap {
            self.replay.pop_front();
        }
        self.replay.push_back(event);
        self.line_count += 1;
    }

    pub fn snapshot(&self) -> Vec<PaneEvent> {
        self.replay.iter().cloned().collect()
    }
}
