/* ==========================================================================
   RetinaX — Empty State (empty-state.js)
   Placeholder shown when a list or dashboard has no records yet.

   Usage:
     <empty-state heading="No records yet" message="Records you add will appear here."
                  action-label="Add record"></empty-state>

   The action button is only rendered when `action-label` is set. Clicking it fires an
   `empty-state-action` event so the host page decides what happens next.
   ========================================================================== */

(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.EmptyState = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};
  const DEFAULT_HEADING = 'No records yet';
  const DEFAULT_MESSAGE = 'There is nothing to show here yet.';

  /**
   * Returns the trimmed attribute text, or the fallback when it is missing or blank.
   * @param {string|null|undefined} value
   * @param {string} fallback
   * @returns {string}
   */
  function textOrDefault(value, fallback) {
    const text = String(value ?? '').trim();
    return text || fallback;
  }

  class EmptyState extends ElementBase {
    static get observedAttributes() {
      return ['heading', 'message', 'action-label'];
    }

    connectedCallback() {
      this.render();
    }

    attributeChangedCallback(name, oldValue, newValue) {
      if (oldValue === newValue || !this.isConnected) return;
      this.render();
    }

    render() {
      const heading = textOrDefault(this.getAttribute('heading'), DEFAULT_HEADING);
      const message = textOrDefault(this.getAttribute('message'), DEFAULT_MESSAGE);
      const actionLabel = textOrDefault(this.getAttribute('action-label'), '');

      this.setAttribute('role', 'status');
      this.textContent = '';

      const wrapper = document.createElement('div');
      wrapper.className = 'empty-state';

      const icon = document.createElement('span');
      icon.className = 'empty-state__icon';
      icon.setAttribute('aria-hidden', 'true');
      icon.innerHTML =
        '<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/></svg>';

      const title = document.createElement('h3');
      title.className = 'empty-state__heading';
      title.textContent = heading;

      const text = document.createElement('p');
      text.className = 'empty-state__message';
      text.textContent = message;

      wrapper.append(icon, title, text);

      if (actionLabel) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'empty-state__action';
        button.textContent = actionLabel;
        button.addEventListener('click', () => {
          this.dispatchEvent(
            new CustomEvent('empty-state-action', { bubbles: true, composed: true })
          );
        });
        wrapper.appendChild(button);
      }

      this.appendChild(wrapper);
    }
  }

  if (typeof customElements !== 'undefined' && !customElements.get('empty-state'))
    customElements.define('empty-state', EmptyState);

  return { EmptyState, textOrDefault, DEFAULT_HEADING, DEFAULT_MESSAGE };
});
