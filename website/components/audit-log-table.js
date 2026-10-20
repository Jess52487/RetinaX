/* ==========================================================================
   RetinaX — Audit Log Table (audit-log-table.js)
   Filterable, paginated table for the audit trail from the audit contract.

   Entries use the audit contract's AuditLogEntry shape:
     { sequence, timestamp, actor, action, target, result }

   The component does not fetch data itself. Set the `entries` property to render them:
     document.querySelector('audit-log-table').entries = entries;
   ========================================================================== */

(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.AuditLogTable = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};
  const DEFAULT_PAGE_SIZE = 10;
  // Ledger timestamps are in seconds. Larger values are treated as milliseconds.
  const MILLISECONDS_THRESHOLD = 1e12;
  const DAY_SECONDS = 86400;

  function toSeconds(timestamp) {
    const value = Number(timestamp);
    if (!Number.isFinite(value) || value < 0) return null;
    return value >= MILLISECONDS_THRESHOLD ? value / 1000 : value;
  }

  // Turns a YYYY-MM-DD value from a date input into the first and last second of that UTC day.
  function dayBounds(value) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(String(value || ''))) return null;
    const start = Date.parse(`${value}T00:00:00Z`) / 1000;
    if (Number.isNaN(start)) return null;
    return { start, end: start + DAY_SECONDS - 1 };
  }

  /**
   * Formats a ledger timestamp as "YYYY-MM-DD HH:MM:SS UTC". Returns "-" for invalid input.
   * @param {number|string} timestamp Seconds or milliseconds since the Unix epoch.
   * @returns {string}
   */
  function formatTimestamp(timestamp) {
    const seconds = toSeconds(timestamp);
    if (seconds === null) return '-';
    const date = new Date(seconds * 1000);
    if (Number.isNaN(date.getTime())) return '-';
    return `${date.toISOString().slice(0, 19).replace('T', ' ')} UTC`;
  }

  /**
   * Filters entries by a free-text search on action or actor, and by an inclusive UTC date range.
   * Results are sorted newest first by sequence number.
   * @param {Array<object>} entries
   * @param {{search?: string, from?: string, to?: string}} [options] `from` and `to` are
   *   YYYY-MM-DD strings. When a date bound is set, entries without a valid timestamp are excluded.
   * @returns {Array<object>}
   */
  function filterEntries(entries, options) {
    const { search = '', from = '', to = '' } = options || {};
    const query = String(search).trim().toLowerCase();
    const fromBounds = dayBounds(from);
    const toBounds = dayBounds(to);
    const list = Array.isArray(entries) ? entries.filter(Boolean) : [];

    return list
      .filter((entry) => {
        const seconds = toSeconds(entry.timestamp);
        if (fromBounds && (seconds === null || seconds < fromBounds.start)) return false;
        if (toBounds && (seconds === null || seconds > toBounds.end)) return false;
        if (!query) return true;
        const fields = [entry.action, entry.actor];
        return fields.some((field) =>
          String(field ?? '')
            .toLowerCase()
            .includes(query)
        );
      })
      .sort((a, b) => (Number(b.sequence) || 0) - (Number(a.sequence) || 0));
  }

  /**
   * Returns one page of items. The requested page is clamped to the valid range.
   * @param {Array<object>} items
   * @param {number} page 1-based page number.
   * @param {number} pageSize Items per page.
   * @returns {{items: Array<object>, page: number, pageCount: number, total: number}}
   */
  function paginate(items, page, pageSize) {
    const size = Math.max(1, Math.floor(pageSize) || DEFAULT_PAGE_SIZE);
    const list = Array.isArray(items) ? items : [];
    const pageCount = Math.max(1, Math.ceil(list.length / size));
    const current = Math.min(Math.max(1, Math.floor(page) || 1), pageCount);
    const start = (current - 1) * size;
    return {
      items: list.slice(start, start + size),
      page: current,
      pageCount,
      total: list.length,
    };
  }

  class AuditLogTable extends ElementBase {
    constructor() {
      super();
      this._entries = [];
      this._filters = { search: '', from: '', to: '' };
      this._page = 1;
      this._body = null;
    }

    get entries() {
      return this._entries;
    }

    set entries(value) {
      this._entries = Array.isArray(value) ? value : [];
      this._page = 1;
      this.refresh();
    }

    connectedCallback() {
      if (!this._body) this.buildShell();
      this.refresh();
    }

    // Builds the controls once. Only the rows, summary, and pager are re-rendered afterwards,
    // so the search field keeps focus while the visitor types.
    buildShell() {
      this.innerHTML =
        '<div class="audit-log-table">' +
        '<form class="audit-log-table__filters" role="search">' +
        '<label>Search action or actor' +
        '<input type="search" class="audit-log-table__search" placeholder="e.g. grant">' +
        '</label>' +
        '<label>From<input type="date" class="audit-log-table__from"></label>' +
        '<label>To<input type="date" class="audit-log-table__to"></label>' +
        '</form>' +
        '<p class="audit-log-table__summary" aria-live="polite"></p>' +
        '<div class="audit-log-table__scroll"><table>' +
        '<caption>Audit trail</caption>' +
        '<thead><tr>' +
        '<th scope="col">Timestamp</th><th scope="col">Action</th>' +
        '<th scope="col">Actor</th><th scope="col">Target</th>' +
        '</tr></thead>' +
        '<tbody class="audit-log-table__body"></tbody>' +
        '</table></div>' +
        '<nav class="audit-log-table__pager" aria-label="Audit log pages">' +
        '<button type="button" class="audit-log-table__prev">Previous</button>' +
        '<span class="audit-log-table__page-info" aria-live="polite"></span>' +
        '<button type="button" class="audit-log-table__next">Next</button>' +
        '</nav>' +
        '</div>';

      this._body = this.querySelector('.audit-log-table__body');
      this._summary = this.querySelector('.audit-log-table__summary');
      this._pageInfo = this.querySelector('.audit-log-table__page-info');
      this._prev = this.querySelector('.audit-log-table__prev');
      this._next = this.querySelector('.audit-log-table__next');

      // Filters apply live, so Enter should not submit the form and reload the page.
      this.querySelector('.audit-log-table__filters').addEventListener('submit', (event) => {
        event.preventDefault();
      });
      this.querySelector('.audit-log-table__search').addEventListener('input', (event) => {
        this.setFilter('search', event.target.value);
      });
      this.querySelector('.audit-log-table__from').addEventListener('input', (event) => {
        this.setFilter('from', event.target.value);
      });
      this.querySelector('.audit-log-table__to').addEventListener('input', (event) => {
        this.setFilter('to', event.target.value);
      });
      this._prev.addEventListener('click', () => this.changePage(-1));
      this._next.addEventListener('click', () => this.changePage(1));
    }

    setFilter(name, value) {
      this._filters[name] = value;
      this._page = 1;
      this.refresh();
    }

    changePage(delta) {
      this._page += delta;
      this.refresh();
    }

    refresh() {
      if (!this._body) return;
      const filtered = filterEntries(this._entries, this._filters);
      const view = paginate(filtered, this._page, DEFAULT_PAGE_SIZE);
      this._page = view.page;

      this._body.textContent = '';
      if (view.items.length === 0) {
        this._body.appendChild(this.buildEmptyRow());
      } else {
        view.items.forEach((entry) => this._body.appendChild(this.buildRow(entry)));
      }

      if (view.total === 0) {
        this._summary.textContent = 'No entries match these filters.';
      } else {
        this._summary.textContent = `${view.total} ${view.total === 1 ? 'entry' : 'entries'}`;
      }
      this._pageInfo.textContent = `Page ${view.page} of ${view.pageCount}`;
      this._prev.disabled = view.page <= 1;
      this._next.disabled = view.page >= view.pageCount;
    }

    buildRow(entry) {
      const row = document.createElement('tr');
      const cells = [
        ['Timestamp', formatTimestamp(entry.timestamp)],
        ['Action', entry.action],
        ['Actor', entry.actor],
        ['Target', entry.target],
      ];
      cells.forEach(([label, value]) => {
        const cell = document.createElement('td');
        cell.setAttribute('data-label', label);
        // textContent keeps audit data as plain text, never as markup.
        cell.textContent = value === undefined || value === null ? '' : String(value);
        row.appendChild(cell);
      });
      return row;
    }

    buildEmptyRow() {
      const row = document.createElement('tr');
      const cell = document.createElement('td');
      cell.colSpan = 4;
      cell.className = 'audit-log-table__empty';
      cell.textContent = 'No audit entries to show.';
      row.appendChild(cell);
      return row;
    }
  }

  if (typeof customElements !== 'undefined' && !customElements.get('audit-log-table'))
    customElements.define('audit-log-table', AuditLogTable);

  return { AuditLogTable, filterEntries, paginate, formatTimestamp, DEFAULT_PAGE_SIZE };
});
