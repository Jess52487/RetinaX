/* ==========================================================================
   RetinaX i18n loader
   --------------------------------------------------------------------------
   - Loads locale catalogs from website/i18n/<language>.json
   - Falls back to English for any missing key
   - Persists the selected language in localStorage
   - Applies translations through data-i18n* attributes
   - Switches the document direction for RTL languages (scaffold)
   ========================================================================== */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.RetinaXI18n = factory();
  }
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  var DEFAULT_LANGUAGE = 'en';
  var STORAGE_KEY = 'retinax-language';
  var SUPPORTED_LANGUAGES = [
    { code: 'en', label: 'English', dir: 'ltr' },
    { code: 'es', label: 'Español', dir: 'ltr' },
  ];
  var RTL_LANGUAGES = ['ar', 'he', 'fa', 'ur'];
  var ATTRIBUTE_BINDINGS = {
    'data-i18n-aria-label': 'aria-label',
    'data-i18n-title': 'title',
    'data-i18n-placeholder': 'placeholder',
    'data-i18n-alt': 'alt',
    'data-i18n-value': 'value',
    'data-i18n-label': 'label',
    'data-i18n-content': 'content',
    'data-i18n-message': 'message',
  };

  var catalogs = new Map();
  var currentLanguage = DEFAULT_LANGUAGE;
  var currentDirection = 'ltr';
  var initialized = false;
  var selectorBound = false;
  var warnedKeys = new Set();
  var scriptBaseUrl = null;

  function hasDocument() {
    return typeof document !== 'undefined';
  }

  if (hasDocument() && document.currentScript && document.currentScript.src) {
    try {
      scriptBaseUrl = new URL('./', document.currentScript.src);
    } catch (_) {
      scriptBaseUrl = null;
    }
  }

  function lookup(catalog, key) {
    if (!catalog || !key) return undefined;
    var parts = String(key).split('.');
    var value = catalog;
    for (var i = 0; i < parts.length; i += 1) {
      if (value === null || value === undefined || typeof value !== 'object') {
        return undefined;
      }
      value = value[parts[i]];
    }
    return value;
  }

  function interpolate(template, vars) {
    if (typeof template !== 'string' || !template) return template;
    if (!vars || typeof vars !== 'object') return template;
    return template.replace(/\{(\w+)\}/g, function (match, name) {
      return Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : match;
    });
  }

  function resolveTranslation(catalog, fallbackCatalog, key, vars) {
    var value = lookup(catalog, key);
    if (value === undefined || value === null || value === '') {
      value = lookup(fallbackCatalog, key);
    }
    if (value === undefined || value === null || value === '') {
      if (!warnedKeys.has(key) && typeof console !== 'undefined' && console.warn) {
        warnedKeys.add(key);
        console.warn('[i18n] Missing translation key: ' + key);
      }
      return key;
    }
    return interpolate(String(value), vars);
  }

  function normalizeLanguage(language) {
    var code = String(language || '')
      .trim()
      .toLowerCase();
    if (!code && typeof navigator !== 'undefined' && navigator.language) {
      code = String(navigator.language).toLowerCase();
    }
    code = code.split('-')[0];
    for (var i = 0; i < SUPPORTED_LANGUAGES.length; i += 1) {
      if (SUPPORTED_LANGUAGES[i].code === code) return code;
    }
    return DEFAULT_LANGUAGE;
  }

  function isRtlLanguage(language) {
    return (
      RTL_LANGUAGES.indexOf(
        String(language || '')
          .toLowerCase()
          .split('-')[0]
      ) !== -1
    );
  }

  function directionFor(language, meta) {
    if (meta && (meta.direction === 'rtl' || meta.direction === 'ltr')) {
      return meta.direction;
    }
    return isRtlLanguage(language) ? 'rtl' : 'ltr';
  }

  function getCatalogBase() {
    if (scriptBaseUrl) return scriptBaseUrl;
    if (hasDocument() && document.baseURI) {
      return new URL('./i18n/', document.baseURI);
    }
    return null;
  }

  async function fetchCatalog(language) {
    if (catalogs.has(language)) return catalogs.get(language);
    var base = getCatalogBase();
    if (!base) {
      catalogs.set(language, null);
      return null;
    }
    try {
      var response = await fetch(new URL(language + '.json', base).toString(), {
        credentials: 'same-origin',
      });
      if (!response.ok) throw new Error('HTTP ' + response.status);
      var catalog = await response.json();
      catalogs.set(language, catalog);
      return catalog;
    } catch (error) {
      if (typeof console !== 'undefined' && console.warn) {
        console.warn('[i18n] Unable to load "' + language + '" catalog:', error);
      }
      catalogs.set(language, null);
      return null;
    }
  }

  async function ensureCatalog(language) {
    var catalog = await fetchCatalog(language);
    return catalog || null;
  }

  function getInitialLanguage() {
    if (hasDocument()) {
      try {
        var url = new URL(window.location.href);
        var fromQuery = url.searchParams.get('lang');
        if (fromQuery) return normalizeLanguage(fromQuery);
      } catch (_) {
        /* ignore malformed URLs */
      }
      try {
        var stored = window.localStorage && window.localStorage.getItem(STORAGE_KEY);
        if (stored) return normalizeLanguage(stored);
      } catch (_) {
        /* localStorage can be unavailable in private mode */
      }
    }
    return normalizeLanguage();
  }

  function t(key, vars, fallbackText) {
    var catalog = catalogs.get(currentLanguage);
    var fallbackCatalog = catalogs.get(DEFAULT_LANGUAGE);
    var value = resolveTranslation(catalog, fallbackCatalog, key, vars);
    if (value === key && fallbackText !== undefined) return fallbackText;
    return value;
  }

  function translateAttributeBindings(element) {
    Object.keys(ATTRIBUTE_BINDINGS).forEach(function (dataAttribute) {
      var key = element.getAttribute && element.getAttribute(dataAttribute);
      if (!key) return;
      element.setAttribute(ATTRIBUTE_BINDINGS[dataAttribute], t(key));
    });

    var compound = element.getAttribute && element.getAttribute('data-i18n-attr');
    if (compound) {
      compound.split(',').forEach(function (pair) {
        var parts = pair.split(':');
        if (parts.length !== 2) return;
        var attribute = parts[0].trim();
        var key = parts[1].trim();
        if (attribute && key) element.setAttribute(attribute, t(key));
      });
    }
  }

  function applyTranslations(scope) {
    if (!hasDocument()) return;
    if (!catalogs.get(DEFAULT_LANGUAGE)) return;
    var root = scope || document;

    var titleKey = 'meta.title';
    if (
      lookup(catalogs.get(currentLanguage), titleKey) ||
      lookup(catalogs.get(DEFAULT_LANGUAGE), titleKey)
    ) {
      document.title = t(titleKey);
    }
    var descriptionKey = 'meta.description';
    var descriptionMeta = document.querySelector('meta[name="description"]');
    if (descriptionMeta) {
      var description = t(descriptionKey, {}, '');
      if (description) descriptionMeta.setAttribute('content', description);
    }

    var textNodes = root.querySelectorAll ? root.querySelectorAll('[data-i18n]') : [];
    Array.prototype.forEach.call(textNodes, function (element) {
      element.textContent = t(element.getAttribute('data-i18n'));
    });

    var attributeNodes = root.querySelectorAll
      ? root.querySelectorAll(
          '[data-i18n-aria-label],[data-i18n-title],[data-i18n-placeholder],[data-i18n-alt],[data-i18n-value],[data-i18n-label],[data-i18n-content],[data-i18n-message],[data-i18n-attr]'
        )
      : [];
    Array.prototype.forEach.call(attributeNodes, translateAttributeBindings);
  }

  function updateDocumentLanguage() {
    if (!hasDocument()) return;
    var meta = lookup(catalogs.get(currentLanguage), '_meta') || {};
    currentDirection = directionFor(currentLanguage, meta);
    var html = document.documentElement;
    if (html) {
      html.setAttribute('lang', currentLanguage);
      html.setAttribute('dir', currentDirection);
      html.setAttribute('data-language', currentLanguage);
    }
    if (document.body) {
      document.body.setAttribute('dir', currentDirection);
      document.body.setAttribute('data-language', currentLanguage);
    }
  }

  function syncLanguageSelector() {
    if (!hasDocument()) return;
    var selector = document.getElementById('language-selector');
    if (!selector) return;

    if (selector.options.length !== SUPPORTED_LANGUAGES.length) {
      selector.textContent = '';
      SUPPORTED_LANGUAGES.forEach(function (language) {
        var option = document.createElement('option');
        option.value = language.code;
        option.textContent = language.label;
        option.setAttribute('lang', language.code);
        selector.appendChild(option);
      });
    }
    selector.value = currentLanguage;
    selector.setAttribute('aria-label', t('nav.languageSelector', {}, 'Select language'));
  }

  function bindLanguageSelector() {
    if (!hasDocument() || selectorBound) return;
    var selector = document.getElementById('language-selector');
    if (!selector) return;
    selector.addEventListener('change', function (event) {
      setLanguage(event.target.value);
    });
    selectorBound = true;
  }

  function emitChange() {
    if (!hasDocument() || typeof document.dispatchEvent !== 'function') return;
    var detail = {
      language: currentLanguage,
      direction: currentDirection,
      meta: lookup(catalogs.get(currentLanguage), '_meta') || {},
    };
    document.dispatchEvent(new CustomEvent('retinax:languagechange', { detail: detail }));
  }

  async function setLanguage(language, options) {
    var settings = options || {};
    var code = normalizeLanguage(language);
    await ensureCatalog(DEFAULT_LANGUAGE);
    if (code !== DEFAULT_LANGUAGE) await ensureCatalog(code);
    if (!catalogs.get(code)) code = DEFAULT_LANGUAGE;

    currentLanguage = code;
    var meta = lookup(catalogs.get(code), '_meta') || {};
    currentDirection = directionFor(code, meta);

    if (settings.persist !== false && hasDocument()) {
      try {
        if (window.localStorage) window.localStorage.setItem(STORAGE_KEY, code);
      } catch (_) {
        /* ignore storage failures */
      }
    }

    updateDocumentLanguage();
    applyTranslations();
    syncLanguageSelector();
    emitChange();
    return currentLanguage;
  }

  async function init() {
    if (initialized) {
      applyTranslations();
      syncLanguageSelector();
      return currentLanguage;
    }
    initialized = true;
    await setLanguage(getInitialLanguage(), { persist: false });
    bindLanguageSelector();
    return currentLanguage;
  }

  function getLanguage() {
    return currentLanguage;
  }

  function getDirection() {
    return currentDirection;
  }

  if (hasDocument() && typeof document.addEventListener === 'function') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', function () {
        init().catch(function (error) {
          if (typeof console !== 'undefined' && console.warn) {
            console.warn('[i18n] Initialization failed:', error);
          }
        });
      });
    } else {
      init().catch(function (error) {
        if (typeof console !== 'undefined' && console.warn) {
          console.warn('[i18n] Initialization failed:', error);
        }
      });
    }
  }

  return {
    DEFAULT_LANGUAGE: DEFAULT_LANGUAGE,
    RTL_LANGUAGES: RTL_LANGUAGES,
    SUPPORTED_LANGUAGES: SUPPORTED_LANGUAGES,
    lookup: lookup,
    interpolate: interpolate,
    resolveTranslation: resolveTranslation,
    normalizeLanguage: normalizeLanguage,
    isRtlLanguage: isRtlLanguage,
    directionFor: directionFor,
    t: t,
    init: init,
    setLanguage: setLanguage,
    getLanguage: getLanguage,
    getDirection: getDirection,
    applyTranslations: applyTranslations,
  };
});
