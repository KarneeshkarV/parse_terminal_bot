use serde::{Deserialize, Serialize};

pub type PaneId   = String;
pub type StreamId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnsiStyle {
    pub fg:        Option<String>,
    pub bg:        Option<String>,
    pub bold:      bool,
    pub italic:    bool,
    pub underline: bool,
    pub dim:       bool,
    pub blink:     bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SemanticContent {
    Raw,
    LogLine {
        level:   String,
        module:  String,
        message: String,
    },
    BtcAnalytics {
        market:       String,
        window_start: String,
        window_end:   String,
        btc_open:     f64,
        btc_close:    f64,
        btc_high:     f64,
        btc_low:      f64,
        btc_diff:     f64,
        gap_pct:      f64,
        up_midpoint:  f64,
        down_midpoint: f64,
        prediction:   String,
    },
    TradeAction {
        action:      String,
        entry_price: Option<f64>,
        shares:      Option<f64>,
        stake_usd:   Option<f64>,
        balance:     Option<f64>,
    },
    TradeExit {
        exit_price:  f64,
        pnl:         f64,
        balance:     f64,
        exit_reason: String,
    },
    SessionSummary {
        duration_hours: f64,
        total_trades:   u32,
        wins:           u32,
        losses:         u32,
        win_rate:       f64,
        total_pnl:      f64,
        final_balance:  f64,
    },
}

impl Default for SemanticContent {
    fn default() -> Self { SemanticContent::Raw }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneEvent {
    pub stream_id:  StreamId,
    pub pane_id:    PaneId,
    pub timestamp:  i64,
    #[serde(flatten)]
    pub event_type: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    Line {
        raw:         String,
        clean:       String,
        style:       AnsiStyle,
        semantic:    SemanticContent,
        line_number: u64,
    },
    PaneRegistered {
        label: Option<String>,
    },
    PaneUnregistered {
        reason: String,
    },
    Snapshot {
        lines:       Vec<PaneEvent>,
        total_lines: u64,
    },
    Ping,
    Error {
        message: String,
    },
}

/// Incoming WS client commands
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ClientCommand {
    Subscribe   { pane_id: PaneId },
    Unsubscribe { pane_id: PaneId },
    Replay      { pane_id: PaneId, lines: Option<usize> },
}
