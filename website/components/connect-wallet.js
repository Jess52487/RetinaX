/* ==========================================================================
   RetinaX — Connect Wallet Button Component
   Placeholder component for wallet connection functionality.
   Exposes window.ConnectWalletButton for programmatic control.
   ========================================================================== */

(function () {
  'use strict';

  class ConnectWalletButton extends HTMLElement {
    static get observedAttributes() {
      return ['label', 'connected', 'address', 'network'];
    }

    constructor() {
      super();
      this._label = 'Connect Wallet';
      this._connected = false;
      this._address = '';
      this._network = 'Stellar Testnet';
    }

    connectedCallback() {
      this.render();
    }

    disconnectedCallback() {
      const btn = this.querySelector('.connect-wallet-btn');
      if (btn) {
        btn.removeEventListener('click', this._handleClick);
      }
    }

    attributeChangedCallback(name, oldValue, newValue) {
      if (oldValue === newValue) return;
      if (name === 'label') {
        this._label = newValue || 'Connect Wallet';
      } else if (name === 'connected') {
        this._connected = newValue !== null && newValue !== 'false';
      } else if (name === 'address') {
        this._address = newValue || '';
      } else if (name === 'network') {
        this._network = newValue || 'Stellar Testnet';
      }
      this.render();
    }

    get label() {
      return this._label;
    }

    set label(val) {
      this._label = val || 'Connect Wallet';
      this.render();
    }

    get connected() {
      return this._connected;
    }

    set connected(val) {
      this._connected = val;
      this.render();
    }

    get address() {
      return this._address;
    }

    set address(val) {
      this._address = val || '';
      this.render();
    }

    get network() {
      return this._network;
    }

    set network(val) {
      this._network = val || 'Stellar Testnet';
      this.render();
    }

    render() {
      const displayLabel = this._connected
        ? this._shortenAddress(this._address)
        : this._label;
      const t = (key, vars, fallback) => {
        if (typeof window !== 'undefined' && window.RetinaXI18n && window.RetinaXI18n.t) {
          return window.RetinaXI18n.t(key, vars, fallback);
        }
        return fallback;
      };
      const ariaLabel = this._connected
        ? t('wallet.connectedAria', { address: this._address }, 'Wallet connected: ' + this._address)
        : t('nav.connectWalletAria', {}, 'Connect Stellar Wallet');

      this.innerHTML = `
        <button class="connect-wallet-btn ${this._connected ? 'connected' : ''}" type="button" aria-label="${ariaLabel}">
          <span class="connect-wallet-icon">
            ${this._connected
              ? '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>'
              : '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12V7H5a2 2 0 0 1 0-4h14v4"/><path d="M3 5v14a2 2 0 0 0 2 2h16v-5"/><path d="M18 12a2 2 0 0 0 0 4h4v-4Z"/></svg>'
            }
          </span>
          <span class="connect-wallet-label">${displayLabel}</span>
          ${this._connected
            ? '<span class="connect-wallet-status-dot"></span>'
            : ''
          }
        </button>
      `;
      this._setupListeners();
    }

    _setupListeners() {
      const btn = this.querySelector('.connect-wallet-btn');
      if (btn) {
        this._handleClick = (e) => {
          e.preventDefault();
          if (this._connected) {
            this.dispatchEvent(new CustomEvent('wallet-disconnect', { bubbles: true, composed: true }));
          } else {
            this.dispatchEvent(new CustomEvent('wallet-connect', { bubbles: true, composed: true }));
          }
        };
        btn.addEventListener('click', this._handleClick);
      }
    }

    _shortenAddress(addr) {
      if (!addr || addr.length <= 12) return addr;
      return addr.slice(0, 6) + '...' + addr.slice(-4);
    }

    connect(address) {
      this._connected = true;
      this._address = address || '';
      this.render();
    }

    disconnect() {
      this._connected = false;
      this._address = '';
      this.render();
    }
  }

  if (!customElements.get('connect-wallet-button')) {
    customElements.define('connect-wallet-button', ConnectWalletButton);
  }

  window.ConnectWalletButton = ConnectWalletButton;
})();
