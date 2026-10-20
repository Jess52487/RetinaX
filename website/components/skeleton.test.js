/* ==========================================================================
   Loading Skeleton States & Transitions Tests (Issue #292)
   Runs with Node.js test runner: node --test website/components/skeleton.test.js
   ========================================================================== */

const { test, describe } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

// Shim minimal browser environment
global.window = global;
global.HTMLElement = function () {
  this.dataset = {};
  this.attributes = {};
  this.children = [];
  this.parentNode = null;
  this.innerHTML = '';
  this.textContent = '';
  this.className = '';
  this.classList = {
    add: (c) => {
      if (!this.className.includes(c)) this.className = (this.className + ' ' + c).trim();
    },
    remove: (c) => {
      this.className = this.className.replace(new RegExp('\\b' + c + '\\b', 'g'), '').trim();
    },
    contains: (c) => this.className.split(/\s+/).includes(c),
  };
  this.setAttribute = (k, v) => {
    this.attributes[k] = String(v);
  };
  this.getAttribute = (k) => (this.attributes[k] !== undefined ? this.attributes[k] : null);
  this.hasAttribute = (k) => this.attributes[k] !== undefined;
  this.removeAttribute = (k) => {
    delete this.attributes[k];
  };
  this.appendChild = (child) => {
    this.children.push(child);
    child.parentNode = this;
    return child;
  };
  this.removeChild = (child) => {
    this.children = this.children.filter((c) => c !== child);
    if (child.parentNode === this) child.parentNode = null;
    return child;
  };
  this.querySelector = (sel) => {
    if (sel === 'button') {
      return this.children.find((c) => c._tag === 'button') || null;
    }
    if (sel === '.patient-card-component') {
      return this.children.find((c) => c.className.includes('patient-card-component')) || null;
    }
    return null;
  };
  this.querySelectorAll = () => [];
  this.addEventListener = () => {};
  this.removeEventListener = () => {};
};

global.document = {
  readyState: 'complete',
  createElement: (tag) => {
    const el = new global.HTMLElement();
    el._tag = tag;
    return el;
  },
  querySelectorAll: () => [],
  addEventListener: () => {},
};

global.customElements = {
  registry: new Map(),
  define: function (name, cls) {
    this.registry.set(name, cls);
  },
  get: function (name) {
    return this.registry.get(name);
  },
};

const PatientCard = require('./patient-card.js');
const RepoAvatar = require('./repo-avatar.js');
const ContractAddressDisplay = require('./contract-address-display.js');
const connectWalletCode = fs.readFileSync(path.join(__dirname, 'connect-wallet.js'), 'utf8');

describe('Skeleton Stylesheet Compliance (#292)', () => {
  const cssPath = path.join(__dirname, 'skeleton.css');
  const css = fs.readFileSync(cssPath, 'utf8');

  test('skeleton.css defines linear-gradient shimmer keyframe animation', () => {
    assert.match(css, /@keyframes\s+skeleton-loading/i);
    assert.match(css, /background-position:\s*200%\s*0/i);
    assert.match(css, /background-position:\s*-200%\s*0/i);
  });

  test('skeleton.css defines smooth transition fade-in animation and utility', () => {
    assert.match(css, /@keyframes\s+skeleton-fade-in/i);
    assert.match(css, /\.skeleton-fade-in\s*\{/i);
  });

  test('skeleton.css supports both dark theme and light theme palette variables', () => {
    assert.match(css, /--skeleton-base/i);
    assert.match(css, /--skeleton-highlight/i);
    assert.match(css, /\[data-theme="light"\]/i);
  });

  test('skeleton.css contains geometric primitives matching component shapes', () => {
    assert.match(css, /\.skeleton-wrapper/i);
    assert.match(css, /\.skeleton-line/i);
    assert.match(css, /\.skeleton-title/i);
    assert.match(css, /\.skeleton-short/i);
    assert.match(css, /\.skeleton-circle/i);
    assert.match(css, /\.skeleton-pill/i);
  });

  test('skeleton.css includes PatientCard and RepoAvatar specialized skeleton classes', () => {
    assert.match(css, /\.pc-skel-circle/i);
    assert.match(css, /\.pc-skel-title/i);
    assert.match(css, /\.pc-skel-line/i);
    assert.match(css, /\.ra-skel--avatar/i);
    assert.match(css, /\.ra-skel--title/i);
    assert.match(css, /\.ra-skel--line/i);
  });

  test('skeleton.css honors prefers-reduced-motion accessibility standard', () => {
    assert.match(css, /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/i);
    assert.match(css, /animation:\s*none/i);
  });
});

describe('PatientCard Loading Skeleton State (#292)', () => {
  test('showLoading() injects matching skeleton layout with avatar circle, title and badges', () => {
    const mount = new global.HTMLElement();
    const pc = new PatientCard(mount, { patientName: 'Jane Smith', age: 34 });

    pc.showLoading();

    assert.ok(
      mount.children[0].classList.contains('is-loading'),
      'Card should have is-loading class'
    );
    const html = mount.children[0].innerHTML;
    assert.match(html, /pc-skel-circle/, 'Contains circular avatar placeholder');
    assert.match(html, /pc-skel-title/, 'Contains title placeholder');
    assert.match(html, /pc-skel-line/, 'Contains multiline content placeholders');
    assert.match(html, /pc-skel-badge/, 'Contains badge placeholders');
  });

  test('update() smoothly replaces skeleton placeholder with real clinical data', () => {
    const mount = new global.HTMLElement();
    const pc = new PatientCard(mount, { patientName: 'Jane Smith', age: 34 });

    pc.showLoading();
    assert.ok(mount.children[0].classList.contains('is-loading'));

    pc.update({ patientName: 'Jane Smith', age: 35 });
    assert.ok(
      !mount.children[0].classList.contains('is-loading'),
      'is-loading removed after update'
    );
    assert.match(mount.children[0].innerHTML, /Jane Smith/);
  });
});

describe('RepoAvatar Loading Skeleton State (#292)', () => {
  test('showLoading() renders repository skeleton banner and avatar shapes', () => {
    const mount = new global.HTMLElement();
    const ra = new RepoAvatar(mount, { org: 'RetinaX-Org', repo: 'RetinaX' });

    ra.showLoading();

    assert.ok(mount.children[0].classList.contains('is-loading'));
    const html = mount.children[0].innerHTML;
    assert.match(html, /ra-skel--title/);
    assert.match(html, /ra-skel--avatar/);
    assert.match(html, /ra-skel--line/);
  });

  test('update() clears loading state and reveals repository details', () => {
    const mount = new global.HTMLElement();
    const ra = new RepoAvatar(mount, { org: 'RetinaX-Org', repo: 'RetinaX' });

    ra.showLoading();
    assert.ok(mount.children[0].classList.contains('is-loading'));

    ra.update({ stars: 1200 });
    assert.ok(!mount.children[0].classList.contains('is-loading'));
    assert.match(mount.children[0].innerHTML, /1\.2k/);
  });
});

describe('ContractAddressDisplay Loading Skeleton State (#292)', () => {
  test('renders skeleton pill with is-loading when loading attribute is present', () => {
    const el = new ContractAddressDisplay();
    el.setAttribute('loading', '');
    el.render();

    assert.match(el.innerHTML, /contract-address-display__skeleton/);
    assert.match(el.innerHTML, /is-loading/);
    assert.match(el.innerHTML, /skeleton-pill/);
    assert.match(el.innerHTML, /aria-busy="true"/);
  });

  test('renders full button with skeleton-fade-in when address is loaded', () => {
    const el = new ContractAddressDisplay();
    el.setAttribute('address', 'CVISIONREC77RETINAXOPHTHALMOLOGYEHRRECORDDATA000004');
    el.render();

    assert.match(el.innerHTML, /contract-address-display__button/);
    assert.match(el.innerHTML, /skeleton-fade-in/);
    assert.match(el.innerHTML, /CVISIO…000004/);
  });
});

describe('ConnectWalletButton Loading Skeleton State (#292)', () => {
  test('connect-wallet component code supports loading attribute and renders skeleton placeholder', () => {
    assert.match(connectWalletCode, /observedAttributes\(\)[^}]+'loading'/);
    assert.match(connectWalletCode, /hasAttribute\(['"]loading['"]\)/);
    assert.match(connectWalletCode, /skeleton-circle/);
    assert.match(connectWalletCode, /skeleton-line/);
    assert.match(connectWalletCode, /skeleton-fade-in/);
  });
});

describe('Data Fetch Simulator Loading Skeleton in index.html (#292)', () => {
  const htmlPath = path.join(__dirname, '..', 'index.html');
  const indexHtml = fs.readFileSync(htmlPath, 'utf8');

  test('index.html links components/skeleton.css', () => {
    assert.match(indexHtml, /<link[^>]*href="components\/skeleton\.css"/);
  });

  test('index.html contains #fetch-placeholder with .skeleton-wrapper and lines', () => {
    assert.match(indexHtml, /id="fetch-placeholder"[^>]*class="[^"]*skeleton-wrapper/);
    assert.match(indexHtml, /class="[^"]*skeleton-line skeleton-title/);
    assert.match(indexHtml, /class="[^"]*skeleton-line skeleton-short/);
  });
});
