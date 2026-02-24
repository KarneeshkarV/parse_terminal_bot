mod broadcaster;
mod config;
mod parser;
mod registry;
mod server;
mod tmux;
mod types;

use std::net::SocketAddr;
use tokio::signal;
use tokio::sync::broadcast as shutdown_bcast;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use broadcaster::Broadcaster;
use config::{Config, ProjectSource};
use registry::PaneRegistry;
use server::{api::AppState, build_router};
use tmux::TmuxManager;

#[tokio::main]
async fn main() {
    // ── Logging ───────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // ── CLI args + Config ─────────────────────────────────────────────────
    let (config_path, source) = match parse_cli_args() {
        Ok(parsed) => parsed,
        Err(e) => {
            error!("{e}");
            std::process::exit(2);
        }
    };

    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Config error: {e}");
            std::process::exit(1);
        }
    };

    let trades_data_dir = match config.trades_data_dir_for(source) {
        Some(path) => path,
        None => {
            error!(
                "Trades data dir not configured for project={} (set [trades].data_dir or per-project keys)",
                source.as_str()
            );
            std::process::exit(1);
        }
    };
    info!("Using project source: {}", source.as_str());

    // ── Core components ───────────────────────────────────────────────────
    let registry = PaneRegistry::new(config.buffer.replay_lines);
    let broadcaster = Broadcaster::new(config.buffer.channel_capacity);
    let manager = TmuxManager::new(
        config.clone(),
        source,
        registry.clone(),
        broadcaster.clone(),
    );

    // ── Pipe dir ──────────────────────────────────────────────────────────
    if let Err(e) = tokio::fs::create_dir_all(config.pipe_dir()).await {
        error!("Cannot create pipe dir: {e}");
        std::process::exit(1);
    }

    // ── Shutdown channel ──────────────────────────────────────────────────
    let (shutdown_tx, _) = shutdown_bcast::channel::<()>(4);

    // ── Discovery loop ────────────────────────────────────────────────────
    {
        let mgr_clone = manager.clone();
        let shutdown_sub = shutdown_tx.subscribe();
        tokio::spawn(async move {
            tmux::discovery::run(mgr_clone, shutdown_sub).await;
        });
    }

    // ── Attach initial panes ──────────────────────────────────────────────
    for pane_id in &config.tmux.initial_panes {
        if let Err(e) = manager.attach(pane_id, None).await {
            error!("Failed to attach initial pane {pane_id}: {e}");
        }
    }

    // ── HTTP server ───────────────────────────────────────────────────────
    let app_state = AppState {
        manager: manager.clone(),
        source,
        trades_data_dir,
    };
    let router = build_router(app_state, &config.server.static_dir);

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .expect("Invalid server address");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Cannot bind to address");

    info!("Server listening on http://{addr}");
    info!("WebSocket endpoint: ws://{addr}/ws");

    let shutdown_tx_clone = shutdown_tx.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("Shutdown signal received — stopping server");
            let _ = shutdown_tx_clone.send(());
        })
        .await
        .expect("Server error");

    // ── Graceful teardown ─────────────────────────────────────────────────
    info!("Detaching all panes...");
    let panes: Vec<String> = registry.list().into_iter().map(|p| p.pane_id).collect();
    for pane_id in panes {
        manager.detach(&pane_id).await;
    }

    // Brief pause for cleanup tasks to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    info!("Shutdown complete.");
}

fn parse_cli_args() -> Result<(String, ProjectSource), String> {
    let mut config_path = String::from("config.toml");
    let mut source = ProjectSource::Python;
    let mut has_config_path = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--project=") {
            source = ProjectSource::parse(value).ok_or_else(|| {
                format!("invalid --project value '{value}', expected python|rust")
            })?;
            continue;
        }

        if arg == "--project" {
            let value = args.next().ok_or_else(|| {
                "missing value after --project (expected python|rust)".to_string()
            })?;
            source = ProjectSource::parse(&value).ok_or_else(|| {
                format!("invalid --project value '{value}', expected python|rust")
            })?;
            continue;
        }

        if arg.starts_with('-') {
            return Err(format!("unknown flag '{arg}'"));
        }

        if has_config_path {
            return Err(format!("unexpected positional argument '{arg}'"));
        }

        config_path = arg;
        has_config_path = true;
    }

    Ok((config_path, source))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c   => {},
        _ = terminate => {},
    }
}
