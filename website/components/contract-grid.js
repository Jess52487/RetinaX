/* ==========================================================================
   RetinaX — Contract Registry Grid
   Data-driven catalogue of every workspace contract with category filters,
   deployment status, testnet addresses, and GitHub source links.
   ========================================================================== */
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.ContractGrid = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  var GITHUB_BASE = 'https://github.com/RetinaX-Org/RetinaX';

  var CATEGORIES = [
    { id: 'all', labelKey: 'contracts.all', fallback: 'All' },
    { id: 'core', labelKey: 'contracts.core', fallback: 'Core' },
    { id: 'security', labelKey: 'contracts.security', fallback: 'Security' },
    { id: 'governance', labelKey: 'contracts.governance', fallback: 'Governance' },
    { id: 'clinical', labelKey: 'contracts.clinical', fallback: 'Clinical' },
    { id: 'infrastructure', labelKey: 'contracts.infrastructure', fallback: 'Infrastructure' },
  ];

  var CONTRACTS = [
    {
      id: 'vision_records',
      name: 'Vision Records',
      path: 'contracts/vision_records',
      category: 'core',
      description: 'Patient eye health history, consent, access grants, and clinical record links.',
      docsPath: 'docs/vision-records-architecture.md',
    },
    {
      id: 'ai_integration',
      name: 'AI Integration',
      path: 'contracts/ai_integration',
      category: 'core',
      description:
        'AI provider registry, SLA health monitoring, and automatic failover weight rotation.',
      docsPath: 'docs/ai_integration.md',
    },
    {
      id: 'analytics',
      name: 'Analytics',
      path: 'contracts/analytics',
      category: 'core',
      description:
        'Privacy-preserving aggregate metrics and trend analysis for clinical operations.',
      docsPath: 'docs/api/analytics.md',
    },
    {
      id: 'cross_chain',
      name: 'Cross-Chain Bridge',
      path: 'contracts/cross_chain',
      category: 'core',
      description:
        'Foreign-chain message validation with Merkle proofs and refund-safe settlement flows.',
      docsPath: 'docs/api/cross_chain.md',
    },
    {
      id: 'access_control',
      name: 'Access Control',
      path: 'contracts/access_control',
      category: 'security',
      description:
        'Role-based and attribute-based access control, delegations, ACL groups, and whitelists.',
      docsPath: 'docs/api/access_control.md',
    },
    {
      id: 'key_manager',
      name: 'Key Manager',
      path: 'contracts/key_manager',
      category: 'security',
      description:
        'Hierarchical deterministic key derivation, rotation, and cryptographic secret governance.',
      docsPath: 'docs/key-manager-architecture.md',
    },
    {
      id: 'identity',
      name: 'Identity & Enclave',
      path: 'contracts/identity',
      category: 'security',
      description:
        'Decentralized identity, guardian-based social recovery, and ZK credential verification.',
      docsPath: 'docs/api/identity.md',
    },
    {
      id: 'audit',
      name: 'Audit Trail',
      path: 'contracts/audit',
      category: 'security',
      description:
        'Tamper-evident, hash-chained audit logs for access events and compliance proofs.',
      docsPath: 'docs/api/audit.md',
    },
    {
      id: 'zk_verifier',
      name: 'ZK Verifier',
      path: 'contracts/zk_verifier',
      category: 'security',
      description: 'On-chain Groth16 zk-SNARK verification for anonymous eligibility proofs.',
      docsPath: 'docs/api/zk_verifier.md',
    },
    {
      id: 'compliance',
      name: 'Compliance',
      path: 'contracts/compliance',
      category: 'security',
      description:
        'GDPR and HIPAA policy enforcement, consent tracking, and clinical data classification.',
      docsPath: 'docs/api/compliance.md',
    },
    {
      id: 'governor',
      name: 'Governor',
      path: 'contracts/governor',
      category: 'governance',
      description: 'Proposals, commit-reveal voting, and governance power delegation.',
      docsPath: 'docs/api/governor.md',
    },
    {
      id: 'staking',
      name: 'Staking',
      path: 'contracts/staking',
      category: 'governance',
      description: 'Stake, request withdrawal, claim rewards, and tolerate rounding differences.',
      docsPath: 'docs/api/staking.md',
    },
    {
      id: 'delegation',
      name: 'Delegation',
      path: 'contracts/delegation',
      category: 'governance',
      description:
        'Delegated task queue with executor registration, assignment, and result submission.',
      docsPath: null,
    },
    {
      id: 'treasury',
      name: 'Treasury',
      path: 'contracts/treasury',
      category: 'governance',
      description: 'Asset limits, governance-approved spending, and DEX swap routing.',
      docsPath: 'docs/api/treasury.md',
    },
    {
      id: 'vault',
      name: 'Vault',
      path: 'contracts/vault',
      category: 'governance',
      description: 'Emergency key reconstruction, key rotation, and dead-man release controls.',
      docsPath: null,
    },
    {
      id: 'zk_voting',
      name: 'ZK Voting',
      path: 'contracts/zk_voting',
      category: 'governance',
      description: 'Anonymous ballots with Merkle roots, nullifiers, and tally verification.',
      docsPath: 'docs/api/zk_voting.md',
    },
    {
      id: 'clinical_records',
      name: 'Clinical Records',
      path: 'contracts/clinical_records',
      category: 'clinical',
      description:
        'Eye examinations, intraocular pressure, slit lamp findings, visual fields, imaging, and two-phase prescriptions.',
      docsPath: 'docs/api/clinical_records.md',
    },
    {
      id: 'fhir',
      name: 'FHIR Resources',
      path: 'contracts/fhir',
      category: 'clinical',
      description: 'FHIR v4 resource storage, patient validation, and version migration.',
      docsPath: 'docs/api/fhir.md',
    },
    {
      id: 'emr_bridge',
      name: 'EMR Bridge',
      path: 'contracts/emr_bridge',
      category: 'clinical',
      description: 'EMR provider registry and interoperable clinical data exchange records.',
      docsPath: 'docs/api/emr_bridge.md',
    },
    {
      id: 'common',
      name: 'Common Library',
      path: 'contracts/common',
      category: 'infrastructure',
      description:
        'Shared errors, admin tiers, progressive authorization, and reusable contract utilities.',
      docsPath: 'docs/common-library.md',
    },
    {
      id: 'events',
      name: 'Event Bus',
      path: 'contracts/events',
      category: 'infrastructure',
      description: 'Structured event envelopes with Lamport ordering and schema registration.',
      docsPath: null,
    },
    {
      id: 'orchestrator',
      name: 'Orchestrator',
      path: 'contracts/orchestrator',
      category: 'infrastructure',
      description: 'Atomic cross-contract transactions with timeout handling and rollback.',
      docsPath: null,
    },
    {
      id: 'state_channel',
      name: 'State Channel',
      path: 'contracts/state_channel',
      category: 'infrastructure',
      description: 'Payment channels with cooperative and unilateral close plus fraud proofs.',
      docsPath: null,
    },
    {
      id: 'metering',
      name: 'Metering',
      path: 'contracts/metering',
      category: 'infrastructure',
      description: 'Per-tenant resource consumption, quota enforcement, and fair cost allocation.',
      docsPath: 'docs/api/metering.md',
    },
  ];

  var STATUS_KEYS = {
    deployed: { key: 'contracts.statusDeployed', fallback: 'Deployed' },
    demo: { key: 'contracts.statusDemo', fallback: 'Demo address' },
    'not-deployed': { key: 'contracts.statusNotDeployed', fallback: 'Not deployed' },
  };

  function getI18n() {
    return typeof window !== 'undefined' ? window.RetinaXI18n : null;
  }

  function translate(key, fallback, vars) {
    var i18n = getI18n();
    if (i18n && typeof i18n.t === 'function') {
      var value = i18n.t(key, vars, fallback);
      if (value) return value;
    }
    return fallback;
  }

  function getDeploymentConfig() {
    if (typeof window === 'undefined') return {};
    return window.RETINAX_CONTRACTS && typeof window.RETINAX_CONTRACTS === 'object'
      ? window.RETINAX_CONTRACTS
      : {};
  }

  function getDemoRegistry() {
    if (typeof window === 'undefined') return {};
    if (window.RetinaX && window.RetinaX.DEFAULT_CONTRACT_REGISTRY) {
      return window.RetinaX.DEFAULT_CONTRACT_REGISTRY;
    }
    return {};
  }

  function resolveContractStatus(contract, config, demoRegistry) {
    var deploymentConfig = config || {};
    var demos = demoRegistry || {};
    var configuredAddress = deploymentConfig[contract.id] || contract.testnetAddress;
    if (configuredAddress) {
      return { status: 'deployed', address: String(configuredAddress) };
    }
    if (demos[contract.id]) {
      return { status: 'demo', address: String(demos[contract.id]) };
    }
    return { status: 'not-deployed', address: '' };
  }

  function filterContracts(contracts, category) {
    var list = contracts || CONTRACTS;
    if (!category || category === 'all') return list.slice();
    return list.filter(function (contract) {
      return contract.category === category;
    });
  }

  function categoryLabel(categoryId) {
    for (var i = 0; i < CATEGORIES.length; i += 1) {
      if (CATEGORIES[i].id === categoryId) {
        return translate(CATEGORIES[i].labelKey, CATEGORIES[i].fallback);
      }
    }
    return categoryId;
  }

  function contractName(contract) {
    return translate('contracts.items.' + contract.id + '.name', contract.name);
  }

  function contractDescription(contract) {
    return translate('contracts.items.' + contract.id + '.description', contract.description);
  }

  function statusLabel(status) {
    var definition = STATUS_KEYS[status] || STATUS_KEYS['not-deployed'];
    return translate(definition.key, definition.fallback);
  }

  function sourceUrl(contract) {
    return GITHUB_BASE + '/tree/main/' + contract.path;
  }

  function docsUrl(contract) {
    if (!contract.docsPath) return '';
    return GITHUB_BASE + '/blob/main/' + contract.docsPath;
  }

  function createElement(tag, className, text) {
    var element = document.createElement(tag);
    if (className) element.className = className;
    if (text !== undefined) element.textContent = text;
    return element;
  }

  function renderCard(contract, config, demoRegistry) {
    var status = resolveContractStatus(contract, config, demoRegistry);
    var displayStatus = status.status === 'deployed' ? 'deployed' : 'not-deployed';
    var card = createElement('article', 'contract-card contract-card--' + displayStatus);
    card.setAttribute('data-category', contract.category);
    card.setAttribute('data-contract', contract.id);

    var header = createElement('div', 'contract-card__header');
    header.appendChild(createElement('span', 'card-badge', contract.path));
    var badge = createElement(
      'span',
      'contract-status contract-status--' + displayStatus,
      statusLabel(displayStatus)
    );
    badge.setAttribute('data-status', displayStatus);
    header.appendChild(badge);
    card.appendChild(header);

    card.appendChild(createElement('h3', 'card-title', contractName(contract)));
    card.appendChild(createElement('p', 'card-desc', contractDescription(contract)));
    card.appendChild(
      createElement('span', 'contract-card__category', categoryLabel(contract.category))
    );

    var addressLabelKey =
      status.status === 'demo' ? 'contracts.demoAddress' : 'contracts.testnetAddress';
    var addressLabelFallback =
      status.status === 'demo' ? 'Demo address (not deployed)' : 'Testnet address';
    var addressBlock = createElement('div', 'contract-card__address');
    addressBlock.appendChild(
      createElement(
        'span',
        'contract-card__label',
        translate(addressLabelKey, addressLabelFallback)
      )
    );
    if (status.address) {
      var addressDisplay = document.createElement('contract-address-display');
      addressDisplay.setAttribute('address', status.address);
      addressBlock.appendChild(addressDisplay);
    } else {
      addressBlock.appendChild(
        createElement(
          'span',
          'contract-card__no-address',
          translate('contracts.noAddress', 'No testnet address configured')
        )
      );
    }
    card.appendChild(addressBlock);

    var footer = createElement('div', 'contract-card__footer');
    var sourceLink = createElement(
      'a',
      'contract-card__link',
      translate('contracts.sourceLink', 'View source')
    );
    sourceLink.href = sourceUrl(contract);
    sourceLink.target = '_blank';
    sourceLink.rel = 'noopener noreferrer';
    footer.appendChild(sourceLink);

    var docs = docsUrl(contract);
    if (docs) {
      var docsLink = createElement(
        'a',
        'contract-card__link',
        translate('contracts.docsLink', 'Read docs')
      );
      docsLink.href = docs;
      docsLink.target = '_blank';
      docsLink.rel = 'noopener noreferrer';
      footer.appendChild(docsLink);
    }
    card.appendChild(footer);

    return card;
  }

  function renderResultsCount(count) {
    var countElement = document.getElementById('contract-result-count');
    if (!countElement) return;
    countElement.textContent = translate('contracts.showing', '{count} contracts shown', {
      count: count,
    });
  }

  function renderContractGrid(container, options) {
    if (!container || typeof document === 'undefined') return [];
    var settings = options || {};
    var category = settings.category || 'all';
    var config = settings.config || getDeploymentConfig();
    var demoRegistry = settings.demoRegistry || getDemoRegistry();
    var visible = filterContracts(CONTRACTS, category);

    container.textContent = '';
    visible.forEach(function (contract) {
      container.appendChild(renderCard(contract, config, demoRegistry));
    });
    renderResultsCount(visible.length);
    return visible;
  }

  function renderFilters(container, activeCategory, onSelect) {
    if (!container || typeof document === 'undefined') return;
    container.textContent = '';
    CATEGORIES.forEach(function (category) {
      var button = createElement(
        'button',
        'contract-filter',
        translate(category.labelKey, category.fallback)
      );
      button.type = 'button';
      button.setAttribute('data-category', category.id);
      button.setAttribute('aria-pressed', String(category.id === activeCategory));
      button.addEventListener('click', function () {
        onSelect(category.id);
      });
      container.appendChild(button);
    });
  }

  function initContractGrid() {
    if (typeof document === 'undefined') return;
    var grid = document.getElementById('contract-grid');
    if (!grid) return;

    var filterContainer = document.getElementById('contract-filters');
    var activeCategory = 'all';

    function update() {
      renderContractGrid(grid, { category: activeCategory });
      if (filterContainer) {
        Array.prototype.forEach.call(
          filterContainer.querySelectorAll('.contract-filter'),
          function (button) {
            button.setAttribute(
              'aria-pressed',
              String(button.getAttribute('data-category') === activeCategory)
            );
          }
        );
      }
    }

    if (filterContainer) {
      filterContainer.setAttribute(
        'aria-label',
        translate('contracts.filterLabel', 'Filter contracts by category')
      );
      renderFilters(filterContainer, activeCategory, function (category) {
        activeCategory = category;
        update();
      });
    }

    update();
    document.addEventListener('retinax:languagechange', function () {
      if (filterContainer) {
        renderFilters(filterContainer, activeCategory, function (category) {
          activeCategory = category;
          update();
        });
      }
      update();
    });
  }

  if (typeof document !== 'undefined') {
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', initContractGrid);
    } else {
      initContractGrid();
    }
  }

  return {
    GITHUB_BASE: GITHUB_BASE,
    CATEGORIES: CATEGORIES,
    CONTRACTS: CONTRACTS,
    filterContracts: filterContracts,
    resolveContractStatus: resolveContractStatus,
    renderContractGrid: renderContractGrid,
    renderFilters: renderFilters,
    initContractGrid: initContractGrid,
  };
});
