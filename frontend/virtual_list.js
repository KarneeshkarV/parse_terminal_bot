/**
 * virtual_list.js — Variable-height virtualised log renderer
 * Tracks actual row heights after render so tall cards don't overlap
 * the rows below them. Auto-scrolls to bottom unless user scrolled up.
 */
export class VirtualList {
  constructor(container, rowHeight = 22, maxItems = 5000) {
    this.container    = container;
    this.rowHeight    = rowHeight;   // default estimate for unmeasured rows
    this.maxItems     = maxItems;
    this.items        = [];          // { html: string, id: number }
    this.nextId       = 0;
    this.autoScroll   = true;
    this._pending     = false;

    // Variable-height tracking
    this._heights      = [];         // measured height per item index (sparse)
    this._offsets      = [0];        // prefix sum: _offsets[i] = top of item i
    this._offsetsDirty = true;

    // Outer scrollable div
    this.scroller = document.createElement('div');
    this.scroller.className = 'vl-scroller';
    container.appendChild(this.scroller);

    // Spacer div sets total scroll height
    this.spacer = document.createElement('div');
    this.spacer.className = 'vl-spacer';
    this.scroller.appendChild(this.spacer);

    // Render area
    this.runway = document.createElement('div');
    this.runway.className = 'vl-runway';
    this.scroller.appendChild(this.runway);

    this.scroller.addEventListener('scroll', () => this._onScroll(), { passive: true });
    this._resizeObs = new ResizeObserver(() => this._scheduleRender());
    this._resizeObs.observe(this.scroller);
  }

  _onScroll() {
    const { scrollTop, scrollHeight, clientHeight } = this.scroller;
    this.autoScroll = (scrollHeight - scrollTop - clientHeight) < this.rowHeight * 2;
    this._scheduleRender();
  }

  push(html) {
    this.items.push({ html, id: this.nextId++ });
    if (this.items.length > this.maxItems) {
      this.items.shift();
      this._heights.shift();
    }
    this._offsetsDirty = true;
    this._scheduleRender();
  }

  replaceAll(htmlArray) {
    this.items = htmlArray.map(h => ({ html: h, id: this.nextId++ }));
    this._heights = [];
    this._offsetsDirty = true;
    this.autoScroll = true;
    this._scheduleRender();
  }

  clear() {
    this.items = [];
    this._heights = [];
    this._offsets = [0];
    this._offsetsDirty = false;
    this._scheduleRender();
  }

  // Rebuild prefix-sum offsets from measured (or estimated) heights
  _computeOffsets() {
    if (!this._offsetsDirty) return;
    const n = this.items.length;
    this._offsets = new Array(n + 1);
    this._offsets[0] = 0;
    for (let i = 0; i < n; i++) {
      this._offsets[i + 1] = this._offsets[i] + (this._heights[i] || this.rowHeight);
    }
    this._offsetsDirty = false;
  }

  // Binary search: first item whose bottom edge is > scrollTop
  _findFirstVisible(scrollTop) {
    const n = this.items.length;
    let lo = 0, hi = n;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this._offsets[mid + 1] <= scrollTop) lo = mid + 1;
      else hi = mid;
    }
    return Math.max(0, lo - 5);
  }

  _scheduleRender() {
    if (this._pending) return;
    this._pending = true;
    requestAnimationFrame(() => {
      this._pending = false;
      this._render();
    });
  }

  _render() {
    this._computeOffsets();
    const total  = this.items.length;
    const totalH = this._offsets[total] || 0;
    this.spacer.style.height = totalH + 'px';

    const viewH     = this.scroller.clientHeight;
    const scrollTop = this.autoScroll
      ? Math.max(0, totalH - viewH)
      : this.scroller.scrollTop;

    if (this.autoScroll) {
      this.scroller.scrollTop = scrollTop;
    }

    const firstVisible = this._findFirstVisible(scrollTop);
    let lastVisible = firstVisible;
    while (
      lastVisible < total - 1 &&
      this._offsets[lastVisible + 1] < scrollTop + viewH
    ) lastVisible++;
    lastVisible = Math.min(total - 1, lastVisible + 5);

    // Build fragment with computed top positions
    const frag = document.createDocumentFragment();
    for (let i = firstVisible; i <= lastVisible; i++) {
      const row       = document.createElement('div');
      row.className   = 'vl-row';
      row.dataset.idx = i;
      row.style.top   = this._offsets[i] + 'px';
      row.innerHTML   = this.items[i].html;
      frag.appendChild(row);
    }

    this.runway.innerHTML = '';
    this.runway.appendChild(frag);

    // Post-render: measure actual heights; re-render if any card was taller than estimated
    requestAnimationFrame(() => this._measureRows());
  }

  _measureRows() {
    let changed = false;
    this.runway.querySelectorAll('.vl-row').forEach(row => {
      const idx = parseInt(row.dataset.idx, 10);
      const h   = row.getBoundingClientRect().height;
      if (h > 0 && Math.round(h) !== Math.round(this._heights[idx] ?? 0)) {
        this._heights[idx] = h;
        changed = true;
      }
    });
    if (changed) {
      this._offsetsDirty = true;
      this._scheduleRender();
    }
  }
}
