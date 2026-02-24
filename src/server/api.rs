use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::warn;

use crate::tmux::TmuxManager;
use crate::types::PaneId;

#[derive(Clone)]
pub struct AppState {
    pub manager:         TmuxManager,
    pub trades_data_dir: String,
}

// ─── Request / response types ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub pane_id: PaneId,
    pub label:   Option<String>,
}

#[derive(Deserialize)]
pub struct ReplayQuery {
    pub lines: Option<usize>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────

/// GET /api/panes
pub async fn list_panes(State(state): State<AppState>) -> impl IntoResponse {
    let panes = state.manager.registry().list();
    Json(panes)
}

/// POST /api/panes
pub async fn register_pane(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    match state.manager.attach(&req.pane_id, req.label).await {
        Ok(()) => {
            let stream_id = state.manager.registry()
                .stream_id(&req.pane_id)
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "pane_id": req.pane_id, "stream_id": stream_id })),
            )
        }
        Err(e) => {
            warn!("register_pane error: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "message": e })),
            )
        }
    }
}

/// DELETE /api/panes/:id
pub async fn unregister_pane(
    State(state): State<AppState>,
    Path(pane_id): Path<String>,
) -> impl IntoResponse {
    state.manager.detach(&pane_id).await;
    Json(serde_json::json!({ "ok": true }))
}

/// GET /api/panes/:id/replay
pub async fn replay_pane(
    State(state): State<AppState>,
    Path(pane_id): Path<String>,
    Query(q): Query<ReplayQuery>,
) -> impl IntoResponse {
    match state.manager.registry().snapshot(&pane_id) {
        Some((mut lines, total)) => {
            if let Some(n) = q.lines {
                let skip = lines.len().saturating_sub(n);
                lines = lines.into_iter().skip(skip).collect();
            }
            (StatusCode::OK, Json(serde_json::json!({ "ok": true, "lines": lines, "total": total })))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "ok": false, "message": "pane not found" })),
        ),
    }
}
