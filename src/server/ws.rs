use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

use crate::server::api::AppState;
use crate::types::{ClientCommand, EventType, PaneEvent};

#[derive(Deserialize)]
pub struct WsQuery {
    pub pane_id: Option<String>,
}

const PING_INTERVAL_SECS: u64 = 30;

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let pane_filter = query.pane_id.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, state, pane_filter))
}

async fn handle_socket(socket: WebSocket, state: AppState, pane_filter: Option<String>) {
    let (mut sink, mut stream) = socket.split();
    let mut rx = state.manager.broadcaster().subscribe();
    let mut ping_tick = interval(Duration::from_secs(PING_INTERVAL_SECS));

    // Send snapshot for initial pane_id
    if let Some(ref pane_id) = pane_filter {
        if let Some((lines, total)) = state.manager.registry().snapshot(pane_id) {
            let stream_id = state
                .manager
                .registry()
                .stream_id(pane_id)
                .unwrap_or_default();

            let snapshot_event = PaneEvent {
                stream_id,
                pane_id: pane_id.clone(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_type: EventType::Snapshot {
                    lines,
                    total_lines: total,
                },
            };

            if let Ok(json) = serde_json::to_string(&snapshot_event) {
                let _ = sink.send(Message::Text(json)).await;
            }
        }
    }

    info!("WS client connected filter={pane_filter:?}");

    loop {
        tokio::select! {
            // Incoming client message
            msg = stream.next() => {
                match msg {
                    None | Some(Err(_)) => break, // Client disconnected
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(txt))) => {
                        if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&txt) {
                            handle_command(cmd, &state, &mut sink).await;
                        }
                    }
                    Some(Ok(_)) => {}
                }
            }

            // Broadcast event from any pane
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        // Filter by pane_id if specified
                        if let Some(ref filter) = pane_filter {
                            if &event.pane_id != filter {
                                continue;
                            }
                        }
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                if sink.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => warn!("JSON serialize error: {e}"),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        let lag_event = PaneEvent {
                            stream_id:  "system".to_string(),
                            pane_id:    pane_filter.clone().unwrap_or_default(),
                            timestamp:  chrono::Utc::now().timestamp_millis(),
                            event_type: EventType::Error {
                                message: format!("Slow consumer: {n} lines dropped"),
                            },
                        };
                        if let Ok(json) = serde_json::to_string(&lag_event) {
                            let _ = sink.send(Message::Text(json)).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            // Periodic ping
            _ = ping_tick.tick() => {
                let ping_event = PaneEvent {
                    stream_id:  "system".to_string(),
                    pane_id:    String::new(),
                    timestamp:  chrono::Utc::now().timestamp_millis(),
                    event_type: EventType::Ping,
                };
                if let Ok(json) = serde_json::to_string(&ping_event) {
                    if sink.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    info!("WS client disconnected filter={pane_filter:?}");
}

async fn handle_command(
    cmd: ClientCommand,
    state: &AppState,
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
) {
    match cmd {
        ClientCommand::Replay { pane_id, lines } => {
            if let Some((mut evts, total)) = state.manager.registry().snapshot(&pane_id) {
                if let Some(n) = lines {
                    let skip = evts.len().saturating_sub(n);
                    evts = evts.into_iter().skip(skip).collect();
                }
                let stream_id = state
                    .manager
                    .registry()
                    .stream_id(&pane_id)
                    .unwrap_or_default();
                let snap = PaneEvent {
                    stream_id,
                    pane_id: pane_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    event_type: EventType::Snapshot {
                        lines: evts,
                        total_lines: total,
                    },
                };
                if let Ok(json) = serde_json::to_string(&snap) {
                    let _ = sink.send(Message::Text(json)).await;
                }
            }
        }
        ClientCommand::Subscribe { pane_id } => {
            debug!(pane_id, "subscribe command received");
            // subscriptions are implicit via pane_id query param
        }
        ClientCommand::Unsubscribe { pane_id } => {
            debug!(pane_id, "unsubscribe command received");
            // reserved for future multi-pane subscriptions
        }
    }
}
