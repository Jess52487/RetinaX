// Unit tests for the pure helpers in audit-log-table.js (run with `node --test`).
// The custom element itself needs a DOM, so only the data logic is exercised here.

const test = require('node:test');
const assert = require('node:assert/strict');
const {
  filterEntries,
  paginate,
  formatTimestamp,
  DEFAULT_PAGE_SIZE,
} = require('./audit-log-table.js');

// 1700000000 seconds is 2023-11-14 22:13:20 UTC.
const NOV_14_2023 = 1700000000;
const NOV_13_2023 = NOV_14_2023 - 86400;
const NOV_15_2023 = NOV_14_2023 + 86400;

function entry(sequence, overrides = {}) {
  return {
    sequence,
    timestamp: NOV_14_2023,
    actor: 'GACTOR',
    action: 'log_event',
    target: 'record:1',
    result: 'ok',
    ...overrides,
  };
}

function seqs(list) {
  return list.map((e) => e.sequence);
}

test('formatTimestamp renders seconds and milliseconds the same way', () => {
  assert.equal(formatTimestamp(NOV_14_2023), '2023-11-14 22:13:20 UTC');
  assert.equal(formatTimestamp(NOV_14_2023 * 1000), '2023-11-14 22:13:20 UTC');
});

test('formatTimestamp returns a placeholder for invalid input', () => {
  assert.equal(formatTimestamp(undefined), '-');
  assert.equal(formatTimestamp('not-a-number'), '-');
  assert.equal(formatTimestamp(-5), '-');
});

test('filterEntries sorts newest first by sequence', () => {
  const result = filterEntries([entry(1), entry(3), entry(2)]);
  assert.deepEqual(seqs(result), [3, 2, 1]);
});

test('filterEntries searches action and actor case-insensitively', () => {
  const entries = [
    entry(1, { action: 'grant_access', actor: 'GALICE' }),
    entry(2, { action: 'revoke_access', actor: 'GBOB' }),
    entry(3, { action: 'log_event', actor: 'GALICIA_ADMIN' }),
  ];
  assert.deepEqual(seqs(filterEntries(entries, { search: 'ACCESS' })), [2, 1]);
  assert.deepEqual(seqs(filterEntries(entries, { search: ' gali ' })), [3, 1]);
  assert.equal(filterEntries(entries, { search: 'missing' }).length, 0);
});

test('filterEntries applies an inclusive UTC date range', () => {
  const entries = [
    entry(1, { timestamp: NOV_13_2023 }),
    entry(2, { timestamp: NOV_14_2023 }),
    entry(3, { timestamp: NOV_15_2023 }),
  ];
  const sameDay = filterEntries(entries, { from: '2023-11-14', to: '2023-11-14' });
  assert.deepEqual(seqs(sameDay), [2]);
  assert.deepEqual(seqs(filterEntries(entries, { from: '2023-11-14' })), [3, 2]);
  assert.deepEqual(seqs(filterEntries(entries, { to: '2023-11-14' })), [2, 1]);
});

test('filterEntries excludes entries without a valid timestamp when a date is set', () => {
  const entries = [entry(1, { timestamp: 'bad' }), entry(2)];
  assert.deepEqual(seqs(filterEntries(entries, { from: '2023-11-14' })), [2]);
  assert.deepEqual(seqs(filterEntries(entries)), [2, 1]);
});

test('filterEntries ignores malformed date strings instead of throwing', () => {
  const entries = [entry(1)];
  assert.equal(filterEntries(entries, { from: '14/11/2023' }).length, 1);
  assert.equal(filterEntries(entries, { to: '2023-13-45' }).length, 1);
});

test('filterEntries tolerates null entries and non-array input', () => {
  assert.deepEqual(seqs(filterEntries([null, entry(1)])), [1]);
  assert.deepEqual(filterEntries(undefined), []);
});

test('paginate splits items into pages of the default size', () => {
  const items = Array.from({ length: 25 }, (_, i) => i + 1);
  const first = paginate(items, 1, DEFAULT_PAGE_SIZE);
  assert.equal(first.items.length, 10);
  assert.equal(first.pageCount, 3);
  assert.equal(first.total, 25);

  const last = paginate(items, 3, DEFAULT_PAGE_SIZE);
  assert.deepEqual(last.items, [21, 22, 23, 24, 25]);
});

test('paginate clamps out-of-range page numbers', () => {
  const items = Array.from({ length: 25 }, (_, i) => i + 1);
  assert.equal(paginate(items, 99, 10).page, 3);
  assert.equal(paginate(items, 0, 10).page, 1);
  assert.equal(paginate(items, Number.NaN, 10).page, 1);
});

test('paginate returns one empty page when there are no items', () => {
  const view = paginate([], 1, 10);
  assert.deepEqual(view, { items: [], page: 1, pageCount: 1, total: 0 });
});
