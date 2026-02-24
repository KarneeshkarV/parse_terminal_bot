/**
 * ws_client.js — WebSocket client with exponential reconnect
 */
export class WsClient {
  constructor(url, onMessage) {
    this.url         = url;
    this.onMessage   = onMessage;
    this.ws          = null;
    this.retryDelay  = 1000;
    this.maxDelay    = 30000;
    this.closed      = false;
    this._connect();
  }

  _connect() {
    if (this.closed) return;
    this.ws = new WebSocket(this.url);

    this.ws.onopen = () => {
      console.log('[WS] Connected:', this.url);
      this.retryDelay = 1000;
      document.dispatchEvent(new CustomEvent('ws:open', { detail: { url: this.url } }));
    };

    this.ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        this.onMessage(msg);
      } catch (e) {
        console.error('[WS] JSON parse error:', e);
      }
    };

    this.ws.onclose = () => {
      if (this.closed) return;
      console.warn(`[WS] Disconnected. Reconnecting in ${this.retryDelay}ms…`);
      document.dispatchEvent(new CustomEvent('ws:close'));
      setTimeout(() => this._connect(), this.retryDelay);
      this.retryDelay = Math.min(this.retryDelay * 2, this.maxDelay);
    };

    this.ws.onerror = (err) => {
      console.error('[WS] Error:', err);
    };
  }

  send(obj) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(obj));
    }
  }

  close() {
    this.closed = true;
    if (this.ws) this.ws.close();
  }
}
