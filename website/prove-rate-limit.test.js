const { test } = require('node:test');
const assert = require('node:assert');
const { createRateLimiter } = require('./prove-rate-limit');

test('limits proof requests within a sliding window and allows recovery', () => {
  let now = 0;
  const allow = createRateLimiter({ maxRequests: 2, windowMs: 1000, clock: { now: () => now } });
  assert.strictEqual(allow(), true);
  assert.strictEqual(allow(), true);
  assert.strictEqual(allow(), false);
  now = 1000;
  assert.strictEqual(allow(), true);
});
