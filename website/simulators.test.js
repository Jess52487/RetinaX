/* ==========================================================================
   Interactive Simulator End-to-End Tests (Issue #295)
   Runs with Node.js test runner: node --test website/simulators.test.js
   ========================================================================== */

const { test, describe, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

// Read index.html for static structure assertions
const htmlPath = path.join(__dirname, 'index.html');
const indexHtml = fs.readFileSync(htmlPath, 'utf8');

// Minimal DOM Element Mock for unit testing simulator logic
class MockClassList {
  constructor(el) {
    this.el = el;
    this.classes = new Set();
  }
  add(...names) {
    names.forEach((n) => this.classes.add(n));
    this.el.className = Array.from(this.classes).join(' ');
  }
  remove(...names) {
    names.forEach((n) => this.classes.delete(n));
    this.el.className = Array.from(this.classes).join(' ');
  }
  contains(name) {
    return this.classes.has(name);
  }
  toggle(name, force) {
    if (force !== undefined) {
      if (force) this.add(name);
      else this.remove(name);
      return force;
    }
    if (this.contains(name)) {
      this.remove(name);
      return false;
    }
    this.add(name);
    return true;
  }
}

class MockElement {
  constructor(tagName = 'div', id = '', className = '') {
    this.tagName = tagName.toUpperCase();
    this.id = id;
    this.className = className;
    this.classList = new MockClassList(this);
    if (className) {
      className
        .split(/\s+/)
        .filter(Boolean)
        .forEach((c) => this.classList.classes.add(c));
    }
    this.dataset = {};
    this.attributes = {};
    this.listeners = {};
    this.children = [];
    this.parentNode = null;
    this.textContent = '';
    this.innerHTML = '';
    this.value = '';
    this.disabled = false;
    this.style = {};
  }

  setAttribute(k, v) {
    this.attributes[k] = String(v);
  }

  getAttribute(k) {
    return this.attributes[k] !== undefined ? this.attributes[k] : null;
  }

  removeAttribute(k) {
    delete this.attributes[k];
  }

  hasAttribute(k) {
    return this.attributes[k] !== undefined;
  }

  addEventListener(type, listener) {
    if (!this.listeners[type]) this.listeners[type] = [];
    this.listeners[type].push(listener);
  }

  removeEventListener(type, listener) {
    if (!this.listeners[type]) return;
    this.listeners[type] = this.listeners[type].filter((l) => l !== listener);
  }

  async dispatchEvent(event) {
    if (!event.target) event.target = this;
    const handlers = this.listeners[event.type] || [];
    for (const h of handlers) {
      await h(event);
    }
  }

  appendChild(child) {
    this.children.push(child);
    child.parentNode = this;
    return child;
  }

  removeChild(child) {
    this.children = this.children.filter((c) => c !== child);
    if (child.parentNode === this) child.parentNode = null;
    return child;
  }

  querySelector(sel) {
    const all = this.querySelectorAll(sel);
    return all.length ? all[0] : null;
  }

  querySelectorAll(sel) {
    const results = [];
    const traverse = (node) => {
      for (const ch of node.children) {
        if (matchesSelector(ch, sel)) results.push(ch);
        traverse(ch);
      }
    };
    traverse(this);
    return results;
  }

  closest(sel) {
    let curr = this;
    while (curr) {
      if (matchesSelector(curr, sel)) return curr;
      curr = curr.parentNode;
    }
    return null;
  }

  focus() {
    if (global.document) global.document.activeElement = this;
  }
}

function matchesSelector(el, sel) {
  if (sel.startsWith('#')) return el.id === sel.slice(1);
  if (sel.startsWith('.')) return el.classList.contains(sel.slice(1));
  if (sel.toLowerCase() === el.tagName.toLowerCase()) return true;
  return false;
}

// Minimal DOM environment setup
function setupMockDOM() {
  const elements = new Map();

  const getOrCreate = (id, tagName = 'div', className = '') => {
    if (!elements.has(id)) {
      const el = new MockElement(tagName, id, className);
      elements.set(id, el);
    }
    return elements.get(id);
  };

  const toastContainer = getOrCreate('retinax-toast-container', 'div', 'toast-container');

  const doc = {
    readyState: 'complete',
    activeElement: null,
    body: new MockElement('body', 'body'),
    getElementById: (id) => elements.get(id) || null,
    querySelectorAll: (sel) => {
      const arr = Array.from(elements.values());
      return arr.filter((el) => matchesSelector(el, sel));
    },
    querySelector: (sel) => {
      const arr = Array.from(elements.values());
      return arr.find((el) => matchesSelector(el, sel)) || null;
    },
    createElement: (tag) => {
      const el = new MockElement(tag);
      el.parentNode = doc.body;
      return el;
    },
    addEventListener: () => {},
  };

  global.document = doc;
  global.window = {
    document: doc,
    setTimeout: (fn, ms) => setTimeout(fn, Math.min(ms, 5)),
    clearTimeout: (id) => clearTimeout(id),
    showGlobalSpinner: () => {},
    hideGlobalSpinner: () => {},
    showErrorFeedback: () => {},
  };

  return { elements, getOrCreate };
}

// Helper to fast-track SorobanRPC delays in tests
function speedUpRPC(app) {
  const origInvoke = app.SorobanRPC.invoke.bind(app.SorobanRPC);
  app.SorobanRPC.invoke = async function (action, params = {}, options = {}) {
    const fastOptions = Object.assign({}, options, { delay: 10 });
    return origInvoke(action, params, fastOptions);
  };
}

describe('Interactive Simulator Structure in index.html (#295)', () => {
  test('index.html contains all 4 required simulator tabs plus data fetch', () => {
    assert.match(indexHtml, /id="tab-rbac"[^>]*data-tab="rbac"/);
    assert.match(indexHtml, /id="tab-zk"[^>]*data-tab="zk"/);
    assert.match(indexHtml, /id="tab-ai"[^>]*data-tab="ai"/);
    assert.match(indexHtml, /id="tab-fhir"[^>]*data-tab="fhir"/);
    assert.match(indexHtml, /id="tab-fetch"[^>]*data-tab="fetch"/);
  });

  test('index.html contains panels for each simulator module', () => {
    assert.match(indexHtml, /id="panel-rbac"/);
    assert.match(indexHtml, /id="panel-zk"/);
    assert.match(indexHtml, /id="panel-ai"/);
    assert.match(indexHtml, /id="panel-fhir"/);
    assert.match(indexHtml, /id="panel-fetch"/);
    assert.match(
      indexHtml,
      /class="[^"]*demo-tab-panel[^"]*"[^>]*id="panel-rbac"|id="panel-rbac"[^>]*class="[^"]*demo-tab-panel[^"]*"/
    );
  });

  test('index.html contains form action buttons for all simulators', () => {
    assert.match(indexHtml, /id="btn-grant-access"/);
    assert.match(indexHtml, /id="btn-gen-zk"/);
    assert.match(indexHtml, /id="btn-test-ai"/);
    assert.match(indexHtml, /id="btn-convert-fhir"/);
    assert.match(indexHtml, /id="btn-fetch-data"/);
  });

  test('index.html contains live code console preview elements', () => {
    assert.match(indexHtml, /id="preview-rbac-code"/);
    assert.match(indexHtml, /id="preview-zk-code"/);
    assert.match(indexHtml, /id="preview-ai-code"/);
    assert.match(indexHtml, /id="preview-fhir-code"/);
  });
});

describe('Interactive Simulator Tab Switching & Keyboard Navigation (#295)', () => {
  let app;
  let tabs;
  let panels;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();

    // Create 5 tabs and panels
    const tabNames = ['rbac', 'zk', 'ai', 'fhir', 'fetch'];
    tabs = tabNames.map((name, i) => {
      const tab = getOrCreate(`tab-${name}`, 'button', `demo-tab-btn ${i === 0 ? 'active' : ''}`);
      tab.dataset.tab = name;
      tab.setAttribute('role', 'tab');
      tab.setAttribute('aria-selected', i === 0 ? 'true' : 'false');
      tab.setAttribute('tabindex', i === 0 ? '0' : '-1');
      tab.setAttribute('aria-controls', `panel-${name}`);
      return tab;
    });

    panels = tabNames.map((name, i) => {
      const panel = getOrCreate(
        `panel-${name}`,
        'div',
        `demo-tab-panel ${i === 0 ? 'active' : ''}`
      );
      panel.setAttribute('role', 'tabpanel');
      panel.setAttribute('aria-labelledby', `tab-${name}`);
      if (i !== 0) panel.setAttribute('hidden', '');
      return panel;
    });

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    speedUpRPC(app);
    app.initDemoTabs();
  });

  test('clicking a simulator tab activates it and deactivates others', async () => {
    const zkTab = tabs[1];
    await zkTab.dispatchEvent({ type: 'click' });

    assert.ok(zkTab.classList.contains('active'), 'ZK tab should have active class');
    assert.strictEqual(zkTab.getAttribute('aria-selected'), 'true');
    assert.strictEqual(zkTab.getAttribute('tabindex'), '0');
    assert.ok(panels[1].classList.contains('active'), 'ZK panel should be active');
    assert.strictEqual(panels[1].hasAttribute('hidden'), false);

    assert.ok(!tabs[0].classList.contains('active'), 'RBAC tab should not be active');
    assert.strictEqual(tabs[0].getAttribute('aria-selected'), 'false');
    assert.strictEqual(tabs[0].getAttribute('tabindex'), '-1');
    assert.strictEqual(panels[0].hasAttribute('hidden'), true);
  });

  test('ArrowRight and ArrowLeft keyboard navigation cycles between simulator tabs', async () => {
    const rbacTab = tabs[0];
    let defaultPrevented = false;

    // ArrowRight from 0 -> 1 (ZK)
    await rbacTab.dispatchEvent({
      type: 'keydown',
      key: 'ArrowRight',
      preventDefault: () => {
        defaultPrevented = true;
      },
    });

    assert.ok(defaultPrevented);
    assert.ok(tabs[1].classList.contains('active'));
    assert.strictEqual(tabs[1].getAttribute('aria-selected'), 'true');
    assert.strictEqual(global.document.activeElement, tabs[1]);

    // ArrowLeft from 1 -> 0 (RBAC)
    defaultPrevented = false;
    await tabs[1].dispatchEvent({
      type: 'keydown',
      key: 'ArrowLeft',
      preventDefault: () => {
        defaultPrevented = true;
      },
    });

    assert.ok(defaultPrevented);
    assert.ok(tabs[0].classList.contains('active'));
    assert.strictEqual(tabs[0].getAttribute('aria-selected'), 'true');
  });

  test('Home and End keys jump to first and last simulator tabs', async () => {
    // End key jumps to last tab (fetch)
    await tabs[0].dispatchEvent({
      type: 'keydown',
      key: 'End',
      preventDefault: () => {},
    });
    assert.ok(tabs[4].classList.contains('active'));
    assert.strictEqual(tabs[4].getAttribute('aria-selected'), 'true');

    // Home key jumps back to first tab (rbac)
    await tabs[4].dispatchEvent({
      type: 'keydown',
      key: 'Home',
      preventDefault: () => {},
    });
    assert.ok(tabs[0].classList.contains('active'));
    assert.strictEqual(tabs[0].getAttribute('aria-selected'), 'true');
  });
});

describe('RBAC Access Control Simulator Logic (#284)', () => {
  const USER = 'GDZRQ4BDEZLU4S7PZ7M3B5YDJHKGKUI27QPBKNVD3MWXWGA3KU7E7RKO';
  let app;
  let form, actionSelect, btn, preview;
  let roleGroup, permissionGroup, durationGroup, durSlider, durVal;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();
    form = getOrCreate('rbac-form', 'form');
    actionSelect = getOrCreate('rbac-action', 'select');
    actionSelect.value = 'get_role';
    getOrCreate('input-patient-addr', 'input').value = USER;
    getOrCreate('rbac-role', 'select').value = '3';
    getOrCreate('rbac-permission', 'select').value = '2';
    roleGroup = getOrCreate('rbac-role-group');
    permissionGroup = getOrCreate('rbac-permission-group');
    durationGroup = getOrCreate('rbac-duration-group');
    durSlider = getOrCreate('dur-slider', 'input');
    durSlider.value = '24';
    durVal = getOrCreate('dur-val', 'span');
    btn = getOrCreate('btn-grant-access', 'button');
    form.appendChild(btn);
    preview = getOrCreate('preview-rbac-code', 'pre');

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    app.initRBACSimulator();
  });

  const submit = () => form.dispatchEvent({ type: 'submit', preventDefault: () => {} });

  test('shows only the inputs the selected contract call needs', async () => {
    assert.ok(roleGroup.hidden && permissionGroup.hidden && durationGroup.hidden);

    actionSelect.value = 'has_permission';
    await actionSelect.dispatchEvent({ type: 'change' });
    assert.ok(!permissionGroup.hidden && roleGroup.hidden && durationGroup.hidden);

    actionSelect.value = 'assign_role';
    await actionSelect.dispatchEvent({ type: 'change' });
    assert.ok(permissionGroup.hidden && !roleGroup.hidden && !durationGroup.hidden);
    assert.match(preview.textContent, /contracts\/access_control::assign_role/);
  });

  test('slider input updates duration display text, ARIA attributes and the call preview', async () => {
    actionSelect.value = 'assign_role';
    await actionSelect.dispatchEvent({ type: 'change' });
    durSlider.value = '48';
    await durSlider.dispatchEvent({ type: 'input', target: durSlider });

    assert.strictEqual(durVal.textContent, '48');
    assert.strictEqual(durSlider.getAttribute('aria-valuenow'), '48');
    assert.strictEqual(durSlider.getAttribute('aria-valuetext'), '48 Hours');
    assert.match(preview.textContent, /role: Optometrist \(3\), expires_at: now \+ 172800 s/);
  });

  test('form submission simulates access grant and updates code console', async () => {
    let prevented = false;
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {
        prevented = true;
      },
    });
  test('get_role reads the role from the contract and shows a loading state meanwhile', async () => {
    let finish;
    const calls = [];
    app.AccessControlClient.getRole = (network, user) => {
      calls.push([network, user]);
      return new Promise((resolve) => (finish = () => resolve({ contractId: 'CAC', role: 3 })));
    };

    const done = submit();
    assert.strictEqual(btn.disabled, true);
    assert.strictEqual(preview.getAttribute('aria-busy'), 'true');
    assert.match(preview.textContent, /Status: Pending/);

    finish();
    await done;
    assert.deepStrictEqual(calls, [['testnet', USER]]);
    assert.strictEqual(btn.disabled, false);
    assert.strictEqual(preview.getAttribute('aria-busy'), null);
    assert.match(preview.textContent, /contracts\/access_control::get_role/);
    assert.match(preview.textContent, /Result: Optometrist \(3\)/);
  });

  test('has_permission passes the selected permission and shows the answer', async () => {
    const calls = [];
    app.AccessControlClient.hasPermission = async (network, user, permission) => {
      calls.push([network, user, permission]);
      return { contractId: 'CAC', granted: true };
    };
    actionSelect.value = 'has_permission';
    await actionSelect.dispatchEvent({ type: 'change' });

    await submit();
    assert.deepStrictEqual(calls, [['testnet', USER, 2]]);
    assert.match(preview.textContent, /permission: WriteRecord \(2\)/);
    assert.match(preview.textContent, /Result: true/);
  });

  test('a failed assign_role shows the decoded contract error', async () => {
    const feedback = [];
    global.window.showErrorFeedback = (message) => feedback.push(message);
    app.AccessControlClient.assignRole = async () => {
      throw new Error('HostError: Error(Contract, #1)\n\nEvent log (newest first): ...');
    };
    actionSelect.value = 'assign_role';
    await actionSelect.dispatchEvent({ type: 'change' });

    await submit();
    assert.match(preview.textContent, /Status: Failed/);
    assert.match(
      preview.textContent,
      /access_control error Unauthorized \(Error\(Contract, #1\)\)/
    );
    assert.doesNotMatch(preview.textContent, /Event log/);
    assert.strictEqual(feedback.length, 1);
    assert.match(feedback[0], /Contract call failed: access_control error Unauthorized/);
    assert.strictEqual(btn.disabled, false);
  });
});

describe('ZK Proof Generator Simulator Logic (#295)', () => {
  let app;
  let acuitySlider, acuityVal, form, btn, preview;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();
    form = getOrCreate('zk-form', 'form');
    acuitySlider = getOrCreate('acuity-slider', 'input');
    acuitySlider.value = '20';
    acuityVal = getOrCreate('acuity-val', 'span');
    btn = getOrCreate('btn-gen-zk', 'button');
    form.appendChild(btn);
    preview = getOrCreate('preview-zk-code', 'pre');

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    speedUpRPC(app);
    app.initZKSimulator();
  });

  test('visual acuity slider changes update displayed ratio', async () => {
    acuitySlider.value = '30';
    await acuitySlider.dispatchEvent({ type: 'input', target: acuitySlider });

    assert.strictEqual(acuityVal.textContent, '20/30');
    assert.strictEqual(acuitySlider.getAttribute('aria-valuenow'), '30');
    assert.match(preview.textContent, /"patient_private_acuity": "20\/30"/);
    assert.match(preview.textContent, /"evaluation_result": "PASS_VERIFIED"/);
  });

  test('form submission generates groth16 proof with verified status', async () => {
    acuitySlider.value = '20';
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 40));
    assert.match(preview.textContent, /contracts\/zk_verifier::visual_acuity_proof/);
    assert.match(preview.textContent, /ONCHAIN_VERIFIED/);
    assert.match(preview.textContent, /PASS_VERIFIED/);
  });

  test('failing visual acuity (> 20/40) flags evaluation failure', async () => {
    acuitySlider.value = '50';
    await acuitySlider.dispatchEvent({ type: 'input', target: acuitySlider });
    assert.match(preview.textContent, /"evaluation_result": "FAIL_REQUIREMENT_NOT_MET"/);
  });
});

describe('AI Oracle Rotation Simulator Logic (#295)', () => {
  let app;
  let statusSelect, form, btn, preview;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();
    form = getOrCreate('ai-form', 'form');
    statusSelect = getOrCreate('select-ai-status', 'select');
    statusSelect.value = 'healthy';
    btn = getOrCreate('btn-test-ai', 'button');
    form.appendChild(btn);
    preview = getOrCreate('preview-ai-code', 'pre');

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    speedUpRPC(app);
    app.initAISimulator();
  });

  test('healthy oracle execution verifies primary provider and diagnostics', async () => {
    statusSelect.value = 'healthy';
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 40));
    assert.match(preview.textContent, /Primary Provider Active: "AI_Vision_Alpha"/);
    assert.match(preview.textContent, /No Diabetic Retinopathy Detected/);
  });

  test('timeout triggers provider rotation failover to secondary oracle', async () => {
    statusSelect.value = 'timeout';
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 40));
    assert.match(preview.textContent, /FAILOVER_TRIGGERED/);
    assert.match(preview.textContent, /Activated Secondary Oracle: "AI_Vision_Beta"/);
    assert.match(preview.textContent, /ProviderRotated/);
  });
});

describe('FHIR v4 Converter Simulator Logic (#295)', () => {
  let app;
  let fhirTypeSelect, form, btn, preview;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();
    form = getOrCreate('fhir-form', 'form');
    fhirTypeSelect = getOrCreate('select-fhir-type', 'select');
    fhirTypeSelect.value = 'refraction';
    btn = getOrCreate('btn-convert-fhir', 'button');
    form.appendChild(btn);
    preview = getOrCreate('preview-fhir-code', 'pre');

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    speedUpRPC(app);
    app.initFHIRSimulator();
  });

  test('converts refraction records to FHIR DiagnosticReport resource', async () => {
    fhirTypeSelect.value = 'refraction';
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 40));
    assert.match(preview.textContent, /"resourceType": "DiagnosticReport"/);
    assert.match(preview.textContent, /"code": "28886-0"/);
    assert.match(preview.textContent, /Refraction examination/);
  });

  test('converts IOP tonometry to FHIR Observation resource', async () => {
    fhirTypeSelect.value = 'iop';
    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 40));
    assert.match(preview.textContent, /"resourceType": "Observation"/);
    assert.match(preview.textContent, /"code": "56844-4"/);
    assert.match(preview.textContent, /Intraocular pressure by Tonometry/);
    assert.match(preview.textContent, /"unit": "mmHg"/);
  });
});

describe('Clinical Data Fetch Simulator Logic (#295)', () => {
  let app;
  let form, btn, idleEl, placeholderEl, resultEl, statusLabel, statusIndicator;

  beforeEach(() => {
    const { getOrCreate } = setupMockDOM();
    form = getOrCreate('fetch-form', 'form');
    btn = getOrCreate('btn-fetch-data', 'button');
    form.appendChild(btn);
    idleEl = getOrCreate('fetch-idle', 'div');
    placeholderEl = getOrCreate('fetch-placeholder', 'div');
    resultEl = getOrCreate('fetch-result-container', 'div');
    statusLabel = getOrCreate('fetch-status-label', 'span');
    statusIndicator = getOrCreate('fetch-status-indicator', 'span');

    delete require.cache[require.resolve('./app.js')];
    app = require('./app.js');
    speedUpRPC(app);
    app.initDataFetchSimulator();
  });

  test('data fetch transitions from placeholder to completed decrypted result', async () => {
    const submitPromise = form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    // Check placeholder during fetch
    assert.strictEqual(statusLabel.textContent, 'DATA_RETRIEVAL IN_PROGRESS');

    await submitPromise;
    await new Promise((resolve) => setTimeout(resolve, 40));

    assert.strictEqual(placeholderEl.style.display, 'none');
    assert.strictEqual(resultEl.style.display, 'block');
    assert.strictEqual(statusLabel.textContent, 'DATA_RETRIEVAL SUCCESS');
    assert.strictEqual(statusIndicator.textContent, 'DECRYPTED_PAYLOAD');
  });
});

describe('Simulator Error States & Resilience (#295)', () => {
  test('recovers gracefully and displays error feedback on failed SorobanRPC invocation', async () => {
    const { getOrCreate } = setupMockDOM();
    const form = getOrCreate('zk-form-err', 'form');
    const btn = getOrCreate('btn-gen-zk', 'button');
    form.appendChild(btn);
    getOrCreate('acuity-slider', 'input').value = '20';

    let errorFeedbackMsg = null;
    global.window.showErrorFeedback = (msg) => {
      errorFeedbackMsg = msg;
    };

    delete require.cache[require.resolve('./app.js')];
    const app = require('./app.js');

    // Force SorobanRPC.invoke to fail
    const originalInvoke = app.SorobanRPC.invoke;
    app.SorobanRPC.invoke = async () => {
      throw new Error('Simulated network timeout');
    };

    app.initZKSimulator();

    await form.dispatchEvent({
      type: 'submit',
      preventDefault: () => {},
    });

    await new Promise((resolve) => setTimeout(resolve, 30));
    assert.ok(errorFeedbackMsg, 'Error feedback should be triggered');
    assert.match(errorFeedbackMsg, /Simulated network timeout/);
    assert.strictEqual(btn.textContent, 'Generate Groth16 zk-SNARK');

    // Restore
    app.SorobanRPC.invoke = originalInvoke;
  });
});
