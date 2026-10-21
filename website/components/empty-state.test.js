// Unit tests for the pure helpers in empty-state.js (run with `node --test`).
// The custom element itself needs a DOM, so only the text logic is exercised here.

const test = require('node:test');
const assert = require('node:assert/strict');
const { textOrDefault, DEFAULT_HEADING, DEFAULT_MESSAGE } = require('./empty-state.js');

test('textOrDefault returns trimmed attribute text', () => {
  assert.equal(textOrDefault('  No prescriptions  ', DEFAULT_HEADING), 'No prescriptions');
});

test('textOrDefault falls back when the attribute is missing or blank', () => {
  assert.equal(textOrDefault(null, DEFAULT_HEADING), DEFAULT_HEADING);
  assert.equal(textOrDefault(undefined, DEFAULT_MESSAGE), DEFAULT_MESSAGE);
  assert.equal(textOrDefault('   ', DEFAULT_MESSAGE), DEFAULT_MESSAGE);
});
