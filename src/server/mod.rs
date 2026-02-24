pub mod api;
pub mod trades;
pub mod ws;

use axum::{
    routing::{delete, get},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

use api::AppState;
use ws::ws_handler;

pub fn build_router(state: AppState, static_dir: &str) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // REST API
        .route("/api/panes", get(api::list_panes).post(api::register_pane))
        .route("/api/panes/:id", delete(api::unregister_pane))
        .route("/api/panes/:id/replay", get(api::replay_pane))
        .route("/api/trades", get(trades::get_trades))
        // WebSocket
        .route("/ws", get(ws_handler))
        // Static frontend
        .nest_service("/", ServeDir::new(static_dir))
        .layer(cors)
        .with_state(state)
}
