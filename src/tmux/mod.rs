pub mod discovery;
pub mod pipe_reader;

use regex::Regex;
use once_cell::sync::Lazy;
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::broadcaster::Broadcaster;
use crate::config::Config;
use crate::registry::PaneRegistry;
use crate::types::{EventType, PaneEvent, PaneId};

static PANE_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Za-z0-9_\-.]+:\d+\.\d+$").unwrap()
});

#[derive(Clone)]
pub struct TmuxManager {
    config:      Config,
    registry:    PaneRegistry,
    broadcaster: Broadcaster,
}

impl TmuxManager {
    pub fn new(config: Config, registry: PaneRegistry, broadcaster: Broadcaster) -> Self {
        Self { config, registry, broadcaster }
    }

    /// Validate pane_id to prevent shell injection.
    fn validate_pane_id(pane_id: &str) -> Result<(), String> {
        if PANE_ID_RE.is_match(pane_id) {
            Ok(())
        } else {
            Err(format!("Invalid pane_id '{pane_id}': must match session:window.pane"))
        }
    }

    /// Attach to a tmux pane: create FIFO, run pipe-pane, spawn reader task.
    pub async fn attach(&self, pane_id: &PaneId, label: Option<String>) -> Result<(), String> {
        Self::validate_pane_id(pane_id)?;

        if self.registry.is_attached(pane_id) {
            return Ok(());
        }

        // Register in registry (idempotent)
        let stream_id = self.registry.register(pane_id, label.clone());

        // Create pipe directory
        let pipe_dir = self.config.pipe_dir();
        tokio::fs::create_dir_all(&pipe_dir).await
            .map_err(|e| format!("Cannot create pipe dir: {e}"))?;

        let fifo_path = pipe_dir.join(format!("{stream_id}.fifo"));

        // Remove stale FIFO if present
        let _ = tokio::fs::remove_file(&fifo_path).await;

        // Create FIFO
        let fifo_str = fifo_path.to_string_lossy().to_string();
        let status = tokio::process::Command::new("mkfifo")
            .arg(&fifo_str)
            .status()
            .await
            .map_err(|e| format!("mkfifo failed: {e}"))?;

        if !status.success() {
            return Err(format!("mkfifo returned non-zero for {fifo_str}"));
        }

        // Set up reader task BEFORE pipe-pane so FIFO has a reader end open
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        self.registry.set_attached(pane_id, shutdown_tx);

        let pane_id_owned  = pane_id.clone();
        let fifo_path_copy = fifo_path.clone();
        let registry_copy  = self.registry.clone();
        let bcast_copy     = self.broadcaster.clone();

        tokio::spawn(async move {
            pipe_reader::run(
                pane_id_owned,
                fifo_path_copy,
                registry_copy,
                bcast_copy,
                shutdown_rx,
            ).await;
        });

        // Close any stale pipe-pane from previous runs before attaching new one
        let _ = tokio::process::Command::new("tmux")
            .args(["pipe-pane", "-t", pane_id.as_str()])
            .status()
            .await;

        // Attach tmux pipe-pane (args as array — no shell interpolation)
        // No -o flag: always force-attach even if a previous pipe existed
        let cmd_str = format!("cat >> {fifo_str}");
        let status = tokio::process::Command::new("tmux")
            .args(["pipe-pane", "-t", pane_id.as_str(), &cmd_str])
            .status()
            .await
            .map_err(|e| format!("tmux pipe-pane failed: {e}"))?;

        if !status.success() {
            warn!("tmux pipe-pane returned non-zero for pane {pane_id}");
        }

        // Emit PaneRegistered event
        let event = PaneEvent {
            stream_id: stream_id.clone(),
            pane_id:   pane_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: EventType::PaneRegistered { label },
        };
        self.broadcaster.send(event);

        info!("Attached to pane={pane_id} stream={stream_id}");
        Ok(())
    }

    /// Detach from a pane: close pipe-pane, signal reader, remove FIFO.
    pub async fn detach(&self, pane_id: &PaneId) {
        if let Err(e) = Self::validate_pane_id(pane_id) {
            warn!("{e}");
            return;
        }

        // Close the write end of the pipe (sends EOF to reader)
        let _ = tokio::process::Command::new("tmux")
            .args(["pipe-pane", "-t", pane_id.as_str()])
            .status()
            .await;

        let stream_id = self.registry.stream_id(pane_id).unwrap_or_default();

        // Signal reader task
        self.registry.unregister(pane_id);

        // Clean up FIFO
        if !stream_id.is_empty() {
            let fifo_path = self.config.pipe_dir().join(format!("{stream_id}.fifo"));
            let _ = tokio::fs::remove_file(&fifo_path).await;
        }

        // Emit unregistered event
        let event = PaneEvent {
            stream_id: stream_id.clone(),
            pane_id:   pane_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            event_type: EventType::PaneUnregistered { reason: "detached".to_string() },
        };
        self.broadcaster.send(event);

        info!("Detached from pane={pane_id}");
    }

    pub fn registry(&self) -> &PaneRegistry {
        &self.registry
    }

    pub fn broadcaster(&self) -> &Broadcaster {
        &self.broadcaster
    }
}
