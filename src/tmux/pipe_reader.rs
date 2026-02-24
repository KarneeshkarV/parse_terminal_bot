use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

use crate::broadcaster::Broadcaster;
use crate::config::ProjectSource;
use crate::parser::{parse_ansi, semantic::SemanticParser};
use crate::registry::PaneRegistry;
use crate::types::{EventType, PaneEvent};

/// Reads from a named FIFO, parses lines, broadcasts events.
pub async fn run(
    pane_id: String,
    fifo_path: PathBuf,
    source: ProjectSource,
    registry: PaneRegistry,
    broadcaster: Broadcaster,
    shutdown_rx: oneshot::Receiver<()>,
) {
    info!(
        "pipe_reader starting for pane={pane_id} fifo={}",
        fifo_path.display()
    );

    // Open FIFO for reading. tokio::fs::File on a FIFO blocks until a writer
    // attaches (which happens when tmux pipe-pane runs).
    let file = match File::open(&fifo_path).await {
        Ok(f) => f,
        Err(e) => {
            error!("Cannot open FIFO {}: {e}", fifo_path.display());
            return;
        }
    };

    let mut reader = BufReader::new(file);
    let mut line_buf = String::new();
    let mut semantic = SemanticParser::new(source);
    let mut shutdown_rx = std::pin::pin!(shutdown_rx);

    loop {
        line_buf.clear();

        let read_fut = reader.read_line(&mut line_buf);
        tokio::select! {
            result = read_fut => {
                match result {
                    Ok(0) => {
                        info!("pipe_reader EOF for pane={pane_id}");
                        break;
                    }
                    Ok(_) => {
                        let raw = line_buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
                        process_line(&raw, &pane_id, &registry, &broadcaster, &mut semantic);
                    }
                    Err(e) => {
                        warn!("pipe_reader read error for pane={pane_id}: {e}");
                        break;
                    }
                }
            }
            _ = &mut shutdown_rx => {
                info!("pipe_reader shutdown signal for pane={pane_id}");
                break;
            }
        }
    }

    registry.set_detached(&pane_id);
    info!("pipe_reader stopped for pane={pane_id}");
}

fn process_line(
    raw: &str,
    pane_id: &str,
    registry: &PaneRegistry,
    broadcaster: &Broadcaster,
    semantic: &mut SemanticParser,
) {
    let (clean, style) = parse_ansi(raw);
    let sem = semantic.feed(&clean);

    let stream_id = match registry.stream_id(&pane_id.to_string()) {
        Some(s) => s,
        None => return,
    };

    let line_count = {
        // Read current line count before incrementing
        registry
            .snapshot(&pane_id.to_string())
            .map(|(_, n)| n)
            .unwrap_or(0)
    };

    let event = PaneEvent {
        stream_id: stream_id.clone(),
        pane_id: pane_id.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        event_type: EventType::Line {
            raw: raw.to_string(),
            clean: clean.clone(),
            style,
            semantic: Box::new(sem),
            line_number: line_count,
        },
    };

    registry.push_line(&pane_id.to_string(), event.clone());
    broadcaster.send(event);

    debug!("pane={pane_id} line: {clean}");
}
