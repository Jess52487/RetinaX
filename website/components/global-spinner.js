/* ==========================================================================
   RetinaX — Global Loading Spinner
   Overlay spinner for contract read operations and async tasks.
   Exposes window.GlobalSpinner with show()/hide() API.
   ========================================================================== */

(function () {
  'use strict';

  class GlobalSpinner extends HTMLElement {
    static get observedAttributes() {
      return ['visible', 'message'];
    }

    constructor() {
      super();
      this._visible = false;
      this._message = 'Loading...';
      this._handleKeydown = this.handleKeydown.bind(this);
    }

    connectedCallback() {
      try {
        this.render();
        this.updateVisibility();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Initialization failure:', err);
      }
    }

    disconnectedCallback() {
      try {
        if (typeof document !== 'undefined') {
          document.removeEventListener('keydown', this._handleKeydown);
          if (document.body) document.body.style.overflow = '';
        }
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Cleanup failure:', err);
      }
    }

    attributeChangedCallback(name, oldValue, newValue) {
      if (oldValue === newValue) return;
      try {
        if (name === 'visible') {
          this._visible = newValue !== null && newValue !== 'false';
        } else if (name === 'message') {
          this._message = newValue || 'Loading...';
        }
        this.render();
        this.updateVisibility();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Attribute change failure:', err, {
          name,
          newValue,
        });
      }
    }

    get visible() {
      return this._visible;
    }

    set visible(val) {
      this._visible = Boolean(val);
      try {
        this.render();
        this.updateVisibility();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Visibility setter failure:', err);
      }
    }

    get message() {
      return this._message;
    }

    set message(val) {
      this._message = val || 'Loading...';
      try {
        this.render();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Message setter failure:', err);
      }
    }

    render() {
      try {
        const safeMessage = typeof this._message === 'string' ? this._message : 'Loading...';
        this.innerHTML = `
          <div class="global-spinner-overlay" role="alert" aria-live="polite" aria-busy="${this._visible}">
            <div class="global-spinner-content">
              <div class="global-spinner-icon">
                <div class="global-spinner-ring"></div>
                <div class="global-spinner-ring"></div>
                <div class="global-spinner-ring"></div>
              </div>
              <p class="global-spinner-message">${safeMessage}</p>
            </div>
          </div>
        `;
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Render failure:', err);
        this.innerHTML = `
          <div class="global-spinner-overlay" role="alert" aria-live="polite">
            <div class="global-spinner-content">
              <p class="global-spinner-message">Loading...</p>
            </div>
          </div>
        `;
      }
    }

    updateVisibility() {
      try {
        const overlay = this.querySelector('.global-spinner-overlay');
        if (!overlay) return;

        if (this._visible) {
          overlay.classList.add('active');
          if (typeof document !== 'undefined' && document.body) {
            document.body.style.overflow = 'hidden';
            document.addEventListener('keydown', this._handleKeydown);
          }
        } else {
          overlay.classList.remove('active');
          if (typeof document !== 'undefined' && document.body) {
            document.body.style.overflow = '';
            document.removeEventListener('keydown', this._handleKeydown);
          }
        }
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Visibility update failure:', err);
        if (typeof document !== 'undefined' && document.body) {
          document.body.style.overflow = '';
        }
      }
    }

    handleKeydown(e) {
      if (e.key === 'Escape' && this._visible) {
        this.hide();
      }
    }

    show(message) {
      try {
        if (message) this._message = message;
        this._visible = true;
        this.render();
        this.updateVisibility();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Show failure:', err);
      }
    }

    hide() {
      try {
        this._visible = false;
        this.render();
        this.updateVisibility();
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] Hide failure:', err);
        if (typeof document !== 'undefined' && document.body) {
          document.body.style.overflow = '';
        }
      }
    }

    showError(errorMessage) {
      try {
        console.error('[GlobalSpinner] Error reported:', errorMessage);
        this._message = errorMessage || 'Operation encountered an error';
        this._visible = true;
        this.render();
        this.updateVisibility();
        setTimeout(() => {
          this.hide();
        }, 4000);
      } catch (err) {
        console.error('[GlobalSpinner Error Boundary] showError failure:', err);
        this.hide();
      }
    }
  }

  if (!customElements.get('global-spinner')) {
    customElements.define('global-spinner', GlobalSpinner);
  }

  window.GlobalSpinner = GlobalSpinner;
})();
