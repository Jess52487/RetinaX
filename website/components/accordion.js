/* ==========================================================================
   RetinaX — Accordion Component
   Accessible accordion with smooth CSS grid height transitions.
   Follows WAI-ARIA Accordion Pattern.
   ========================================================================== */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.Accordion = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  function initAccordion(container) {
    if (!container) return;

    var triggers = container.querySelectorAll('.accordion__trigger');

    triggers.forEach(function (trigger) {
      trigger.addEventListener('click', function () {
        var expanded = trigger.getAttribute('aria-expanded') === 'true';
        trigger.setAttribute('aria-expanded', String(!expanded));
      });
    });
  }

  function initAll() {
    if (typeof document === 'undefined') return;
    var accordions = document.querySelectorAll('[data-accordion]');
    accordions.forEach(initAccordion);
  }

  if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', initAll);
    } else {
      initAll();
    }
  }

  return { initAccordion: initAccordion, initAll: initAll };
});
