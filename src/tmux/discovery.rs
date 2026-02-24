use std::collections::HashSet;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::tmux::TmuxManager;

/// Auto-discover tmux panes matching configured sessions and register/unregister them.
pub async fn run(manager: TmuxManager, mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
    let interval_ms = manager.config.tmux.discovery_interval_ms;
    let sessions = manager.config.tmux.sessions_to_watch.clone();
    let mut tick = interval(Duration::from_millis(interval_ms));

    info!("Discovery loop started (interval={interval_ms}ms, sessions={sessions:?})");

    loop {
        tokio::select! {
            _ = tick.tick() => {
                discover_once(&manager, &sessions).await;
            }
            _ = shutdown_rx.recv() => {
                info!("Discovery loop shutting down");
                break;
            }
        }
    }
}

async fn discover_once(manager: &TmuxManager, sessions: &[String]) {
    let output = match tokio::process::Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}:#{window_index}.#{pane_index}",
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            warn!("tmux list-panes failed: {e}");
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let live_panes: HashSet<String> = stdout
        .lines()
        .filter(|line| sessions.iter().any(|s| line.starts_with(s)))
        .map(|s| s.to_string())
        .collect();

    debug!(
        "Discovery found {} panes matching sessions",
        live_panes.len()
    );

    // Attach any newly discovered panes
    for pane_id in &live_panes {
        if !manager.registry().contains(pane_id) {
            info!("Discovery: auto-attaching pane={pane_id}");
            if let Err(e) = manager.attach(pane_id, None).await {
                warn!("Discovery: attach failed for {pane_id}: {e}");
            }
        }
    }

    // Detach panes that are no longer present
    let registered: Vec<String> = manager
        .registry()
        .list()
        .into_iter()
        .map(|p| p.pane_id)
        .collect();

    for pane_id in registered {
        if !live_panes.contains(&pane_id) {
            info!("Discovery: auto-detaching gone pane={pane_id}");
            manager.detach(&pane_id).await;
        }
    }
}
