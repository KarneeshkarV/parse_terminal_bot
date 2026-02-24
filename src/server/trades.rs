use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;
use std::collections::BTreeMap;
use tokio::fs;

use super::api::AppState;

#[derive(Serialize, Default, Clone)]
pub struct DailyPnl {
    pub date:   String,
    pub pnl:    f64,
    pub wins:   u64,
    pub losses: u64,
    pub bets:   u64,
}

#[derive(Serialize, Default, Clone)]
pub struct TradeStats {
    pub total_windows: u64,
    pub bets_placed:   u64,
    pub wins:          u64,
    pub losses:        u64,
    pub skips:         u64,
    pub invalid:       u64,
    pub win_rate:      f64,
    pub total_pnl:     f64,
    pub current_balance:  f64,
    pub initial_balance:  f64,
    pub sessions:      u64,
    pub algo:          String,
    pub last_updated:  String,
}

#[derive(Serialize, Clone)]
pub struct TradeRecord {
    pub timestamp:     String,
    pub market_slug:   String,
    pub window_start:  String,
    pub window_end:    String,
    pub prediction:    String,
    pub side:          String,
    pub exit_reason:   String,
    pub pnl:           f64,
    pub balance_after: f64,
    pub stake_usd:     f64,
    pub btc_open:      f64,
    pub btc_diff:      f64,
    pub btc_gap_pct:   f64,
    pub up_midpoint:   f64,
    pub down_midpoint: f64,
    pub session:       String,
}

#[derive(Serialize)]
pub struct TradesResponse {
    pub stats:  TradeStats,
    pub trades: Vec<TradeRecord>,
}

pub async fn get_trades(State(state): State<AppState>) -> impl IntoResponse {
    let dir = &state.trades_data_dir;

    // Collect all live_*.csv files
    let mut csv_files: Vec<String> = Vec::new();
    if let Ok(mut entries) = fs::read_dir(dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("live_") && name.ends_with(".csv") {
                csv_files.push(entry.path().to_string_lossy().to_string());
            }
        }
    }
    csv_files.sort();

    let mut stats  = TradeStats::default();
    let mut trades: Vec<TradeRecord> = Vec::new();
    let mut daily: BTreeMap<String, DailyPnl> = BTreeMap::new();
    let mut initial_balance_set = false;

    stats.sessions = csv_files.len() as u64;

    for path in &csv_files {
        // Derive session label from filename (e.g. "live_Adv..._20260218_133426.csv" → "20260218_133426")
        let session_label = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplitn(3, '_').nth(1)
                .and_then(|_| {
                    // grab last two underscore-separated tokens
                    let parts: Vec<&str> = s.split('_').collect();
                    if parts.len() >= 2 {
                        Some(format!("{}_{}", parts[parts.len()-2], parts[parts.len()-1]))
                    } else { None }
                }))
            .unwrap_or_default();

        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut lines = content.lines();
        let _ = lines.next(); // skip header

        for line in lines {
            let cols: Vec<&str> = line.splitn(28, ',').collect();
            if cols.len() < 27 { continue; }

            let timestamp    = cols[0].to_string();
            let market_slug  = cols[1].to_string();
            let window_start = cols[2].to_string();
            let window_end   = cols[3].to_string();
            let prediction   = cols[5].to_string();
            let btc_open: f64    = cols[6].parse().unwrap_or(0.0);
            let btc_diff: f64    = cols[10].parse().unwrap_or(0.0);
            let btc_gap_pct: f64 = cols[12].parse().unwrap_or(0.0);
            let up_midpoint: f64   = cols[15].parse().unwrap_or(0.0);
            let down_midpoint: f64 = cols[16].parse().unwrap_or(0.0);
            let side         = cols[19].to_string();
            let stake_usd: f64   = cols[21].parse().unwrap_or(0.0);
            let pnl: f64         = cols[22].parse().unwrap_or(0.0);
            let exit_reason  = cols[23].to_string();
            let balance_before: f64 = cols[24].parse().unwrap_or(0.0);
            let balance_after: f64  = cols[25].parse().unwrap_or(0.0);

            if stats.algo.is_empty() && !cols[4].is_empty() {
                stats.algo = cols[4].to_string();
            }

            if !initial_balance_set && balance_before > 0.0 {
                stats.initial_balance = balance_before;
                initial_balance_set = true;
            }

            stats.total_windows += 1;
            if balance_after > 0.0 {
                stats.current_balance = balance_after;
                stats.last_updated = timestamp.clone();
            }

            match exit_reason.as_str() {
                "skip" => stats.skips += 1,
                "invalid_price" | "too_small" | "buy_failed" => stats.invalid += 1,
                "resolution" | "stop_loss" => {
                    stats.bets_placed += 1;
                    if pnl > 0.0 {
                        stats.wins += 1;
                    } else {
                        stats.losses += 1;
                    }
                    stats.total_pnl += pnl;

                    // Per-day accumulation (date = first 10 chars of timestamp)
                    let date = timestamp.chars().take(10).collect::<String>();
                    let day = daily.entry(date.clone()).or_insert_with(|| DailyPnl {
                        date,
                        ..Default::default()
                    });
                    day.bets += 1;
                    day.pnl  += pnl;
                    if pnl > 0.0 { day.wins += 1; } else { day.losses += 1; }
                }
                _ => {}
            }

            trades.push(TradeRecord {
                timestamp,
                market_slug,
                window_start,
                window_end,
                prediction,
                side,
                exit_reason,
                pnl,
                balance_after,
                stake_usd,
                btc_open,
                btc_diff,
                btc_gap_pct,
                up_midpoint,
                down_midpoint,
                session: session_label.clone(),
            });
        }
    }

    if stats.bets_placed > 0 {
        stats.win_rate = (stats.wins as f64 / stats.bets_placed as f64) * 100.0;
    }

    // Return most-recent 500 trades (frontend can paginate further)
    let total = trades.len();
    if total > 500 {
        trades = trades.into_iter().rev().take(500).rev().collect();
    }

    let daily_pnl: Vec<DailyPnl> = daily.into_values().collect();

    Json(serde_json::json!({
        "ok": true,
        "stats": stats,
        "trades": trades,
        "total_records": total,
        "daily_pnl": daily_pnl,
    }))
}
