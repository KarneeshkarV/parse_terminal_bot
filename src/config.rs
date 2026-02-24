use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub tmux:   TmuxConfig,
    pub buffer: BufferConfig,
    pub trades: TradesConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradesConfig {
    pub data_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host:       String,
    pub port:       u16,
    pub static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmuxConfig {
    pub pipe_dir:               String,
    pub discovery_interval_ms:  u64,
    pub sessions_to_watch:      Vec<String>,
    pub initial_panes:          Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BufferConfig {
    pub replay_lines:          usize,
    pub channel_capacity:      usize,
    pub pipe_channel_capacity: usize,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read config {path}: {e}"))?;
        let cfg: Config = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?;
        Ok(cfg)
    }

    pub fn pipe_dir(&self) -> PathBuf {
        PathBuf::from(&self.tmux.pipe_dir)
    }
}
