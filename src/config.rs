use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSource {
    Python,
    Rust,
}

impl ProjectSource {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "python" => Some(Self::Python),
            "rust" => Some(Self::Rust),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Rust => "rust",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub tmux: TmuxConfig,
    pub buffer: BufferConfig,
    pub trades: TradesConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradesConfig {
    pub data_dir: Option<String>,
    pub python_data_dir: Option<String>,
    pub rust_data_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmuxConfig {
    pub pipe_dir: String,
    pub discovery_interval_ms: u64,
    pub sessions_to_watch: Vec<String>,
    pub initial_panes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BufferConfig {
    pub replay_lines: usize,
    pub channel_capacity: usize,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read config {path}: {e}"))?;
        let cfg: Config =
            toml::from_str(&text).map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?;
        Ok(cfg)
    }

    pub fn pipe_dir(&self) -> PathBuf {
        PathBuf::from(&self.tmux.pipe_dir)
    }

    pub fn trades_data_dir_for(&self, source: ProjectSource) -> Option<String> {
        let scoped = match source {
            ProjectSource::Python => self.trades.python_data_dir.as_deref(),
            ProjectSource::Rust => self.trades.rust_data_dir.as_deref(),
        }
        .map(str::trim)
        .filter(|s| !s.is_empty());

        scoped
            .or_else(|| {
                self.trades
                    .data_dir
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .map(ToOwned::to_owned)
    }
}
