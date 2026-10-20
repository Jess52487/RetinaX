const test = require('node:test');
const assert = require('node:assert/strict');
const { createRateLimiter, createKeyedRateLimiter } = require('./prove-rate-limit');

test('limits proof requests within a sliding window and allows recovery', () => {
  let now = 0;
  const allow = createRateLimiter({ maxRequests: 2, windowMs: 1000, clock: { now: () => now } });
  assert.equal(allow(), true);
  assert.equal(allow(), true);
  assert.equal(allow(), false);
  now = 1000;
  assert.equal(allow(), true);
});

test('keeps proof quotas separate for different request keys', () => {
  const allow = createKeyedRateLimiter({ maxRequests: 1, windowMs: 1000, clock: { now: () => 0 } });
  assert.equal(allow('wallet-a'), true);
  assert.equal(allow('wallet-a'), false);
  assert.equal(allow('wallet-b'), true);
});
