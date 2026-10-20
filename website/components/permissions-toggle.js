/* ==========================================================================
   RetinaX — Permissions Toggle (permissions-toggle.js)
   Accessible on/off switch for granting or revoking a permission on the
   access control dashboard.

   Usage:
     <permissions-toggle permission="read_records" label="Read records" checked></permissions-toggle>

   Attributes: `permission` (identifier), `label` (visible text), `checked`, `disabled`.
   Clicking the switch, or pressing Space or Enter on it, flips `checked` and fires a
   `permission-change` event with `detail: { permission, granted }`. The component does
   not call the contract itself, so the host page decides how to persist the change.
   ========================================================================== */

(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.PermissionsToggle = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};

  /**
   * Returns the checked state after a toggle attempt. Disabled switches never change.
   * @param {boolean} checked Current state.
   * @param {boolean} disabled Whether the switch is disabled.
   * @returns {boolean}
   */
  function nextState(checked, disabled) {
    return disabled ? Boolean(checked) : !checked;
  }

  class PermissionsToggle extends ElementBase {
    static get observedAttributes() {
      return ['permission', 'label', 'checked', 'disabled'];
    }

    connectedCallback() {
      this.render();
    }

    attributeChangedCallback(name, oldValue, newValue) {
      if (oldValue === newValue || !this.isConnected) return;
      this.render();
    }

    get checked() {
      return this.hasAttribute('checked');
    }

    set checked(value) {
      this.toggleAttribute('checked', Boolean(value));
    }

    get disabled() {
      return this.hasAttribute('disabled');
    }

    set disabled(value) {
      this.toggleAttribute('disabled', Boolean(value));
    }

    render() {
      const label = this.getAttribute('label') || this.getAttribute('permission') || 'Permission';
      this.textContent = '';

      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'permissions-toggle';
      button.setAttribute('role', 'switch');
      button.setAttribute('aria-checked', String(this.checked));
      button.disabled = this.disabled;

      const track = document.createElement('span');
      track.className = 'permissions-toggle__track';
      track.setAttribute('aria-hidden', 'true');
      const thumb = document.createElement('span');
      thumb.className = 'permissions-toggle__thumb';
      track.appendChild(thumb);

      const text = document.createElement('span');
      text.className = 'permissions-toggle__label';
      text.textContent = label;

      button.append(track, text);
      button.addEventListener('click', () => this.toggle());
      this.appendChild(button);
    }

    toggle() {
      const next = nextState(this.checked, this.disabled);
      if (next === this.checked) return;
      this.checked = next;
      this.dispatchEvent(
        new CustomEvent('permission-change', {
          bubbles: true,
          composed: true,
          detail: { permission: this.getAttribute('permission') || '', granted: next },
        })
      );
    }
  }

  if (typeof customElements !== 'undefined' && !customElements.get('permissions-toggle'))
    customElements.define('permissions-toggle', PermissionsToggle);

  return { PermissionsToggle, nextState };
});
