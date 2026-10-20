(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.ContractAddressDisplay = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';
  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};
  function shorten(address, start, end) {
    const value = String(address || '').trim();
    return value.length <= start + end + 1
      ? value
      : value.slice(0, start) + '…' + value.slice(-end);
  }
  class ContractAddressDisplay extends ElementBase {
    static get observedAttributes() {
      return ['address', 'loading'];
    }
    connectedCallback() {
      this.render();
    }
    attributeChangedCallback() {
      if (this.isConnected) this.render();
    }
    render() {
      if (this.hasAttribute('loading')) {
        this.innerHTML =
          '<div class="contract-address-display__skeleton is-loading" role="status" aria-busy="true" aria-label="Loading contract address...">' +
          '<span class="skeleton-line skeleton-pill" style="width: 140px; height: 26px; border-radius: 9999px;"></span>' +
          '</div>';
        return;
      }
      const address =
        (this.getAttribute && this.getAttribute('address')) || this.textContent.trim();
      if (!address) return;
      this._address = address;
      this.innerHTML =
        '<button type="button" class="contract-address-display__button skeleton-fade-in" aria-label="Copy contract address" title="Copy contract address">' +
        '<span class="contract-address-display__value">' +
        shorten(address, 6, 6) +
        '</span>' +
        '<span class="contract-address-display__status" aria-live="polite"></span></button>';
      this._button = this.querySelector('button');
      this._status = this.querySelector('.contract-address-display__status');
      if (this._button) this._button.addEventListener('click', () => this.copy());
    }
    async copy() {
      try {
        if (navigator.clipboard && navigator.clipboard.writeText)
          await navigator.clipboard.writeText(this._address);
        else {
          const input = document.createElement('textarea');
          input.value = this._address;
          input.setAttribute('readonly', '');
          input.style.position = 'fixed';
          input.style.opacity = '0';
          document.body.appendChild(input);
          input.select();
          document.execCommand('copy');
          input.remove();
        }
        this._status.textContent = 'Copied';
        this._button.setAttribute('aria-label', 'Contract address copied');
      } catch (_) {
        this._status.textContent = 'Copy failed';
        this._button.setAttribute('aria-label', 'Copy contract address failed');
      }
    }
  }
  if (typeof customElements !== 'undefined' && !customElements.get('contract-address-display'))
    customElements.define('contract-address-display', ContractAddressDisplay);
  return ContractAddressDisplay;
});
