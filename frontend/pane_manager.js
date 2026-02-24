/**
 * pane_manager.js — Per-pane state + tab switching
 */
import { VirtualList } from './virtual_list.js';
import { renderEvent }  from './semantic_renderer.js';

export class PaneManager {
  constructor(tabsEl, contentEl) {
    this.tabsEl    = tabsEl;
    this.contentEl = contentEl;
    this.panes     = new Map(); // pane_id → { vlist, tab, lineCount }
    this.activeId  = null;
  }

  ensurePane(paneId, label) {
    if (this.panes.has(paneId)) return;

    // Create tab
    const tab = document.createElement('button');
    tab.className   = 'pane-tab';
    tab.textContent = label || paneId;
    tab.dataset.paneId = paneId;
    tab.onclick = () => this.activate(paneId);
    this.tabsEl.appendChild(tab);

    // Create content area
    const wrap = document.createElement('div');
    wrap.className   = 'pane-content';
    wrap.dataset.paneId = paneId;
    wrap.style.display = 'none';
    this.contentEl.appendChild(wrap);

    const vlist = new VirtualList(wrap);
    this.panes.set(paneId, { vlist, tab, wrap, lineCount: 0 });

    if (!this.activeId) this.activate(paneId);
  }

  activate(paneId) {
    if (!this.panes.has(paneId)) return;
    this.panes.forEach(({ tab, wrap }, id) => {
      const active = id === paneId;
      tab.classList.toggle('active', active);
      wrap.style.display = active ? 'flex' : 'none';
    });
    this.activeId = paneId;
  }

  handleEvent(event) {
    const { pane_id, type } = event;

    switch (type) {
      case 'pane_registered': {
        const { label } = event;
        this.ensurePane(pane_id, label);
        break;
      }

      case 'snapshot': {
        this.ensurePane(pane_id);
        const pane = this.panes.get(pane_id);
        const htmlLines = (event.lines || [])
          .map(e => renderEvent(e))
          .filter(Boolean);
        pane.vlist.replaceAll(htmlLines);
        pane.lineCount = event.total_lines || htmlLines.length;
        pane.tab.querySelector('.pane-tab-count') && (pane.tab.querySelector('.pane-tab-count').textContent = pane.lineCount);
        break;
      }

      case 'line': {
        this.ensurePane(pane_id);
        const pane = this.panes.get(pane_id);
        const html = renderEvent(event);
        if (html) pane.vlist.push(html);
        pane.lineCount++;
        // Update tab badge
        let badge = pane.tab.querySelector('.pane-tab-count');
        if (!badge) {
          badge = document.createElement('span');
          badge.className = 'pane-tab-count';
          pane.tab.appendChild(badge);
        }
        badge.textContent = pane.lineCount;
        break;
      }

      case 'error': {
        this.ensurePane(pane_id || 'system');
        const pane = this.panes.get(pane_id || 'system');
        pane.vlist.push(`<div class="line-error">⚠ ${event.message}</div>`);
        break;
      }

      case 'ping':
        break; // keep-alive, ignore

      default:
        break;
    }
  }
}
