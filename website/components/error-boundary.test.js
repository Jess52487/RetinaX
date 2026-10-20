const { test, describe } = require('node:test');
const assert = require('node:assert');

// ---- Shim minimal browser globals ----
function MockHTMLElement() {
  this.dataset = {};
  this._children = [];
  this.innerHTML = '';
  this.className = '';
  this.style = {};
  this.classList = {
    _classes: new Set(),
    add: (c) => this.classList._classes.add(c),
    remove: (c) => this.classList._classes.delete(c),
    contains: (c) => this.classList._classes.has(c),
  };
  this.appendChild = (el) => {
    this._children.push(el);
    el.parentNode = this;
  };
  this.querySelector = (sel) => {
    if (sel === '.patient-card-component' || sel === '.repo-avatar-component') {
      return (
        this._children.find(
          (c) => c.className && c.className.includes(sel.replace('.', ''))
        ) || null
      );
    }
    if (sel === '.patient-card__retry-btn' || sel === '.repo-avatar__retry-btn') {
      return {
        addEventListener: function (evt, cb) {
          this._cb = cb;
        },
      };
    }
    if (sel === '.global-spinner-overlay') {
      return {
        classList: {
          add: () => {},
          remove: () => {},
        },
      };
    }
    return null;
  };
  this.querySelectorAll = () => [];
  this.getAttribute = () => null;
  this.setAttribute = () => {};
}

global.HTMLElement = MockHTMLElement;

global.document = {
  readyState: 'complete',
  body: { style: { overflow: '' } },
  querySelectorAll: () => [],
  addEventListener: () => {},
  removeEventListener: () => {},
  createElement: function (tag) {
    const el = new MockHTMLElement();
    el._tag = tag;
    return el;
  },
};
global.window = global;
global.customElements = {
  get: () => false,
  define: () => {},
};

const PatientCard = require('./patient-card.js');
const RepoAvatar = require('./repo-avatar.js');
require('./global-spinner.js');

describe('Web Component Error Boundaries (#290)', () => {
  test('PatientCard gracefully recovers from render error and renders user-friendly error UI', () => {
    const mount = new MockHTMLElement();
    const pc = new PatientCard(mount, {
      patientName: 'Test Patient',
    });

    pc.showError('Network error reading patient FHIR resource');
    const card = mount.querySelector('.patient-card-component');
    assert.ok(card);
    assert.ok(card.innerHTML.includes('Unable to load patient card'));
    assert.ok(card.innerHTML.includes('Network error reading patient FHIR resource'));
    assert.ok(card.innerHTML.includes('patient-card__retry-btn'));
  });

  test('RepoAvatar handles render failure and displays user-friendly error UI with retry action', () => {
    const mount = new MockHTMLElement();
    const ra = new RepoAvatar(mount, {
      org: 'RetinaX-Org',
      repo: 'RetinaX',
    });

    ra.showError('Failed to fetch repository telemetry from Soroban RPC');
    const card = mount.querySelector('.repo-avatar-component');
    assert.ok(card);
    assert.ok(card.innerHTML.includes('Unable to load'));
    assert.ok(card.innerHTML.includes('Failed to fetch repository telemetry from Soroban RPC'));
    assert.ok(card.innerHTML.includes('repo-avatar__retry-btn'));
  });

  test('GlobalSpinner handles render and visibility safely without throwing', () => {
    const spinner = new global.GlobalSpinner();
    spinner.render();
    assert.ok(spinner.innerHTML.includes('global-spinner-overlay'));

    spinner.showError('Contract timeout in Soroban RPC');
    assert.strictEqual(spinner.visible, true);
    assert.strictEqual(spinner.message, 'Contract timeout in Soroban RPC');

    spinner.hide();
    assert.strictEqual(spinner.visible, false);
    assert.strictEqual(global.document.body.style.overflow, '');
  });
});
