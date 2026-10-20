// Unit tests for the pure helpers in permissions-toggle.js (run with `node --test`).
// The custom element itself needs a DOM, so only the state logic is exercised here.

const test = require('node:test');
const assert = require('node:assert/strict');
const { nextState } = require('./permissions-toggle.js');

test('nextState flips the switch when enabled', () => {
  assert.equal(nextState(false, false), true);
  assert.equal(nextState(true, false), false);
});

test('nextState keeps the current value when disabled', () => {
  assert.equal(nextState(false, true), false);
  assert.equal(nextState(true, true), true);
});
