// Functional tests for examination-viewer.js (run with: node --test)
const test = require('node:test');
const assert = require('node:assert/strict');

const ExaminationViewer = require('./examination-viewer.js');

const sampleExam = {
  visual_acuity: {
    uncorrected: { left_eye: '20/40', right_eye: '20/25' },
    corrected: { left_eye: '20/20', right_eye: '20/20' },
  },
  iop: { left_eye: 16, right_eye: 22, method: 'Goldmann', timestamp: 1760000000 },
  slit_lamp: {
    cornea: 'Clear',
    anterior_chamber: 'Deep and quiet',
    iris: 'Normal',
    lens: 'Clear',
  },
  clinical_notes: 'Follow-up in six months.',
};

test('classifyIop marks values against the normal range', () => {
  assert.equal(ExaminationViewer.classifyIop(9).level, 'low');
  assert.equal(ExaminationViewer.classifyIop(10).level, 'normal');
  assert.equal(ExaminationViewer.classifyIop(21).level, 'normal');
  assert.equal(ExaminationViewer.classifyIop(22).level, 'high');
});

test('classifyIop reports missing or zero readings as not recorded', () => {
  assert.equal(ExaminationViewer.classifyIop(0).level, 'unknown');
  assert.equal(ExaminationViewer.classifyIop(null).level, 'unknown');
  assert.equal(ExaminationViewer.classifyIop('abc').level, 'unknown');
});

test('iopBarPercent clamps to the 0–100 scale', () => {
  assert.equal(ExaminationViewer.iopBarPercent(0), 0);
  assert.equal(ExaminationViewer.iopBarPercent(20), 50);
  assert.equal(ExaminationViewer.iopBarPercent(80), 100);
});

test('escHtml escapes markup-significant characters', () => {
  assert.equal(
    ExaminationViewer.escHtml('<img src=x onerror="a">&\''),
    '&lt;img src=x onerror=&quot;a&quot;&gt;&amp;&#39;'
  );
  assert.equal(ExaminationViewer.escHtml(undefined), '');
});

test('renderExamination shows left and right eye values and both acuity columns', () => {
  const html = ExaminationViewer.renderExamination(sampleExam);
  assert.match(html, /20\/40/);
  assert.match(html, /20\/25/);
  assert.match(html, /Uncorrected/);
  assert.match(html, /Corrected/);
  assert.match(html, /16 mmHg/);
  assert.match(html, /22 mmHg/);
  assert.match(html, /examination-viewer__badge--high/);
  assert.match(html, /Follow-up in six months\./);
});

test('renderExamination shows a dash when corrected acuity is absent', () => {
  const exam = {
    ...sampleExam,
    visual_acuity: { uncorrected: { left_eye: '20/30', right_eye: '20/30' }, corrected: null },
  };
  const html = ExaminationViewer.renderExamination(exam);
  assert.match(html, /data-label="Corrected">—</);
});

test('renderExamination omits sections that have no data', () => {
  const html = ExaminationViewer.renderExamination({ iop: sampleExam.iop });
  assert.match(html, /Intraocular pressure/);
  assert.doesNotMatch(html, /Visual acuity/);
  assert.doesNotMatch(html, /Slit lamp/);
  assert.doesNotMatch(html, /Clinical notes/);
});

test('renderExamination escapes clinical notes and handles empty input', () => {
  const html = ExaminationViewer.renderExamination({
    clinical_notes: '<script>alert(1)</script>',
  });
  assert.doesNotMatch(html, /<script>/);
  assert.match(html, /&lt;script&gt;/);

  assert.match(ExaminationViewer.renderExamination(null), /No examination data/);
});
