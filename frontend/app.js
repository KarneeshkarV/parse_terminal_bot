/**
 * app.js — Dashboard + Terminal wiring
 */
import { WsClient }    from './ws_client.js';
import { PaneManager } from './pane_manager.js';

const WS_BASE = `ws://${location.host}/ws`;

// ── DOM refs ──────────────────────────────────────────────────────────────

const tabsEl       = document.getElementById('pane-tabs');
const contentEl    = document.getElementById('pane-content');
const statusEl     = document.getElementById('conn-status');
const searchEl     = document.getElementById('search-input');
const paneSelEl    = document.getElementById('pane-select');
const refreshBtn   = document.getElementById('refresh-btn');
const lastRefreshEl= document.getElementById('last-refresh');
const tradeTbody      = document.getElementById('trade-tbody');
const dailyPnlRowsEl  = document.getElementById('daily-pnl-rows');
const dailyPnlDaysEl  = document.getElementById('daily-pnl-days');

const manager = new PaneManager(tabsEl, contentEl);

// ═══════════════════════════════════════════════════════════════
// MAIN TAB SWITCHING
// ═══════════════════════════════════════════════════════════════

const mainTabs       = document.querySelectorAll('.main-tab');
const dashboardCtrl  = document.getElementById('dashboard-controls');
const terminalCtrl   = document.getElementById('terminal-controls');
const viewDashboard  = document.getElementById('view-dashboard');
const viewTerminal   = document.getElementById('view-terminal');

let activeView = 'dashboard';

function switchView(view) {
  activeView = view;
  mainTabs.forEach(t => t.classList.toggle('active', t.dataset.view === view));

  viewDashboard.classList.toggle('active', view === 'dashboard');
  viewTerminal.classList.toggle('active',  view === 'terminal');

  dashboardCtrl.style.display = view === 'dashboard' ? '' : 'none';
  terminalCtrl.classList.toggle('hidden', view === 'dashboard');

  // When switching to terminal, nudge the VirtualList ResizeObserver by
  // dispatching a resize event so it recalculates heights from a now-visible container
  if (view === 'terminal') {
    requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
  }
}

mainTabs.forEach(tab => {
  tab.addEventListener('click', () => switchView(tab.dataset.view));
});

// ═══════════════════════════════════════════════════════════════
// DASHBOARD — DATA LOADING
// ═══════════════════════════════════════════════════════════════

let refreshTimer = null;

async function loadTrades() {
  refreshBtn.classList.add('spinning');
  try {
    const resp = await fetch('/api/trades');
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const data = await resp.json();
    if (data.ok) {
      renderDashboard(data);
    } else {
      showTradesError('Server returned error');
    }
  } catch (e) {
    console.error('Failed to load trades:', e);
    showTradesError(e.message || 'Fetch failed — is the server running the new binary?');
  } finally {
    refreshBtn.classList.remove('spinning');
    lastRefreshEl.textContent = 'Updated ' + new Date().toLocaleTimeString('en-US', { hour12: false });
  }
}

function showTradesError(msg) {
  tradeTbody.innerHTML = `<tr class="trade-empty"><td colspan="9" style="color:var(--red)">⚠ ${esc(msg)}<br><span style="color:var(--text-dim);font-size:10px">Restart the server: RUST_LOG=info ./target/release/parse_terminal_bot config.toml</span></td></tr>`;
}

function renderDashboard(data) {
  const { stats, trades, total_records } = data;

  // ── Stat cards ──────────────────────────────────────────────
  setText('sv-windows', fmtNum(stats.total_windows));
  setText('ss-windows', `${fmtNum(stats.skips)} skips · ${fmtNum(stats.invalid)} invalid`);

  setText('sv-bets', fmtNum(stats.bets_placed));
  const betPct = stats.total_windows > 0
    ? ((stats.bets_placed / stats.total_windows) * 100).toFixed(1)
    : '0.0';
  setText('ss-bets', `${betPct}% of windows`);

  const wrEl = document.getElementById('sv-winrate');
  const wrCard = document.getElementById('sc-winrate');
  const wr = stats.win_rate.toFixed(1);
  wrEl.textContent = `${wr}%`;
  wrEl.className = 'stat-value ' + (stats.win_rate >= 50 ? 'pos' : 'neg');
  wrCard.classList.remove('card-good', 'card-bad');
  wrCard.classList.add(stats.win_rate >= 50 ? 'card-good' : 'card-bad');
  setText('ss-winrate', `${fmtNum(stats.wins)}W · ${fmtNum(stats.losses)}L`);

  const pnlEl = document.getElementById('sv-pnl');
  const pnlCard = document.getElementById('sc-pnl');
  const pnlSign = stats.total_pnl >= 0 ? '+' : '';
  pnlEl.textContent = `${pnlSign}$${Math.abs(stats.total_pnl).toFixed(4)}`;
  pnlEl.className = 'stat-value ' + (stats.total_pnl >= 0 ? 'pos' : 'neg');
  pnlCard.classList.remove('card-good', 'card-bad');
  pnlCard.classList.add(stats.total_pnl >= 0 ? 'card-good' : 'card-bad');
  const pnlSubSign = stats.total_pnl >= 0 ? '+' : '';
  setText('ss-pnl', `${pnlSubSign}${((stats.total_pnl / Math.max(stats.initial_balance, 0.01)) * 100).toFixed(2)}% return`);

  const balEl = document.getElementById('sv-balance');
  balEl.textContent = `$${stats.current_balance.toFixed(2)}`;
  balEl.className = 'stat-value acc';
  setText('ss-balance', `initial: $${stats.initial_balance.toFixed(2)}`);

  setText('sv-sessions', fmtNum(stats.sessions));
  setText('ss-sessions', `CSV run files`);

  // ── Meta bar ─────────────────────────────────────────────────
  setText('meta-algo',    stats.algo || '—');
  setText('meta-updated', stats.last_updated ? fmtTime(stats.last_updated) : '—');
  setText('meta-records', fmtNum(total_records));
  setText('meta-skips',   fmtNum(stats.skips));
  setText('meta-invalid', fmtNum(stats.invalid));

  // ── Daily P&L ────────────────────────────────────────────────
  renderDailyPnl(data.daily_pnl, stats.initial_balance);

  // ── Trade table ──────────────────────────────────────────────
  const count = trades.length;
  setText('tth-count', `${fmtNum(total_records)} total · showing ${fmtNum(count)}`);

  if (count === 0) {
    tradeTbody.innerHTML = '<tr class="trade-empty"><td colspan="9">No trade records found.</td></tr>';
    return;
  }

  // Render most-recent first
  const rows = [...trades].reverse().map(t => buildTradeRow(t)).join('');
  tradeTbody.innerHTML = rows;
}

function buildTradeRow(t) {
  const reason = t.exit_reason || '';
  const isWin  = (reason === 'resolution') && t.pnl > 0;
  const isLoss = (reason === 'resolution' && t.pnl <= 0) || reason === 'stop_loss';
  const isSkip = reason === 'skip';

  let rowCls = 'tr-invalid';
  if (isWin)        rowCls = 'tr-win';
  else if (isLoss)  rowCls = 'tr-loss';
  else if (isSkip)  rowCls = 'tr-skip';

  const predChip = chipPred(t.prediction);
  const sideChip = chipPred(t.side || t.prediction);
  const exitChip = chipExit(reason);

  const pnlHtml = fmtPnlCell(t.pnl, reason);
  const balHtml = t.balance_after > 0
    ? `<span style="color:var(--text-mid)">$${t.balance_after.toFixed(2)}</span>`
    : `<span style="color:var(--text-dim)">—</span>`;

  const diff = t.btc_diff;
  const diffSign = diff >= 0 ? '+' : '';
  const diffCls  = diff >= 0 ? 'td-diff-pos' : 'td-diff-neg';
  const diffHtml = `<span class="${diffCls}">${diffSign}$${Math.abs(diff).toFixed(0)}</span>`;

  const stakeHtml = t.stake_usd > 0
    ? `$${t.stake_usd.toFixed(2)}`
    : `<span style="color:var(--text-dim)">—</span>`;

  const timeShort = fmtTime(t.timestamp);
  const market = t.market_slug.replace('btc-updown-15m-', '');

  return `<tr class="${rowCls}">
    <td class="td-time">${esc(timeShort)}</td>
    <td class="td-market" title="${esc(t.market_slug)}">${esc(market)}</td>
    <td>${predChip}</td>
    <td>${sideChip}</td>
    <td>${exitChip}</td>
    <td class="num">${pnlHtml}</td>
    <td class="num">${balHtml}</td>
    <td class="num">${diffHtml}</td>
    <td class="num">${stakeHtml}</td>
  </tr>`;
}

// ── Daily P&L ─────────────────────────────────────────────────────────────

function renderDailyPnl(daily, initialBalance) {
  if (!daily || daily.length === 0) {
    dailyPnlDaysEl.textContent = '0 days';
    dailyPnlRowsEl.innerHTML = '<div class="dpnl-empty">No bet data yet</div>';
    return;
  }

  dailyPnlDaysEl.textContent = `${daily.length} day${daily.length !== 1 ? 's' : ''}`;

  const maxAbs = Math.max(...daily.map(d => Math.abs(d.pnl)), 0.01);

  // Most-recent day first
  const rows = daily.slice().reverse().map(d => {
    const isPos    = d.pnl >= 0;
    const sign     = isPos ? '+' : '';
    const cls      = isPos ? 'dpnl-pos' : 'dpnl-neg';
    const barPct   = (Math.abs(d.pnl) / maxAbs * 100).toFixed(1);
    const winRate  = d.bets > 0 ? ((d.wins / d.bets) * 100).toFixed(0) : '—';
    const retPct   = initialBalance > 0
      ? (sign + ((d.pnl / initialBalance) * 100).toFixed(2) + '%')
      : '—';
    const dateShort = d.date.slice(5); // "MM-DD"

    return `<div class="dpnl-row">
      <span class="dpnl-date">${esc(dateShort)}</span>
      <div class="dpnl-bar-track">
        <div class="dpnl-bar ${cls}" style="width:${barPct}%"></div>
      </div>
      <span class="dpnl-val ${cls}">${sign}$${Math.abs(d.pnl).toFixed(4)}<span class="dpnl-pct">&nbsp;(${retPct})</span></span>
      <span class="dpnl-bets">${fmtNum(d.bets)} bets</span>
      <span class="dpnl-wl">${fmtNum(d.wins)}W&nbsp;${fmtNum(d.losses)}L</span>
      <span class="dpnl-wr ${isPos ? 'dpnl-pos' : 'dpnl-neg'}">${winRate}%</span>
    </div>`;
  }).join('');

  dailyPnlRowsEl.innerHTML = rows;
}

// ── Chip helpers ──────────────────────────────────────────────────────────

function chipPred(val) {
  if (!val) return `<span class="chip chip-blank">—</span>`;
  const v = val.toUpperCase();
  if (v === 'UP'   || v === 'BUY_UP')   return `<span class="chip chip-up">▲ ${esc(val)}</span>`;
  if (v === 'DOWN' || v === 'BUY_DOWN') return `<span class="chip chip-down">▼ ${esc(val)}</span>`;
  if (v === 'SKIP')                     return `<span class="chip chip-skip">⊘ SKIP</span>`;
  return `<span class="chip chip-blank">${esc(val)}</span>`;
}

function chipExit(reason) {
  const cls = {
    resolution: 'exit-resolution',
    stop_loss:  'exit-stop_loss',
    skip:       'exit-skip',
    invalid_price: 'exit-invalid',
    too_small:  'exit-invalid',
    buy_failed: 'exit-invalid',
  }[reason] || 'exit-other';
  return `<span class="exit-chip ${cls}">${esc(reason || '—')}</span>`;
}

function fmtPnlCell(pnl, reason) {
  if (reason === 'skip' || reason === 'invalid_price' || reason === 'too_small' || reason === 'buy_failed') {
    return `<span style="color:var(--text-dim)">—</span>`;
  }
  if (pnl === 0) return `<span class="td-pnl-zer">$0.0000</span>`;
  const sign = pnl > 0 ? '+' : '';
  const cls  = pnl > 0 ? 'td-pnl-pos' : 'td-pnl-neg';
  return `<span class="${cls}">${sign}$${Math.abs(pnl).toFixed(4)}</span>`;
}

// ── Utility ───────────────────────────────────────────────────────────────

function setText(id, val) {
  const el = document.getElementById(id);
  if (el) el.textContent = val;
}

function fmtNum(n) {
  return Number(n).toLocaleString('en-US');
}

function fmtTime(ts) {
  if (!ts) return '—';
  const date = new Date(ts.replace(' ', 'T') + 'Z');
  if (isNaN(date.getTime())) return ts.slice(5, 19);
  const istOffset = 5.5 * 60 * 60 * 1000;
  const istDate = new Date(date.getTime() + istOffset);
  const day = String(istDate.getUTCDate()).padStart(2, '0');
  const month = String(istDate.getUTCMonth() + 1).padStart(2, '0');
  const hours = String(istDate.getUTCHours()).padStart(2, '0');
  const minutes = String(istDate.getUTCMinutes()).padStart(2, '0');
  const seconds = String(istDate.getUTCSeconds()).padStart(2, '0');
  return `${day}/${month} ${hours}:${minutes}:${seconds}`;
}

function esc(str) {
  if (!str) return '';
  return str.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

// ── Auto-refresh ──────────────────────────────────────────────────────────

function startAutoRefresh() {
  clearInterval(refreshTimer);
  refreshTimer = setInterval(loadTrades, 15_000);
}

refreshBtn.addEventListener('click', loadTrades);

// ═══════════════════════════════════════════════════════════════
// TERMINAL — WebSocket + pane management (unchanged logic)
// ═══════════════════════════════════════════════════════════════

async function loadPanes() {
  try {
    const resp  = await fetch('/api/panes');
    const panes = await resp.json();
    panes.forEach(p => {
      manager.ensurePane(p.pane_id, p.label || p.pane_id);
      const opt = document.createElement('option');
      opt.value = p.pane_id;
      opt.textContent = p.label || p.pane_id;
      paneSelEl.appendChild(opt);
    });
    // Always connect to all panes so the bot pane (3.0) output is never missed
    // regardless of discovery order; user can filter via the selector
    connectAll();
  } catch (e) {
    console.error('Failed to load panes:', e);
    connectAll();
  }
}

let activeClient = null;

function connectPane(paneId) {
  if (activeClient) activeClient.close();
  const url = `${WS_BASE}?pane_id=${encodeURIComponent(paneId)}`;
  activeClient = new WsClient(url, (msg) => manager.handleEvent(msg));
}

function connectAll() {
  if (activeClient) activeClient.close();
  activeClient = new WsClient(WS_BASE, (msg) => manager.handleEvent(msg));
}

paneSelEl.addEventListener('change', () => {
  const val = paneSelEl.value;
  if (val === '__all__') {
    connectAll();
  } else if (val) {
    connectPane(val);
    manager.activate(val);
  }
});

document.addEventListener('ws:open',  () => {
  statusEl.textContent = '● Connected';
  statusEl.className   = 'status-connected';
});
document.addEventListener('ws:close', () => {
  statusEl.textContent = '● Reconnecting…';
  statusEl.className   = 'status-disconnected';
});

searchEl && searchEl.addEventListener('input', () => {
  const q = searchEl.value.toLowerCase();
  document.querySelectorAll('.vl-row').forEach(row => {
    row.style.opacity = (!q || row.textContent.toLowerCase().includes(q)) ? '1' : '0.2';
  });
});

const addPaneForm = document.getElementById('add-pane-form');
if (addPaneForm) {
  addPaneForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const paneId = document.getElementById('add-pane-id').value.trim();
    const label  = document.getElementById('add-pane-label').value.trim() || undefined;
    if (!paneId) return;
    try {
      const resp = await fetch('/api/panes', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pane_id: paneId, label }),
      });
      const data = await resp.json();
      if (data.ok) {
        manager.ensurePane(paneId, label || paneId);
        const opt = document.createElement('option');
        opt.value = paneId;
        opt.textContent = label || paneId;
        paneSelEl.appendChild(opt);
        addPaneForm.reset();
      } else {
        alert('Error: ' + data.message);
      }
    } catch (e) {
      alert('Request failed: ' + e.message);
    }
  });
}

// ═══════════════════════════════════════════════════════════════
// BOOT
// ═══════════════════════════════════════════════════════════════

loadTrades();
loadPanes();
startAutoRefresh();
