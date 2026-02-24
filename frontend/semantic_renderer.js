/**
 * semantic_renderer.js — Converts SemanticContent → HTML strings
 */

export function renderEvent(event) {
  if (event.type !== 'line') return null;
  const { clean, style, semantic } = event;
  return renderSemantic(semantic, clean, style);
}

function renderSemantic(sem, raw, style) {
  if (!sem) return renderRaw(raw, style);
  switch (sem.kind) {
    case 'log_line':        return renderLogLine(sem);
    case 'btc_analytics':   return renderBtcAnalytics(sem);
    case 'trade_action':    return renderTradeAction(sem);
    case 'trade_exit':      return renderTradeExit(sem);
    case 'session_summary': return renderSessionSummary(sem);
    case 'rust_market_end': return renderRustMarketEnd(sem);
    case 'rust_session_summary': return renderRustSessionSummary(sem);
    default:                return renderRaw(raw, style);
  }
}

// ── Raw line ──────────────────────────────────────────────────────────────

function renderRaw(text, style) {
  const color = style && style.fg ? `color:var(--ansi-${style.fg})` : '';
  const bold  = style && style.bold ? 'font-weight:600' : '';
  const css   = [color, bold].filter(Boolean).join(';');
  return `<div class="line-raw" ${css ? `style="${css}"` : ''}>${esc(text)}</div>`;
}

// ── Log line ──────────────────────────────────────────────────────────────

function renderLogLine(s) {
  const lvl   = s.level.toLowerCase();
  const badge = `<span class="badge badge-${lvl}">${s.level}</span>`;
  const mod   = `<span class="log-module">${esc(s.module)}</span>`;
  const msg   = `<span class="log-message">${esc(s.message)}</span>`;
  return `<div class="line-log">${badge}${mod}${msg}</div>`;
}

// ── BTC Analytics card ────────────────────────────────────────────────────

function renderBtcAnalytics(s) {
  const pDir    = s.btc_diff >= 0 ? 'pos' : 'neg';
  const pSign   = s.btc_diff >= 0 ? '+' : '';
  const predCls = s.prediction === 'UP'   ? 'pred-up'
                : s.prediction === 'DOWN' ? 'pred-down' : 'pred-skip';
  const predIcon = s.prediction === 'UP'   ? '▲'
                 : s.prediction === 'DOWN' ? '▼' : '⊘';

  return `
<div class="card card-analytics">
  <div class="card-header">
    <span class="card-title">📊 ${esc(s.market)}</span>
    <span class="card-subtitle">${esc(s.window_start)} → ${esc(s.window_end)} UTC</span>
    <span class="pred-badge ${predCls}">${predIcon} ${s.prediction}</span>
  </div>
  <div class="analytics-grid">
    <div class="ag-cell"><div class="ag-label">Open</div><div class="ag-value">${fmt(s.btc_open)}</div></div>
    <div class="ag-cell"><div class="ag-label">Close (m13)</div><div class="ag-value">${fmt(s.btc_close)}</div></div>
    <div class="ag-cell"><div class="ag-label">High</div><div class="ag-value">${fmt(s.btc_high)}</div></div>
    <div class="ag-cell"><div class="ag-label">Low</div><div class="ag-value">${fmt(s.btc_low)}</div></div>
    <div class="ag-cell"><div class="ag-label">Diff</div><div class="ag-value ${pDir}">${pSign}${s.btc_diff.toFixed(2)} (${pSign}${(s.gap_pct).toFixed(4)}%)</div></div>
    <div class="ag-cell"><div class="ag-label">Up mid</div><div class="ag-value">${s.up_midpoint.toFixed(4)}</div></div>
    <div class="ag-cell"><div class="ag-label">Down mid</div><div class="ag-value">${s.down_midpoint.toFixed(4)}</div></div>
    <div class="ag-cell"><div class="ag-label">Spread</div><div class="ag-value">${(s.up_midpoint + s.down_midpoint).toFixed(4)}</div></div>
  </div>
</div>`;
}

// ── Trade action ──────────────────────────────────────────────────────────

function renderTradeAction(s) {
  const isSkip  = s.action.startsWith('SKIP');
  const isBuyUp = s.action === 'BUY_UP';
  const cls     = isSkip ? 'ta-skip' : isBuyUp ? 'ta-up' : 'ta-down';
  const icon    = isSkip ? '⊘' : isBuyUp ? '▲' : '▼';

  let details = '';
  if (!isSkip && s.entry_price != null) {
    details = `
    <div class="ta-details">
      <span>Entry <strong>$${s.entry_price.toFixed(4)}</strong></span>
      <span>Shares <strong>${s.shares != null ? s.shares.toFixed(2) : '—'}</strong></span>
      <span>Stake <strong>$${s.stake_usd != null ? s.stake_usd.toFixed(2) : '—'}</strong></span>
    </div>`;
  }

  // Show skip reason if the parser supplies one
  const skipReason = isSkip && s.reason
    ? `<div class="ta-skip-reason">${esc(s.reason)}</div>`
    : '';

  const bal = s.balance != null
    ? `<span class="ta-balance">Balance $${s.balance.toFixed(2)}</span>`
    : '';

  return `
<div class="card card-trade-action ${cls}">
  <div class="ta-header">
    <span class="ta-icon">${icon}</span>
    <span class="ta-label">${esc(s.action)}</span>
    ${bal}
  </div>
  ${details}${skipReason}
</div>`;
}

// ── Trade exit ────────────────────────────────────────────────────────────

function renderTradeExit(s) {
  const win  = s.pnl >= 0;
  const cls  = win ? 'te-win' : 'te-loss';
  const icon = win ? '✅' : '❌';
  const sign = s.pnl >= 0 ? '+' : '';

  return `
<div class="card card-trade-exit ${cls}">
  <span class="te-icon">${icon}</span>
  <span class="te-pnl">${sign}$${s.pnl.toFixed(4)}</span>
  <span class="te-exit">Exit $${s.exit_price.toFixed(4)}</span>
  <span class="te-reason">${esc(s.exit_reason)}</span>
  <span class="te-balance">Balance $${s.balance.toFixed(2)}</span>
</div>`;
}

// ── Session summary ───────────────────────────────────────────────────────

function renderSessionSummary(s) {
  const sign = s.total_pnl >= 0 ? '+' : '';
  const winCls = s.total_pnl >= 0 ? 'pos' : 'neg';
  return `
<div class="card card-session-summary">
  <div class="ss-title">▪ Session Summary</div>
  <div class="ss-grid">
    <div class="ss-stat"><div class="ss-label">Duration</div><div class="ss-value">${s.duration_hours.toFixed(1)}h</div></div>
    <div class="ss-stat"><div class="ss-label">Trades</div><div class="ss-value">${s.total_trades}</div></div>
    <div class="ss-stat"><div class="ss-label">Wins</div><div class="ss-value pos">${s.wins}</div></div>
    <div class="ss-stat"><div class="ss-label">Losses</div><div class="ss-value neg">${s.losses}</div></div>
    <div class="ss-stat"><div class="ss-label">Win Rate</div><div class="ss-value">${s.win_rate.toFixed(1)}%</div></div>
    <div class="ss-stat"><div class="ss-label">Total P&L</div><div class="ss-value ${winCls}">${sign}$${s.total_pnl.toFixed(2)}</div></div>
    <div class="ss-stat ss-wide"><div class="ss-label">Final Balance</div><div class="ss-value">${s.final_balance.toFixed(2)}</div></div>
  </div>
</div>`;
}

function renderRustMarketEnd(s) {
  const win = s.window_pnl >= 0;
  const winSign = s.window_pnl >= 0 ? '+' : '-';
  const sessionSign = s.session_pnl >= 0 ? '+' : '-';
  const pnlCls = win ? 'pos' : 'neg';
  const predCls = s.prediction === 'UP' ? 'pred-up'
    : s.prediction === 'DOWN' ? 'pred-down'
    : 'pred-skip';
  return `
<div class="card card-trade-exit ${win ? 'te-win' : 'te-loss'}">
  <span class="te-icon">${win ? '✅' : '❌'}</span>
  <span class="te-reason">${esc(s.mode)} · ${esc(s.market)}</span>
  <span class="pred-badge ${predCls}">${esc(s.prediction)}</span>
  <span class="te-pnl ${pnlCls}">${winSign}$${Math.abs(s.window_pnl).toFixed(2)}</span>
  <span class="te-balance">session ${sessionSign}$${Math.abs(s.session_pnl).toFixed(2)}</span>
  <span class="te-exit">${esc(s.exit_reason)} · ${s.wins}W/${s.losses}L (${s.skipped} skipped)</span>
</div>`;
}

function renderRustSessionSummary(s) {
  const sign = s.total_pnl >= 0 ? '+' : '';
  const winCls = s.total_pnl >= 0 ? 'pos' : 'neg';
  const finalBal = s.final_balance != null ? `$${s.final_balance.toFixed(2)}` : 'n/a';
  return `
<div class="card card-session-summary">
  <div class="ss-title">▪ Rust Session Summary</div>
  <div class="ss-grid">
    <div class="ss-stat"><div class="ss-label">Mode</div><div class="ss-value">${esc(s.mode)}</div></div>
    <div class="ss-stat"><div class="ss-label">Algo</div><div class="ss-value">${esc(s.algo)}</div></div>
    <div class="ss-stat"><div class="ss-label">Duration</div><div class="ss-value">${s.duration_hours.toFixed(1)}h</div></div>
    <div class="ss-stat"><div class="ss-label">Windows</div><div class="ss-value">${s.windows_seen}</div></div>
    <div class="ss-stat"><div class="ss-label">Trades</div><div class="ss-value">${s.total_trades}</div></div>
    <div class="ss-stat"><div class="ss-label">Skipped</div><div class="ss-value">${s.skipped}</div></div>
    <div class="ss-stat"><div class="ss-label">Wins</div><div class="ss-value pos">${s.wins}</div></div>
    <div class="ss-stat"><div class="ss-label">Losses</div><div class="ss-value neg">${s.losses}</div></div>
    <div class="ss-stat"><div class="ss-label">Win Rate</div><div class="ss-value">${s.win_rate.toFixed(1)}%</div></div>
    <div class="ss-stat"><div class="ss-label">Total P&L</div><div class="ss-value ${winCls}">${sign}$${Math.abs(s.total_pnl).toFixed(2)}</div></div>
    <div class="ss-stat ss-wide"><div class="ss-label">Final Balance</div><div class="ss-value">${finalBal}</div></div>
  </div>
</div>`;
}

// ── Helpers ───────────────────────────────────────────────────────────────

function esc(str) {
  if (!str) return '';
  return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

function fmt(n) {
  return '$' + Number(n).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
