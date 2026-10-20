const { createRateLimiter } = require('./prove-rate-limit');

test('limits proof requests within a sliding window and allows recovery', () => {
  let now = 0;
  const allow = createRateLimiter({ maxRequests: 2, windowMs: 1000, clock: { now: () => now } });
  expect(allow()).toBe(true);
  expect(allow()).toBe(true);
  expect(allow()).toBe(false);
  now = 1000;
  expect(allow()).toBe(true);
});
