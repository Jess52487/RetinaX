# Website Developer Guide

This guide covers the RetinaX marketing and playground site in [`website/`](../website). It is a
plain HTML/CSS/JavaScript site with **no build step and no framework**: the files in `website/`
are served exactly as they are, both locally and on Vercel.

- [Local development setup](#local-development-setup)
- [Project layout](#project-layout)
- [Dependencies](#dependencies)
- [Component architecture](#component-architecture)
- [Creating a component](#creating-a-component)
- [Testing approach](#testing-approach)
- [Linting and formatting](#linting-and-formatting)
- [Internationalization](#internationalization)
- [Pull request checklist](#pull-request-checklist)

## Local development setup

### Prerequisites

- **Node.js 20 or later** (`engines.node` in `website/package.json`; CI uses Node 20). Node is only
  needed for tests, linting and formatting, not to run the site.
- Any static file server. The examples below use Python or `npx serve`, but anything that serves a
  directory works.

### Install tooling

```bash
cd website
npm ci          # installs the dev-only tooling (ESLint, Prettier) from package-lock.json
```

### Run the site

The site must be served over HTTP. Opening `index.html` directly with `file://` breaks the i18n
loader, which uses `fetch()` to load `i18n/<language>.json`.

```bash
# From the repository root, pick one:
python3 -m http.server 8080 --directory website
npx serve website
vercel dev       # uses vercel.json (outputDirectory: website) and its security headers
```

Then open <http://localhost:8080> (or the port your server prints). There is no hot reload; refresh
the browser after editing a file.

### Contract configuration

`index.html` defines `window.RETINAX_CONTRACTS` with empty contract IDs for `audit`, `identity`,
`key_manager` and `vision_records`. To point the playground at deployed testnet contracts, fill in
those IDs, or define `window.RETINAX_CONTRACTS` in a script that runs before that block. Values set
earlier take precedence because the inline script merges into any existing object.

### Common commands

Run these from `website/`:

| Command                | What it does                                           |
| ---------------------- | ------------------------------------------------------ |
| `npm test`             | Runs every `*.test.js` file with `node --test`         |
| `npm run lint`         | ESLint using the root `eslint.config.mjs`              |
| `npm run format:check` | Prettier check using the root `.prettierrc.json`       |
| `npm run format`       | Rewrites `**/*.js` with Prettier                       |

## Project layout

```text
website/
├── index.html            # Single page: markup, CDN tags, and the <script> load order
├── 404.html              # Not-found page served by Vercel
├── styles.css            # Global tokens, layout and section styles
├── app.js                # Page bootstrap: AOS init, navigation, simulators, modal, theme toggle
├── utils.js              # window.RetinaXUtils: date, FHIR, address and number formatting
├── prove-rate-limit.js   # Sliding-window rate limiter for proof requests (not yet loaded by index.html)
├── components/           # One .js and/or .css file per component, plus its .test.js
├── i18n/
│   ├── loader.js         # window.RetinaXI18n: catalog loading and data-i18n* binding
│   ├── en.json           # English catalog (source of truth and fallback)
│   └── es.json           # Spanish catalog
├── assets/               # Images and logo (served with a one-year immutable cache header)
├── *.test.js             # Page-level tests (simulators, routing, accessibility, i18n)
└── package.json          # Scripts and dev dependencies only
```

Deployment is configured in the root [`vercel.json`](../vercel.json): `outputDirectory` is
`website`, and `buildCommand`, `installCommand` and `framework` are all `null`, so nothing is
compiled on deploy.

## Dependencies

The site has no runtime npm dependencies. Browser libraries come from CDNs in `index.html`, and npm
installs only development tooling.

### Runtime (loaded from CDN in `index.html`)

| Library                                                         | Loaded from                                         | Purpose                                                                                                                                                                                                                                  |
| --------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [AOS](https://michalsnik.github.io/aos/) (Animate On Scroll)    | `unpkg.com/aos@next` (CSS in `<head>`, JS at end of `<body>`) | Scroll-triggered entrance animations. Elements opt in with `data-aos="fade-up"`, `data-aos-delay`, `data-aos-duration`. Initialised once in `app.js` with `duration: 800`, `easing: 'ease-out-cubic'`, `once: true`, `offset: 100`. |
| [GSAP](https://gsap.com/) 3.12.5                                | `cdnjs.cloudflare.com` (in `<head>`)                | Script-driven animation: card hover lift, result reveals in the simulators (`gsap.fromTo`), and the hero timeline in `initGSAPAnimations()` (kept for opt-in use; AOS handles the current page).                                         |
| GSAP ScrollTrigger 3.12.5                                       | `cdnjs.cloudflare.com` (in `<head>`)                | GSAP plugin for scroll-linked timelines. It is loaded but no script registers it yet; use it for new scroll-driven GSAP animations instead of adding another scroll library.                                                                                                                                                      |
| Google Fonts: Fraunces, Plus Jakarta Sans, Inter, JetBrains Mono | `fonts.googleapis.com`                              | Editorial serif headings, sans-serif UI text, and monospace for addresses, hashes and code.                                                                                                                                             |

Both animation libraries are optional at runtime. Code must check before using them
(`if (typeof gsap === 'undefined') return;`, `if (typeof AOS !== 'undefined') { ... }`) so the page
still works if a CDN is blocked and so modules can be `require`d in Node tests. `AOS` and `gsap` are
declared as read-only globals in `eslint.config.mjs`.

### Development (`website/package.json`)

| Package    | Version | Purpose                                                               |
| ---------- | ------- | --------------------------------------------------------------------- |
| `eslint`   | 9.11.1  | Linting, configured by the flat config at the repository root         |
| `prettier` | 3.3.3   | Formatting: 100-column lines, single quotes, ES5 trailing commas, LF  |

The test runner is Node's built-in `node:test` module, so there is no test framework dependency.

Before adding a new runtime library, check whether a few lines of plain JavaScript would do. If you
do add one, load it from a CDN with an exact pinned version, guard its use with a `typeof` check,
add its global to `eslint.config.mjs`, and add it to the table above.

## Component architecture

### Script loading

There is no bundler or module loader. Every script is a classic `<script>` tag in `index.html`, so
**load order matters**:

1. GSAP and ScrollTrigger (in `<head>`)
2. AOS, then `utils.js` (exposes `window.RetinaXUtils`, used by components and `app.js`)
3. Inline `window.RETINAX_CONTRACTS` configuration
4. Components: `repo-avatar.js`, `contract-address-display.js`, `i18n/loader.js`,
   `contract-grid.js`, `accordion.js`, `connect-wallet.js`, `global-spinner.js`,
   `error-feedback.js`
5. `app.js`, which wires everything together on `DOMContentLoaded`

A component that depends on another script must be included after it.

### The UMD wrapper

Every reusable script uses the same Universal Module Definition wrapper, so one file works both as a
browser global and as a CommonJS module for tests:

```js
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.EmptyState = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';
  // ...
  return { EmptyState, textOrDefault };
});
```

In the browser the factory's return value becomes `window.EmptyState`; under Node it becomes
`module.exports`. `app.js` uses a guarded `if (typeof module !== 'undefined' && module.exports)`
block at the bottom to export its functions for tests.

### Two component styles

**Custom elements** for self-contained widgets that own their markup, for example
[`empty-state.js`](../website/components/empty-state.js),
[`permissions-toggle.js`](../website/components/permissions-toggle.js) and
[`patient-card.js`](../website/components/patient-card.js). They extend `HTMLElement`, read
configuration from attributes, re-render in `attributeChangedCallback`, and report actions to the
host page with bubbling `CustomEvent`s rather than calling page code directly.

**Enhancers** for markup that already lives in `index.html`, for example
[`accordion.js`](../website/components/accordion.js). They find elements by a data attribute
(`[data-accordion]`) and attach behaviour on `DOMContentLoaded`, or immediately if the document has
already loaded.

### Styling

- Each component has its own stylesheet in `components/` (for example `empty-state.css`), linked
  from `index.html`. `styles.css` holds global design tokens and section layout.
- Class names follow BEM: `.empty-state`, `.empty-state__heading`, `.empty-state--compact`.
- Use the CSS custom properties from `styles.css` (for example `var(--text-muted)`) instead of
  hard-coded colours so the theme toggle keeps working.
- Animate with CSS transitions where possible. The accordion, for instance, animates height with a
  CSS grid transition rather than JavaScript.

### Accessibility

`accessibility.test.js` checks the page structure, and every component is expected to:

- use semantic elements (`<button type="button">`, headings in order) before reaching for ARIA;
- follow the matching WAI-ARIA pattern (the accordion toggles `aria-expanded` on its trigger);
- mark decorative SVG and icons with `aria-hidden="true"`;
- announce dynamic content with `role="status"` or `aria-live` regions;
- stay keyboard operable, including closing overlays with Escape.

## Creating a component

Use [`empty-state.js`](../website/components/empty-state.js) as the reference for a custom element.

### 1. Write the component

Create `website/components/my-widget.js`:

```js
/* ==========================================================================
   RetinaX — My Widget (my-widget.js)
   One-paragraph description of what the widget shows and when to use it.

   Usage:
     <my-widget label="Visual acuity"></my-widget>
   ========================================================================== */

(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.MyWidget = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  // Lets the file load under Node, where HTMLElement does not exist.
  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};

  /**
   * Pure helper: no DOM access, so it can be unit tested directly.
   * @param {string|null|undefined} value
   * @returns {string}
   */
  function normaliseLabel(value) {
    return String(value ?? '').trim() || 'Untitled';
  }

  class MyWidget extends ElementBase {
    static get observedAttributes() {
      return ['label'];
    }

    connectedCallback() {
      this.render();
    }

    attributeChangedCallback(name, oldValue, newValue) {
      if (oldValue === newValue || !this.isConnected) return;
      this.render();
    }

    render() {
      this.textContent = '';
      const wrapper = document.createElement('div');
      wrapper.className = 'my-widget';
      wrapper.textContent = normaliseLabel(this.getAttribute('label'));
      this.appendChild(wrapper);
    }
  }

  if (typeof customElements !== 'undefined' && !customElements.get('my-widget'))
    customElements.define('my-widget', MyWidget);

  return { MyWidget, normaliseLabel };
});
```

Conventions to keep:

- **Put logic in pure functions** and export them. Tests run in Node without a DOM, so anything
  worth testing should not need one.
- **Guard browser globals** (`HTMLElement`, `customElements`, `document`, `gsap`, `AOS`) with
  `typeof` checks so `require()` works in Node.
- **Guard `customElements.define`** with `customElements.get(...)` so loading the script twice does
  not throw.
- **Build DOM with `createElement` and `textContent`.** If you must interpolate user or chain data
  into HTML, escape it with `RetinaXUtils.escapeHtml`.
- **Communicate outward with events**, for example
  `this.dispatchEvent(new CustomEvent('my-widget-change', { bubbles: true, composed: true }))`.
- **Format dates, addresses and numbers with `RetinaXUtils`** instead of writing new formatters.

### 2. Add styles

Create `website/components/my-widget.css` with BEM classes and theme variables.

### 3. Register it in `index.html`

```html
<!-- in <head>, with the other component stylesheets -->
<link rel="stylesheet" href="components/my-widget.css">

<!-- before app.js, after any script it depends on -->
<script src="components/my-widget.js"></script>
```

### 4. Translate user-facing text

Add any new strings to `i18n/en.json` and `i18n/es.json`, and bind them with `data-i18n*`
attributes (see [Internationalization](#internationalization)).

### 5. Add tests

Create `website/components/my-widget.test.js` next to the component (see below).

## Testing approach

Tests use Node's built-in test runner, **`node --test`**, with `node:assert/strict`. There is no
Jest, Mocha or browser test runner.

```bash
cd website
npm test                                         # every *.test.js under website/
node --test components/empty-state.test.js       # a single file
node --test --test-name-pattern="textOrDefault"  # tests whose name matches
```

`node --test` with no arguments discovers `*.test.js` files recursively, so a new test file is picked
up as soon as it exists. CI runs the same command in `.github/workflows/frontend-lint.yml` after the
Prettier and ESLint checks, on every push or pull request that touches `website/**`.

### What to test and how

There is no DOM in Node, so tests fall into three kinds:

1. **Pure helper tests (preferred).** `require` the component and test the functions it exports.

   ```js
   // Unit tests for the pure helpers in my-widget.js (run with `node --test`).
   // The custom element itself needs a DOM, so only the text logic is exercised here.

   const test = require('node:test');
   const assert = require('node:assert/strict');
   const { normaliseLabel } = require('./my-widget.js');

   test('normaliseLabel trims the attribute text', () => {
     assert.equal(normaliseLabel('  Visual acuity  '), 'Visual acuity');
   });

   test('normaliseLabel falls back when the label is blank', () => {
     assert.equal(normaliseLabel('   '), 'Untitled');
   });
   ```

2. **Hand-written DOM mocks** when behaviour depends on elements. `simulators.test.js` defines small
   `MockClassList` and element classes that implement only the methods the code under test calls.
   Keep mocks minimal and local to the test file.

3. **Static markup checks** that read `index.html` with `fs.readFileSync` and assert on its
   structure. `accessibility.test.js` and `i18n.test.js` use this to check landmarks, ARIA
   attributes, and that every `data-i18n` key exists in each catalog.

When you fix a bug, add a test that fails without the fix. When you add a translation key, the i18n
tests will fail until it is present in every catalog.

## Linting and formatting

- ESLint uses the flat config in the repository root, [`eslint.config.mjs`](../eslint.config.mjs).
  It declares browser globals, the UMD globals (`module`, `require`) and the CDN globals (`AOS`,
  `gsap`), with a looser rule set for `*.test.js`. If you introduce a new global, declare it there
  rather than disabling the rule inline.
- Prettier uses [`.prettierrc.json`](../.prettierrc.json) at the root: `printWidth: 100`,
  `singleQuote: true`, `trailingComma: "es5"`, `endOfLine: "lf"`.

Run `npm run format && npm run lint && npm test` from `website/` before pushing. CI fails on any
formatting difference.

## Internationalization

[`i18n/loader.js`](../website/i18n/loader.js) loads `i18n/<language>.json`, falls back to English
for missing keys, remembers the choice in `localStorage`, and switches `dir` for RTL languages.

Bind text with attributes instead of hard-coding it in JavaScript:

| Attribute                 | Sets                     |
| ------------------------- | ------------------------ |
| `data-i18n`               | element text             |
| `data-i18n-aria-label`    | `aria-label`             |
| `data-i18n-title`         | `title`                  |
| `data-i18n-placeholder`   | `placeholder`            |
| `data-i18n-alt`           | `alt`                    |
| `data-i18n-content`       | `content` (meta tags)    |

Keys are dot paths into the catalog, such as `nav.overview`. Add every new key to `en.json` and to
each other catalog in the same pull request.

## Pull request checklist

- [ ] Runs from a static server with no build step and no console errors
- [ ] New scripts use the UMD wrapper and guard browser globals
- [ ] Logic is in exported pure functions with `node --test` coverage
- [ ] Styles live in `components/<name>.css` and use theme variables
- [ ] Keyboard and screen reader behaviour follow the relevant WAI-ARIA pattern
- [ ] New strings exist in every `i18n/*.json` catalog
- [ ] `npm run format:check`, `npm run lint` and `npm test` pass in `website/`
- [ ] Any new CDN dependency is pinned, guarded, declared in ESLint, and listed in this guide
