const { test, describe } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

describe('WCAG 2.1 Accessibility Compliance (#289)', () => {
  const htmlPath = path.join(__dirname, 'index.html');
  const cssPath = path.join(__dirname, 'styles.css');
  const html = fs.readFileSync(htmlPath, 'utf8');
  const css = fs.readFileSync(cssPath, 'utf8');

  test('index.html contains skip-to-content link pointing to main landmark', () => {
    assert.match(html, /<a\s+href="#main-content"\s+class="skip-link">Skip to main content<\/a>/i);
    assert.match(html, /<main\s+id="main-content">/i);
    assert.match(html, /<\/main>/i);
  });

  test('interactive simulator tabs implement proper WAI-ARIA tab and tabpanel roles', () => {
    assert.match(html, /role="tablist"/i);
    assert.match(html, /id="tab-rbac"[^>]*role="tab"[^>]*aria-controls="panel-rbac"/i);
    assert.match(html, /id="panel-rbac"[^>]*role="tabpanel"[^>]*aria-labelledby="tab-rbac"/i);

    assert.match(html, /id="tab-zk"[^>]*role="tab"[^>]*aria-controls="panel-zk"/i);
    assert.match(html, /id="panel-zk"[^>]*role="tabpanel"[^>]*aria-labelledby="tab-zk"/i);

    assert.match(html, /id="tab-ai"[^>]*role="tab"[^>]*aria-controls="panel-ai"/i);
    assert.match(html, /id="panel-ai"[^>]*role="tabpanel"[^>]*aria-labelledby="tab-ai"/i);

    assert.match(html, /id="tab-fhir"[^>]*role="tab"[^>]*aria-controls="panel-fhir"/i);
    assert.match(html, /id="panel-fhir"[^>]*role="tabpanel"[^>]*aria-labelledby="tab-fhir"/i);

    assert.match(html, /id="tab-fetch"[^>]*role="tab"[^>]*aria-controls="panel-fetch"/i);
    assert.match(html, /id="panel-fetch"[^>]*role="tabpanel"[^>]*aria-labelledby="tab-fetch"/i);
  });

  test('all form inputs have associated labels and ARIA descriptors', () => {
    assert.match(html, /<label[^>]*for="input-patient-addr"/i);
    assert.match(html, /<label[^>]*for="rbac-action"/i);
    assert.match(html, /<label[^>]*for="rbac-permission"/i);
    assert.match(html, /<label[^>]*for="rbac-role"/i);
    assert.match(html, /<label[^>]*for="wallet-network-select"/i);
    assert.match(html, /<label[^>]*for="dur-slider"/i);
    assert.match(html, /<label[^>]*for="acuity-slider"/i);
    assert.match(html, /<label[^>]*for="select-ai-status"/i);
    assert.match(html, /<label[^>]*for="select-fhir-type"/i);
  });

  test('heading hierarchy avoids arbitrary skipping in footer', () => {
    assert.match(html, /<nav class="footer-col"[^>]*>\s*<h3>Protocol<\/h3>/i);
    assert.match(html, /<nav class="footer-col"[^>]*>\s*<h3>Ecosystem<\/h3>/i);
    assert.match(html, /<nav class="footer-col"[^>]*>\s*<h3>Open Source<\/h3>/i);
  });

  test('styles.css contains visible focus rings and prefers-reduced-motion', () => {
    assert.match(css, /:focus-visible/i);
    assert.match(css, /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/i);
  });
});
