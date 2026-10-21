/* ==========================================================================
   ExaminationViewer — Eye examination results component
   RetinaX Design System: Clinical White · Slate Navy · Medical Teal

   Usage (HTML element):
   ─────────────────────────────────────────────────────────────────────────
   <examination-viewer data-exam='{"visual_acuity": {...}, "iop": {...}}'></examination-viewer>

   Usage (JS property):
   ─────────────────────────────────────────────────────────────────────────
   const viewer = document.querySelector('examination-viewer');
   viewer.exam = {
     visual_acuity: {
       uncorrected: { left_eye: '20/40', right_eye: '20/25' },
       corrected:   { left_eye: '20/20', right_eye: '20/20' },   // or null
     },
     iop: { left_eye: 16, right_eye: 22, method: 'Goldmann', timestamp: 1760000000 },
     slit_lamp: { cornea: 'Clear', anterior_chamber: 'Deep and quiet',
                  iris: 'Normal', lens: 'Clear' },
     visual_field: null,                                        // optional
     clinical_notes: 'Follow-up in six months.',                // optional
   };
   ─────────────────────────────────────────────────────────────────────────
   Pure helpers (classifyIop, renderExamination, escHtml) are exposed as
   static members so they can be tested without a DOM.
   ========================================================================== */

(function (root, factory) {
  /* UMD export — works as ES module, CommonJS, or plain <script> */
  if (typeof define === 'function' && define.amd) {
    define([], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.ExaminationViewer = factory();
  }
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const ElementBase = typeof HTMLElement !== 'undefined' ? HTMLElement : function () {};

  /** Normal intraocular pressure range in mmHg (commonly cited clinical bounds). */
  const IOP_NORMAL_MIN = 10;
  const IOP_NORMAL_MAX = 21;
  /** Upper end of the visual scale used to size the IOP bars. */
  const IOP_SCALE_MAX = 40;

  /**
   * Escape a value for safe insertion into HTML.
   * @param {*} value
   * @returns {string}
   */
  function escHtml(value) {
    return String(value == null ? '' : value).replace(/[&<>"']/g, function (ch) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch];
    });
  }

  /**
   * Classify an IOP reading against the normal range.
   * @param {number|string} mmHg
   * @returns {{level: 'normal'|'low'|'high'|'unknown', label: string}}
   */
  function classifyIop(mmHg) {
    const value = Number(mmHg);
    if (!Number.isFinite(value) || value <= 0) {
      return { level: 'unknown', label: 'Not recorded' };
    }
    if (value < IOP_NORMAL_MIN) return { level: 'low', label: 'Below normal' };
    if (value <= IOP_NORMAL_MAX) return { level: 'normal', label: 'Normal' };
    return { level: 'high', label: 'Elevated' };
  }

  /**
   * Width (0–100) of the IOP bar for a reading, clamped to the scale.
   * @param {number|string} mmHg
   * @returns {number}
   */
  function iopBarPercent(mmHg) {
    const value = Number(mmHg);
    if (!Number.isFinite(value) || value <= 0) return 0;
    return Math.min(value / IOP_SCALE_MAX, 1) * 100;
  }

  /** Render a single value, showing an em dash when it is missing. */
  function valueOrDash(value) {
    return value === null || value === undefined || value === '' ? '—' : escHtml(value);
  }

  function renderIopRow(eye, mmHg) {
    const status = classifyIop(mmHg);
    const width = iopBarPercent(mmHg);
    return (
      '<div class="examination-viewer__iop-row">' +
      '<span class="examination-viewer__iop-eye">' + eye + '</span>' +
      '<span class="examination-viewer__iop-value">' +
      valueOrDash(Number(mmHg) > 0 ? mmHg : null) +
      (Number(mmHg) > 0 ? ' mmHg' : '') +
      '</span>' +
      '<div class="examination-viewer__iop-track" role="img" aria-label="' +
      escHtml(eye + ': ' + status.label) +
      '">' +
      '<div class="examination-viewer__iop-bar examination-viewer__iop-bar--' +
      status.level +
      '" style="width:' +
      width.toFixed(1) +
      '%"></div>' +
      '</div>' +
      '<span class="examination-viewer__badge examination-viewer__badge--' +
      status.level +
      '">' +
      escHtml(status.label) +
      '</span>' +
      '</div>'
    );
  }

  function renderIop(iop) {
    if (!iop) return '';
    return (
      '<section class="examination-viewer__section" aria-labelledby="examination-viewer-iop">' +
      '<h3 id="examination-viewer-iop" class="examination-viewer__heading">Intraocular pressure</h3>' +
      '<p class="examination-viewer__note">Normal range ' +
      IOP_NORMAL_MIN +
      '–' +
      IOP_NORMAL_MAX +
      ' mmHg' +
      (iop.method ? ' · Method: ' + escHtml(iop.method) : '') +
      '</p>' +
      renderIopRow('Left eye', iop.left_eye) +
      renderIopRow('Right eye', iop.right_eye) +
      '</section>'
    );
  }

  function renderAcuity(va) {
    if (!va) return '';
    const uncorrected = va.uncorrected || {};
    const corrected = va.corrected || null;
    const rows = [
      ['Left eye', uncorrected.left_eye, corrected && corrected.left_eye],
      ['Right eye', uncorrected.right_eye, corrected && corrected.right_eye],
    ];
    const body = rows
      .map(function (row) {
        return (
          '<tr><th scope="row">' +
          row[0] +
          '</th><td data-label="Uncorrected">' +
          valueOrDash(row[1]) +
          '</td><td data-label="Corrected">' +
          valueOrDash(row[2]) +
          '</td></tr>'
        );
      })
      .join('');
    return (
      '<section class="examination-viewer__section" aria-labelledby="examination-viewer-va">' +
      '<h3 id="examination-viewer-va" class="examination-viewer__heading">Visual acuity</h3>' +
      '<table class="examination-viewer__table">' +
      '<caption class="examination-viewer__sr-only">Visual acuity, uncorrected compared with corrected</caption>' +
      '<thead><tr><th scope="col">Eye</th><th scope="col">Uncorrected</th><th scope="col">Corrected</th></tr></thead>' +
      '<tbody>' +
      body +
      '</tbody></table>' +
      '</section>'
    );
  }

  function renderSlitLamp(slit) {
    if (!slit) return '';
    const fields = [
      ['Cornea', slit.cornea],
      ['Anterior chamber', slit.anterior_chamber],
      ['Iris', slit.iris],
      ['Lens', slit.lens],
    ];
    const items = fields
      .map(function (f) {
        return '<div><dt>' + f[0] + '</dt><dd>' + valueOrDash(f[1]) + '</dd></div>';
      })
      .join('');
    return (
      '<section class="examination-viewer__section" aria-labelledby="examination-viewer-slit">' +
      '<h3 id="examination-viewer-slit" class="examination-viewer__heading">Slit lamp</h3>' +
      '<dl class="examination-viewer__grid">' +
      items +
      '</dl></section>'
    );
  }

  /**
   * Build the full HTML for an examination record.
   * Sections without data are omitted so partial records still render.
   * @param {Object} exam
   * @returns {string}
   */
  function renderExamination(exam) {
    if (!exam || typeof exam !== 'object') {
      return '<p class="examination-viewer__empty">No examination data available.</p>';
    }
    const notes = exam.clinical_notes
      ? '<section class="examination-viewer__section"><h3 class="examination-viewer__heading">Clinical notes</h3>' +
        '<p class="examination-viewer__notes">' +
        escHtml(exam.clinical_notes) +
        '</p></section>'
      : '';
    return (
      '<div class="examination-viewer__inner">' +
      renderAcuity(exam.visual_acuity) +
      renderIop(exam.iop) +
      renderSlitLamp(exam.slit_lamp) +
      notes +
      '</div>'
    );
  }

  class ExaminationViewer extends ElementBase {
    static get observedAttributes() {
      return ['data-exam'];
    }

    constructor() {
      super();
      this._exam = null;
    }

    get exam() {
      return this._exam;
    }

    set exam(value) {
      this._exam = value;
      this.render();
    }

    connectedCallback() {
      if (!this._exam) this._readAttribute();
      this.render();
    }

    attributeChangedCallback() {
      if (!this.isConnected) return;
      this._readAttribute();
      this.render();
    }

    /** Parse the data-exam JSON attribute; invalid JSON renders the empty state. */
    _readAttribute() {
      const raw = this.getAttribute ? this.getAttribute('data-exam') : null;
      if (!raw) return;
      try {
        this._exam = JSON.parse(raw);
      } catch (_) {
        this._exam = null;
      }
    }

    render() {
      this.setAttribute && this.setAttribute('role', 'region');
      this.setAttribute && this.setAttribute('aria-label', 'Eye examination results');
      this.innerHTML = renderExamination(this._exam);
    }
  }

  ExaminationViewer.escHtml = escHtml;
  ExaminationViewer.classifyIop = classifyIop;
  ExaminationViewer.iopBarPercent = iopBarPercent;
  ExaminationViewer.renderExamination = renderExamination;
  ExaminationViewer.IOP_NORMAL_MIN = IOP_NORMAL_MIN;
  ExaminationViewer.IOP_NORMAL_MAX = IOP_NORMAL_MAX;

  if (typeof customElements !== 'undefined' && !customElements.get('examination-viewer'))
    customElements.define('examination-viewer', ExaminationViewer);

  return ExaminationViewer;
});
