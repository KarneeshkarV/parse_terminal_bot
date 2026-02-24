/// BTC-domain semantic state-machine parser.
///
/// Matches output from:
///   /home/karneeshkar/Desktop/personal/Polymarket_base/polymarket_api/btc_backtest/live_trader.py
///
/// Key print patterns:
///   Analytics block: starts with "  ─────" line, contains Market/Window/BTC fields, ends at Prediction:
///   Trade action:    "  Action:           BUY_UP / BUY_DOWN / SKIP (no edge)"
///   Trade exit:      "  Exit price: / P&L: / Balance after: / ─────"
///   Session summary: "SESSION SUMMARY" header
///   Log lines:       "YYYY-MM-DD HH:MM:SS | LEVEL | module | message"
use once_cell::sync::Lazy;
use regex::Regex;

use crate::config::ProjectSource;
use crate::types::SemanticContent;

// ─── Compiled regexes ──────────────────────────────────────────────────────

static LOG_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    // Matches both formats:
    //   "2026-02-18 23:04:00,725 [INFO] module.name: message"  (Python default)
    //   "2026-02-18 23:04:00 | INFO | module | message"        (custom format)
    Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}[,.]?\d*\s+(?:\[(INFO|WARNING|ERROR|DEBUG|CRITICAL)\]\s+([\w.]+):\s+(.+)|\| ?(INFO|WARNING|ERROR|DEBUG|CRITICAL)\s*\| ?([\w.]+)\s*\| ?(.+))$").unwrap()
});

static SEPARATOR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*[─\-]{10,}").unwrap());

static MARKET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Market:\s+(.+)").unwrap());

static WINDOW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Window:\s+(\d{2}:\d{2}) - (\d{2}:\d{2}) UTC").unwrap());

static BTC_OPEN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"BTC Open[^:]*:\s+\$([0-9,]+\.?\d*)").unwrap());

static BTC_CLOSE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"BTC Close[^:]*:\s+\$([0-9,]+\.?\d*)").unwrap());

static BTC_HIGH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"BTC High[^:]*:\s+\$([0-9,]+\.?\d*)").unwrap());

static BTC_LOW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"BTC Low[^:]*:\s+\$([0-9,]+\.?\d*)").unwrap());

static BTC_DIFF_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"BTC Diff[^:]*:\s+\$([+-]?[0-9,]+\.?\d*)").unwrap());

static GAP_PCT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\(([+-]?[0-9.]+)%\)").unwrap());

static UP_MID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Up midpoint:\s+([0-9.]+)").unwrap());

static DOWN_MID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Down midpoint:\s+([0-9.]+)").unwrap());

static PREDICTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Prediction:\s+(UP|DOWN|SKIP)").unwrap());

static ACTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Action:\s+(BUY_UP|BUY_DOWN|SKIP[^\n]*)").unwrap());

static ENTRY_PRICE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Entry price:\s+\$([0-9.]+)").unwrap());

static SHARES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Shares:\s+([0-9.]+)").unwrap());

static STAKE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Stake:\s+\$([0-9.]+)").unwrap());

static BALANCE_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    // Matches "Balance:  $123.45" (SKIP case) or "Balance before:  $123.45"
    Regex::new(r"Balance[^:]*:\s+\$([0-9.]+)").unwrap()
});

static EXIT_PRICE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Exit price:\s+\$([0-9.]+)").unwrap());

static EXIT_REASON_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Exit reason:\s+(\S+)").unwrap());

static PNL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"P&L:\s+\$([+-]?[0-9.]+)").unwrap());

static BALANCE_AFTER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Balance after:\s+\$([0-9.]+)").unwrap());

static SESSION_SUMMARY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"SESSION SUMMARY").unwrap());

static RUST_SESSION_HEADER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"SESSION SUMMARY\s*\|\s*mode=([^|]+)\|\s*algo=(.+)$").unwrap());

static RUST_MARKET_END_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"MARKET END\s*\|\s*mode=([^|]+)\|\s*market=(.+)$").unwrap());

static WINDOWS_SEEN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Windows seen:\s+(\d+)").unwrap());

static SKIPPED_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Skipped:\s+(\d+)").unwrap());

static WINDOW_PNL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Window P&L:\s+\$([+-]?[0-9.]+)").unwrap());

static SESSION_PNL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Session P&L:\s+\$([+-]?[0-9.]+)").unwrap());

static CSV_LOG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"CSV log:\s+(.+)").unwrap());

static WINS_LOSSES_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Wins/Losses:\s+(\d+)\s*/\s*(\d+)\s*\(skipped:\s*(\d+)\)").unwrap());

static DURATION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Duration:\s+([0-9.]+)\s+hours?").unwrap());

static TRADES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Trades executed:\s+(\d+)").unwrap());

static WINS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Wins:\s+(\d+)").unwrap());

static LOSSES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Losses:\s+(\d+)").unwrap());

static WIN_RATE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"Win rate:\s+([0-9.]+)%").unwrap());

static TOTAL_PNL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Total P&L:\s+\$([+-]?[0-9.]+)").unwrap());

static FINAL_BAL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Final balance:\s+\$([0-9.]+)").unwrap());

static SESSION_END_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[=]{10,}").unwrap());

// ─── State machine ─────────────────────────────────────────────────────────

#[derive(Default)]
enum State {
    #[default]
    Scanning,
    InAnalyticsBlock(AnalyticsAccum),
    InTradeAction(TradeActionAccum),
    InTradeExit(TradeExitAccum),
    InSessionSummary(SessionAccum),
    InRustMarketEnd(RustMarketEndAccum),
}

#[derive(Default)]
struct AnalyticsAccum {
    market: String,
    window_start: String,
    window_end: String,
    btc_open: f64,
    btc_close: f64,
    btc_high: f64,
    btc_low: f64,
    btc_diff: f64,
    gap_pct: f64,
    up_midpoint: f64,
    down_midpoint: f64,
    sep_count: u32,
}

#[derive(Default)]
struct TradeActionAccum {
    action: String,
    entry_price: Option<f64>,
    shares: Option<f64>,
    stake_usd: Option<f64>,
    balance: Option<f64>,
    sep_count: u32,
}

#[derive(Default)]
struct TradeExitAccum {
    exit_price: f64,
    pnl: f64,
    balance: f64,
    exit_reason: String,
    sep_count: u32,
}

#[derive(Default)]
struct SessionAccum {
    mode: String,
    algo: String,
    duration_hours: f64,
    windows_seen: u32,
    total_trades: u32,
    skipped: u32,
    wins: u32,
    losses: u32,
    win_rate: f64,
    total_pnl: f64,
    final_balance: Option<f64>,
    csv_log: String,
    end_sep_seen: bool,
}

#[derive(Default)]
struct RustMarketEndAccum {
    mode: String,
    market: String,
    prediction: String,
    exit_reason: String,
    window_pnl: f64,
    session_pnl: f64,
    wins: u32,
    losses: u32,
    skipped: u32,
    sep_count: u32,
}

/// Per-pane stateful semantic parser.
pub struct SemanticParser {
    source: ProjectSource,
    state: State,
}

impl SemanticParser {
    pub fn new(source: ProjectSource) -> Self {
        Self {
            source,
            state: State::Scanning,
        }
    }

    /// Feed a clean (ANSI-stripped) line. Returns Some(SemanticContent) when
    /// a complete structured block has been parsed.
    pub fn feed(&mut self, line: &str) -> SemanticContent {
        let trimmed = line.trim();

        match &mut self.state {
            State::Scanning => self.scan_line(trimmed),

            State::InAnalyticsBlock(_) => {
                let emit = self.accumulate_analytics(trimmed);
                emit.unwrap_or(SemanticContent::Raw)
            }

            State::InTradeAction(_) => {
                let emit = self.accumulate_trade_action(trimmed);
                emit.unwrap_or(SemanticContent::Raw)
            }

            State::InTradeExit(_) => {
                let emit = self.accumulate_trade_exit(trimmed);
                emit.unwrap_or(SemanticContent::Raw)
            }

            State::InSessionSummary(_) => {
                let emit = self.accumulate_session(trimmed);
                emit.unwrap_or(SemanticContent::Raw)
            }

            State::InRustMarketEnd(_) => {
                let emit = self.accumulate_rust_market_end(trimmed);
                emit.unwrap_or(SemanticContent::Raw)
            }
        }
    }

    fn scan_line(&mut self, line: &str) -> SemanticContent {
        // Log line — groups 1-3 = [LEVEL] module: msg, groups 4-6 = | LEVEL | module | msg
        if let Some(caps) = LOG_LINE_RE.captures(line) {
            let (level, module, message) = if caps.get(1).is_some() {
                (
                    caps[1].to_string(),
                    caps[2].to_string(),
                    caps[3].to_string(),
                )
            } else {
                (
                    caps[4].to_string(),
                    caps[5].to_string(),
                    caps[6].to_string(),
                )
            };
            return SemanticContent::LogLine {
                level,
                module,
                message,
            };
        }

        match self.source {
            ProjectSource::Python => {
                if SEPARATOR_RE.is_match(line) {
                    self.state = State::InAnalyticsBlock(AnalyticsAccum {
                        sep_count: 1,
                        ..Default::default()
                    });
                    return SemanticContent::Raw;
                }

                if SESSION_SUMMARY_RE.is_match(line) {
                    self.state = State::InSessionSummary(SessionAccum::default());
                    return SemanticContent::Raw;
                }
            }
            ProjectSource::Rust => {
                if let Some(caps) = RUST_MARKET_END_RE.captures(line) {
                    self.state = State::InRustMarketEnd(RustMarketEndAccum {
                        mode: caps[1].trim().to_string(),
                        market: caps[2].trim().to_string(),
                        ..Default::default()
                    });
                    return SemanticContent::Raw;
                }

                if let Some(caps) = RUST_SESSION_HEADER_RE.captures(line) {
                    self.state = State::InSessionSummary(SessionAccum {
                        mode: caps[1].trim().to_string(),
                        algo: caps[2].trim().to_string(),
                        ..Default::default()
                    });
                    return SemanticContent::Raw;
                }
            }
        }

        SemanticContent::Raw
    }

    fn accumulate_analytics(&mut self, line: &str) -> Option<SemanticContent> {
        let State::InAnalyticsBlock(ref mut a) = self.state else {
            return None;
        };

        if SEPARATOR_RE.is_match(line) {
            a.sep_count += 1;
            return None;
        }

        if let Some(caps) = MARKET_RE.captures(line) {
            a.market = caps[1].trim().to_string();
        }
        if let Some(caps) = WINDOW_RE.captures(line) {
            a.window_start = caps[1].to_string();
            a.window_end = caps[2].to_string();
        }
        if let Some(caps) = BTC_OPEN_RE.captures(line) {
            a.btc_open = parse_num(&caps[1]);
        }
        if let Some(caps) = BTC_CLOSE_RE.captures(line) {
            a.btc_close = parse_num(&caps[1]);
        }
        if let Some(caps) = BTC_HIGH_RE.captures(line) {
            a.btc_high = parse_num(&caps[1]);
        }
        if let Some(caps) = BTC_LOW_RE.captures(line) {
            a.btc_low = parse_num(&caps[1]);
        }
        if let Some(caps) = BTC_DIFF_RE.captures(line) {
            a.btc_diff = parse_num(&caps[1]);
        }
        if let Some(caps) = GAP_PCT_RE.captures(line) {
            a.gap_pct = parse_num(&caps[1]);
        }
        if let Some(caps) = UP_MID_RE.captures(line) {
            a.up_midpoint = parse_num(&caps[1]);
        }
        if let Some(caps) = DOWN_MID_RE.captures(line) {
            a.down_midpoint = parse_num(&caps[1]);
        }

        if let Some(caps) = PREDICTION_RE.captures(line) {
            let prediction = caps[1].to_string();
            let State::InAnalyticsBlock(a) = std::mem::replace(&mut self.state, State::Scanning)
            else {
                return None;
            };

            // After prediction, check if next block is a trade action
            // (The action line might come in the NEXT call, so just emit analytics here)
            return Some(SemanticContent::BtcAnalytics {
                market: a.market,
                window_start: a.window_start,
                window_end: a.window_end,
                btc_open: a.btc_open,
                btc_close: a.btc_close,
                btc_high: a.btc_high,
                btc_low: a.btc_low,
                btc_diff: a.btc_diff,
                gap_pct: a.gap_pct,
                up_midpoint: a.up_midpoint,
                down_midpoint: a.down_midpoint,
                prediction,
            });
        }

        // Detect Action line (trade action starts right after analytics sometimes)
        if let Some(caps) = ACTION_RE.captures(line) {
            let action = caps[1].trim().to_string();
            self.state = State::InTradeAction(TradeActionAccum {
                action,
                ..Default::default()
            });
        }

        None
    }

    fn accumulate_trade_action(&mut self, line: &str) -> Option<SemanticContent> {
        let State::InTradeAction(ref mut a) = self.state else {
            return None;
        };

        if SEPARATOR_RE.is_match(line) {
            a.sep_count += 1;
            // First separator in action block: emit action, then enter exit block
            if a.sep_count >= 1 && !a.action.is_empty() {
                let is_skip = a.action.starts_with("SKIP");
                let State::InTradeAction(a) = std::mem::replace(
                    &mut self.state,
                    // Non-skip trades are followed by an exit block
                    if is_skip {
                        State::Scanning
                    } else {
                        State::InTradeExit(TradeExitAccum::default())
                    },
                ) else {
                    return None;
                };
                return Some(SemanticContent::TradeAction {
                    action: a.action,
                    entry_price: a.entry_price,
                    shares: a.shares,
                    stake_usd: a.stake_usd,
                    balance: a.balance,
                });
            }
            return None;
        }

        // Pick up action line if we entered from scanning
        if a.action.is_empty() {
            if let Some(caps) = ACTION_RE.captures(line) {
                a.action = caps[1].trim().to_string();
                return None;
            }
        }

        if let Some(caps) = ENTRY_PRICE_RE.captures(line) {
            a.entry_price = Some(parse_num(&caps[1]));
        }
        if let Some(caps) = SHARES_RE.captures(line) {
            a.shares = Some(parse_num(&caps[1]));
        }
        if let Some(caps) = STAKE_RE.captures(line) {
            a.stake_usd = Some(parse_num(&caps[1]));
        }
        if let Some(caps) = BALANCE_LINE_RE.captures(line) {
            a.balance = Some(parse_num(&caps[1]));
        }

        // SKIP exits immediately when we see Balance line
        if a.action.starts_with("SKIP") && a.balance.is_some() {
            let State::InTradeAction(a) = std::mem::replace(&mut self.state, State::Scanning)
            else {
                return None;
            };
            return Some(SemanticContent::TradeAction {
                action: a.action,
                entry_price: None,
                shares: None,
                stake_usd: None,
                balance: a.balance,
            });
        }

        None
    }

    fn accumulate_trade_exit(&mut self, line: &str) -> Option<SemanticContent> {
        let State::InTradeExit(ref mut a) = self.state else {
            return None;
        };

        if SEPARATOR_RE.is_match(line) {
            a.sep_count += 1;
            if a.sep_count >= 2 && a.exit_price > 0.0 {
                let State::InTradeExit(a) = std::mem::replace(&mut self.state, State::Scanning)
                else {
                    return None;
                };
                return Some(SemanticContent::TradeExit {
                    exit_price: a.exit_price,
                    pnl: a.pnl,
                    balance: a.balance,
                    exit_reason: a.exit_reason,
                });
            }
            return None;
        }

        if let Some(caps) = EXIT_PRICE_RE.captures(line) {
            a.exit_price = parse_num(&caps[1]);
        }
        if let Some(caps) = EXIT_REASON_RE.captures(line) {
            a.exit_reason = caps[1].to_string();
        }
        if let Some(caps) = PNL_RE.captures(line) {
            a.pnl = parse_num(&caps[1]);
        }
        if let Some(caps) = BALANCE_AFTER_RE.captures(line) {
            a.balance = parse_num(&caps[1]);
        }

        None
    }

    fn accumulate_session(&mut self, line: &str) -> Option<SemanticContent> {
        let State::InSessionSummary(ref mut a) = self.state else {
            return None;
        };

        // Session block ends on second "====" separator
        if SESSION_END_RE.is_match(line.trim_start()) {
            if a.end_sep_seen {
                let State::InSessionSummary(a) =
                    std::mem::replace(&mut self.state, State::Scanning)
                else {
                    return None;
                };
                return Some(match self.source {
                    ProjectSource::Python => SemanticContent::SessionSummary {
                        duration_hours: a.duration_hours,
                        total_trades: a.total_trades,
                        wins: a.wins,
                        losses: a.losses,
                        win_rate: a.win_rate,
                        total_pnl: a.total_pnl,
                        final_balance: a.final_balance.unwrap_or(0.0),
                    },
                    ProjectSource::Rust => SemanticContent::RustSessionSummary {
                        mode: a.mode,
                        algo: a.algo,
                        duration_hours: a.duration_hours,
                        windows_seen: a.windows_seen,
                        total_trades: a.total_trades,
                        skipped: a.skipped,
                        wins: a.wins,
                        losses: a.losses,
                        win_rate: a.win_rate,
                        total_pnl: a.total_pnl,
                        final_balance: a.final_balance,
                        csv_log: a.csv_log,
                    },
                });
            } else {
                a.end_sep_seen = true;
                return None;
            }
        }

        if let Some(caps) = DURATION_RE.captures(line) {
            a.duration_hours = parse_num(&caps[1]);
        }
        if let Some(caps) = WINDOWS_SEEN_RE.captures(line) {
            a.windows_seen = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = TRADES_RE.captures(line) {
            a.total_trades = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = SKIPPED_RE.captures(line) {
            a.skipped = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = WINS_RE.captures(line) {
            a.wins = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = LOSSES_RE.captures(line) {
            a.losses = caps[1].parse().unwrap_or(0);
        }
        if let Some(caps) = WIN_RATE_RE.captures(line) {
            a.win_rate = parse_num(&caps[1]);
        }
        if let Some(caps) = TOTAL_PNL_RE.captures(line) {
            a.total_pnl = parse_num(&caps[1]);
        }
        if let Some(caps) = RUST_SESSION_HEADER_RE.captures(line) {
            a.mode = caps[1].trim().to_string();
            a.algo = caps[2].trim().to_string();
        }
        if let Some(caps) = FINAL_BAL_RE.captures(line) {
            a.final_balance = Some(parse_num(&caps[1]));
        } else if line.to_ascii_lowercase().contains("final balance:")
            && line.to_ascii_lowercase().contains("n/a")
        {
            a.final_balance = None;
        }
        if let Some(caps) = CSV_LOG_RE.captures(line) {
            a.csv_log = caps[1].trim().to_string();
        }

        None
    }

    fn accumulate_rust_market_end(&mut self, line: &str) -> Option<SemanticContent> {
        let State::InRustMarketEnd(ref mut a) = self.state else {
            return None;
        };

        if SEPARATOR_RE.is_match(line) {
            a.sep_count += 1;
            if a.sep_count >= 1 && !a.market.is_empty() {
                let State::InRustMarketEnd(a) = std::mem::replace(&mut self.state, State::Scanning)
                else {
                    return None;
                };
                return Some(SemanticContent::RustMarketEnd {
                    mode: a.mode,
                    market: a.market,
                    prediction: a.prediction,
                    exit_reason: a.exit_reason,
                    window_pnl: a.window_pnl,
                    session_pnl: a.session_pnl,
                    wins: a.wins,
                    losses: a.losses,
                    skipped: a.skipped,
                });
            }
            return None;
        }

        if let Some(caps) = PREDICTION_RE.captures(line) {
            a.prediction = caps[1].to_string();
        }
        if let Some(caps) = EXIT_REASON_RE.captures(line) {
            a.exit_reason = caps[1].to_string();
        }
        if let Some(caps) = WINDOW_PNL_RE.captures(line) {
            a.window_pnl = parse_num(&caps[1]);
        }
        if let Some(caps) = SESSION_PNL_RE.captures(line) {
            a.session_pnl = parse_num(&caps[1]);
        }
        if let Some(caps) = WINS_LOSSES_RE.captures(line) {
            a.wins = caps[1].parse().unwrap_or(0);
            a.losses = caps[2].parse().unwrap_or(0);
            a.skipped = caps[3].parse().unwrap_or(0);
        }

        None
    }
}

fn parse_num(s: &str) -> f64 {
    s.replace(',', "").parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::SemanticParser;
    use crate::config::ProjectSource;
    use crate::types::SemanticContent;

    #[test]
    fn parses_python_session_summary() {
        let mut parser = SemanticParser::new(ProjectSource::Python);

        for line in [
            "SESSION SUMMARY",
            "==========",
            "Duration: 1.5 hours",
            "Trades executed: 2",
            "Wins: 1",
            "Losses: 1",
            "Win rate: 50.0%",
            "Total P&L: $+12.34",
            "Final balance: $100.12",
        ] {
            let _ = parser.feed(line);
        }

        let event = parser.feed("==========");
        match event {
            SemanticContent::SessionSummary {
                total_trades,
                wins,
                losses,
                total_pnl,
                final_balance,
                ..
            } => {
                assert_eq!(total_trades, 2);
                assert_eq!(wins, 1);
                assert_eq!(losses, 1);
                assert!((total_pnl - 12.34).abs() < 0.001);
                assert!((final_balance - 100.12).abs() < 0.001);
            }
            _ => panic!("expected python session summary event"),
        }
    }

    #[test]
    fn parses_rust_market_end_and_session_summary() {
        let mut parser = SemanticParser::new(ProjectSource::Rust);

        let market_lines = [
            "MARKET END | mode=trading_live | market=btc-updown-15m-test",
            "  Prediction:      UP",
            "  Exit reason:     resolution",
            "  Window P&L:      $+1.20",
            "  Session P&L:     $-0.80",
            "  Wins/Losses:     3/2  (skipped: 1)",
        ];
        for line in market_lines {
            let _ = parser.feed(line);
        }

        let market_event = parser.feed("----------");
        match market_event {
            SemanticContent::RustMarketEnd {
                mode,
                prediction,
                exit_reason,
                wins,
                losses,
                skipped,
                ..
            } => {
                assert_eq!(mode, "trading_live");
                assert_eq!(prediction, "UP");
                assert_eq!(exit_reason, "resolution");
                assert_eq!(wins, 3);
                assert_eq!(losses, 2);
                assert_eq!(skipped, 1);
            }
            _ => panic!("expected rust market end event"),
        }

        let session_lines = [
            "SESSION SUMMARY | mode=paper | algo=advanced_momentum",
            "==========",
            "Duration: 2.0 hours",
            "Windows seen: 8",
            "Trades executed: 5",
            "Skipped: 3",
            "Wins: 3",
            "Losses: 2",
            "Win rate: 60.0%",
            "Total P&L: $+4.56",
            "Final balance: $123.45",
            "CSV log: data/btc_live_trades/paper_x.csv",
        ];
        for line in session_lines {
            let _ = parser.feed(line);
        }

        let session_event = parser.feed("==========");
        match session_event {
            SemanticContent::RustSessionSummary {
                mode,
                algo,
                windows_seen,
                total_trades,
                skipped,
                ..
            } => {
                assert_eq!(mode, "paper");
                assert_eq!(algo, "advanced_momentum");
                assert_eq!(windows_seen, 8);
                assert_eq!(total_trades, 5);
                assert_eq!(skipped, 3);
            }
            _ => panic!("expected rust session summary event"),
        }
    }
}
