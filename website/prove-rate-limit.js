(function () {
  'use strict';

  const WINDOW_MS = 60_000;
  const MAX_REQUESTS = 5;
  const allowByKey = createKeyedRateLimiter();

  function createRateLimiter({ maxRequests = MAX_REQUESTS, windowMs = WINDOW_MS, clock = Date } = {}) {
    const timestamps = [];
    return function allowRequest() {
      const now = clock.now();
      while (timestamps.length && now - timestamps[0] >= windowMs) timestamps.shift();
      if (timestamps.length >= maxRequests) return false;
      timestamps.push(now);
      return true;
    };
  }

  function createKeyedRateLimiter(options = {}) {
    const limiters = new Map();
    return function allowKey(key = 'anonymous') {
      if (!limiters.has(key)) limiters.set(key, createRateLimiter(options));
      return limiters.get(key)();
    };
  }

  function allow() {
    const key = document.querySelector('[data-wallet-address]')?.dataset.walletAddress || 'anonymous';
    return allowByKey(key);
  }

  if (typeof document !== 'undefined') document.addEventListener('submit', function (event) {
    const form = event.target;
    if (!form || !form.querySelector('#btn-gen-zk')) return;
    if (allow()) return;
    event.preventDefault();
    const message = document.getElementById('zk-rate-limit-message');
    if (message) {
      message.hidden = false;
      message.textContent = 'Too many proof requests. Please wait a minute before trying again.';
    }
  }, true);

  if (typeof module !== 'undefined') module.exports = { createRateLimiter, createKeyedRateLimiter };
})();
