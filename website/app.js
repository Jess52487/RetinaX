/* ==========================================================================
   RetinaX — Clinical Interactive JS & GSAP + AOS Mega Animations
   ========================================================================== */

if (typeof document !== 'undefined') {
  document.addEventListener('DOMContentLoaded', () => {
    initPageTransitions();
    // 1. Initialize AOS (Animate On Scroll)
    if (typeof AOS !== 'undefined') {
      AOS.init({
        duration: 800,
        easing: 'ease-out-cubic',
        once: true,
        offset: 100,
      });
    }

    // 3. Initialize Interactive Simulators
    initNavigationA11y();
    initDemoTabs();
    initRBACSimulator();
    initZKSimulator();
    initAISimulator();
    initFHIRSimulator();
    initDataFetchSimulator();
    initModal();
    initWallet();
    initGlobalSpinner();
    initThemeToggle();
    const year = document.getElementById('copyright-year');
    if (year) year.textContent = String(new Date().getFullYear());
  });
}

function initPageTransitions() {
  if (typeof window === 'undefined' || typeof document === 'undefined') return;
  if (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  document.addEventListener('click', (event) => {
    const link = event.target.closest('a[href]');
    if (!link || link.target === '_blank' || link.origin !== window.location.origin) return;
    const href = link.getAttribute('href');
    if (!href || href.startsWith('#') || href.startsWith('mailto:')) return;
    event.preventDefault();
    document.body.classList.add('page-transitioning');
    window.setTimeout(() => {
      window.location.href = href;
    }, 180);
  });
}

/* ==========================================================================
   Theme preference
   ========================================================================== */
const THEME_STORAGE_KEY = 'retinax-theme';

// Storage can throw (blocked cookies, some private modes), so reads and writes are guarded.
// A missing or invalid stored value is treated as "no preference chosen".
function readStoredTheme() {
  try {
    const value = window.localStorage && window.localStorage.getItem(THEME_STORAGE_KEY);
    return value === 'light' || value === 'dark' ? value : null;
  } catch (_) {
    return null;
  }
}

function writeStoredTheme(theme) {
  try {
    if (window.localStorage) window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch (_) {
    // The theme still applies to this page view; it just will not be remembered.
  }
}

function initThemeToggle() {
  const toggle = document.getElementById('theme-toggle');
  const root = document.documentElement;
  const media = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;
  const setTheme = (theme) => {
    root.dataset.theme = theme;
    if (toggle) {
      toggle.setAttribute('aria-pressed', String(theme === 'dark'));
      toggle.setAttribute(
        'aria-label',
        theme === 'dark'
          ? i18nText('nav.themeToggleToLight', 'Switch to light mode')
          : i18nText('nav.themeToggleToDark', 'Switch to dark mode')
      );
      toggle.textContent =
        theme === 'dark'
          ? i18nText('nav.themeLight', '☀️ Light mode')
          : i18nText('nav.themeDark', '🌙 Dark mode');
    }
  };

  setTheme(readStoredTheme() || (media && media.matches ? 'dark' : 'light'));

  // Follow OS changes only while the visitor has not chosen a theme themselves.
  if (media && media.addEventListener) {
    media.addEventListener('change', (event) => {
      if (!readStoredTheme()) setTheme(event.matches ? 'dark' : 'light');
    });
  }

  if (toggle)
    toggle.addEventListener('click', () => {
      const theme = root.dataset.theme === 'dark' ? 'light' : 'dark';
      // Animate color changes for a moment so the switch is not jarring. Motion-reduced users get
      // no transition, enforced in styles.css.
      root.classList.add('theme-transition');
      window.setTimeout(() => root.classList.remove('theme-transition'), 300);
      setTheme(theme);
      writeStoredTheme(theme);
    });

  document.addEventListener('retinax:languagechange', () => {
    setTheme(root.dataset.theme === 'dark' ? 'dark' : 'light');
  });
}

/* ==========================================================================
   Toast Notification System for Cross-Contract Calls
   ========================================================================== */
const ToastSystem = {
  container: null,

  init() {
    if (typeof document === 'undefined') return;
    this.container = document.getElementById('toast-container');
    if (!this.container) {
      this.container = document.createElement('div');
      this.container.id = 'toast-container';
      this.container.className = 'toast-container';
      this.container.setAttribute('aria-live', 'polite');
      this.container.setAttribute('aria-atomic', 'true');
      document.body.appendChild(this.container);
    }
  },

  show(message, type = 'pending', duration = 5000) {
    if (typeof document === 'undefined') {
      return { id: `toast-${Date.now()}`, message, type };
    }
    if (!this.container) this.init();

    const toast = document.createElement('div');
    toast.className = `toast toast-${type}`;
    toast.setAttribute('role', 'alert');

    const icon = document.createElement('span');
    icon.className = 'toast-icon';

    const msg = document.createElement('span');
    msg.className = 'toast-message';
    msg.textContent = message;

    const closeBtn = document.createElement('button');
    closeBtn.className = 'toast-close';
    closeBtn.innerHTML = '&times;';
    closeBtn.setAttribute('aria-label', i18nText('common.closeNotification', 'Close notification'));
    closeBtn.addEventListener('click', () => this.dismiss(toast));

    if (type === 'pending') {
      icon.innerHTML = '<span class="loading-spinner"></span>';
    } else if (type === 'success') {
      icon.innerHTML =
        '<svg class="toast-checkmark" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
        '<circle class="toast-checkmark-circle" cx="12" cy="12" r="10" pathLength="1"/>' +
        '<polyline class="toast-checkmark-tick" points="7 12.5 10.5 16 17 8.5" pathLength="1"/></svg>';
    } else if (type === 'error') {
      icon.innerHTML = '&#10007;';
    } else if (type === 'warning') {
      icon.innerHTML = '&#9888;';
    }

    toast.appendChild(icon);
    toast.appendChild(msg);
    toast.appendChild(closeBtn);
    this.container.appendChild(toast);

    if (typeof requestAnimationFrame !== 'undefined') {
      requestAnimationFrame(() => {
        toast.classList.add('show');
      });
    } else {
      toast.classList.add('show');
    }

    if (duration > 0) {
      setTimeout(() => this.dismiss(toast), duration);
    }

    return toast;
  },

  dismiss(toast) {
    if (!toast) return;
    if (typeof toast.classList !== 'undefined') {
      toast.classList.remove('show');
    }
    setTimeout(() => {
      if (toast.parentNode) {
        toast.parentNode.removeChild(toast);
      }
    }, 300);
  },

  pending(message) {
    return this.show(message, 'pending', 0);
  },

  success(message, duration = 5000) {
    return this.show(message, 'success', duration);
  },

  error(message, duration = 8000) {
    return this.show(message, 'error', duration);
  },

  warning(message, duration = 6000) {
    return this.show(message, 'warning', duration);
  },
};

/* ==========================================================================
   Cross-Contract Call Manager with Loading States
   ========================================================================== */
const CrossContractCall = {
  activeCalls: new Map(),

  async execute(callId, operation, options = {}) {
    const {
      pendingMsg = i18nText('common.processing', 'Processing...'),
      successMsg = i18nText('common.confirmed', 'Transaction confirmed'),
      errorMsg = i18nText('common.failed', 'Transaction failed'),
      simulateFailure = false,
      failureRate = 0,
    } = options;

    if (this.activeCalls.has(callId)) {
      return { status: 'pending', message: 'Call already in progress' };
    }

    const toast = ToastSystem.pending(pendingMsg);
    this.activeCalls.set(callId, { toast, startTime: Date.now() });

    try {
      const result = await operation();

      if (simulateFailure || (failureRate > 0 && Math.random() < failureRate)) {
        throw new Error('Contract call reverted: insufficient permissions or invalid state');
      }

      this.activeCalls.delete(callId);
      ToastSystem.dismiss(toast);
      ToastSystem.success(successMsg);

      return { status: 'success', result };
    } catch (error) {
      this.activeCalls.delete(callId);
      ToastSystem.dismiss(toast);

      const errorMessage = error.message || String(error);
      const isPartialFailure = errorMessage.includes('revert') || errorMessage.includes('partial');

      if (isPartialFailure) {
        ToastSystem.error(`Partial failure: ${errorMessage}. Some state may have been updated.`);
      } else {
        ToastSystem.error(`${errorMsg}: ${errorMessage}`);
      }

      return { status: 'error', error: errorMessage, isPartialFailure };
    }
  },

  isActive(callId) {
    return this.activeCalls.has(callId);
  },

  cancel(callId) {
    const call = this.activeCalls.get(callId);
    if (call) {
      ToastSystem.dismiss(call.toast);
      this.activeCalls.delete(callId);
      return true;
    }
    return false;
  },
};

/* ==========================================================================
   Micro-Contracts Registry & Action Routing System (Issue #5)
   Handles multiple micro-contracts: audit, identity, key_manager, vision_records
   ========================================================================== */

/**
 * Truncate middle of address or hash for UI display
 */
function truncateMiddle(str, startChars = 6, endChars = 6) {
  if (!str || typeof str !== 'string') return '';
  if (str.length <= startChars + endChars) return str;
  return `${str.slice(0, startChars)}...${str.slice(-endChars)}`;
}

/**
 * Default Stellar/Soroban Contract Addresses for RetinaX Micro-Contracts
 */
const DEFAULT_CONTRACT_REGISTRY = {
  audit: 'CAUDIT7XX4M5K6RETINAXAUDITTRAIL2026PROTOCOLLOGGER00001',
  identity: 'CIDENTITY99RETINAXPATIENTIDPROVIDERENCLAVEAUTH000002',
  key_manager: 'CKEYMGR88RETINAXEPHEMERALROTATIONDELEGATIONKEY000003',
  vision_records: 'CVISIONREC77RETINAXOPHTHALMOLOGYEHRRECORDDATA000004',
  zk_verifier: 'CZKVERIFY66RETINAXGROTH16SNARKPROOFENGINE0000005',
  ai_integration: 'CAIORACLE55RETINAXDIAGNOSTICORACLEROTATION000006',
  fhir: 'CFHIRV4MAP44RETINAXEMRBRIDGELOINCDATACONVERT0000007',
  compliance: 'CCOMPLIANCE33RETINAXGDPRHIPAATRANSITPOLICY000008',
};

/**
 * ContractRegistry: Registry of micro-contract addresses
 */
const ContractRegistry = {
  _contracts: new Map(Object.entries(DEFAULT_CONTRACT_REGISTRY)),

  /**
   * Initialize or override contract addresses from config
   * @param {Object} config - Key-value pair of micro-contract names and addresses
   */
  init(config = {}) {
    this._contracts = new Map(Object.entries(DEFAULT_CONTRACT_REGISTRY));
    this.loadFromConfig(config);
    return this;
  },

  /**
   * Load addresses from custom config object or environment
   * @param {Object} config
   */
  loadFromConfig(config = {}) {
    if (typeof config === 'object' && config !== null) {
      for (const [name, address] of Object.entries(config)) {
        if (address) {
          this.set(name, address);
        }
      }
    }
    return this;
  },

  /**
   * Register or update a micro-contract address
   * @param {string} name - Contract key (e.g., 'audit', 'identity', 'key_manager', 'vision_records')
   * @param {string} address - Contract address string
   */
  set(name, address) {
    if (!name || typeof name !== 'string') {
      throw new TypeError('Contract name must be a non-empty string');
    }
    if (!address || typeof address !== 'string') {
      throw new TypeError('Contract address must be a non-empty string');
    }
    const cleanName = name.trim().toLowerCase();
    const cleanAddress = address.trim();
    this._contracts.set(cleanName, cleanAddress);
  },

  /**
   * Get the address for a micro-contract
   * @param {string} name - Micro-contract name
   * @returns {string} Contract address
   */
  get(name) {
    if (!name || typeof name !== 'string') {
      throw new TypeError('Contract name must be a non-empty string');
    }
    const cleanName = name.trim().toLowerCase();
    const addr = this._contracts.get(cleanName);
    if (!addr) {
      throw new Error(`Micro-contract "${name}" is not registered in ContractRegistry`);
    }
    return addr;
  },

  /**
   * Check if a micro-contract is registered
   * @param {string} name
   * @returns {boolean}
   */
  has(name) {
    if (!name || typeof name !== 'string') return false;
    return this._contracts.has(name.trim().toLowerCase());
  },

  /**
   * Get all registered micro-contracts
   * @returns {Object} Map of contract name to address
   */
  getAll() {
    return Object.fromEntries(this._contracts.entries());
  },

  /**
   * Reset registry to defaults
   */
  reset() {
    this._contracts = new Map(Object.entries(DEFAULT_CONTRACT_REGISTRY));
    return this;
  },
};

/**
 * Action-to-Micro-Contract Route Table
 */
const DEFAULT_ACTION_ROUTES = {
  // vision_records actions
  grant_access: 'vision_records',
  revoke_access: 'vision_records',
  get_record: 'vision_records',
  create_record: 'vision_records',
  update_record: 'vision_records',
  'rbac-grant': 'vision_records',
  'patient-record-fetch': 'vision_records',
  'data-fetch': 'vision_records',

  // identity actions
  register_patient: 'identity',
  verify_identity: 'identity',
  get_profile: 'identity',
  update_profile: 'identity',
  'identity-verify': 'identity',
  'patient-auth': 'identity',

  // key_manager actions
  rotate_key: 'key_manager',
  store_key: 'key_manager',
  get_public_key: 'key_manager',
  revoke_key: 'key_manager',
  'key-rotation': 'key_manager',
  delegate_access: 'key_manager',

  // audit actions
  log_event: 'audit',
  get_audit_trail: 'audit',
  verify_audit: 'audit',
  record_access_attempt: 'audit',
  'audit-log': 'audit',
  'compliance-audit': 'audit',

  // zk_verifier actions
  verify_zk_proof: 'zk_verifier',
  'zk-proof': 'zk_verifier',

  // ai_integration actions
  rotate_provider: 'ai_integration',
  'ai-diagnostic': 'ai_integration',
  evaluate_scan: 'ai_integration',

  // fhir converter actions
  convert_fhir: 'fhir',
  'fhir-convert': 'fhir',
};

/**
 * ContractRouter: Routes user actions and RPC calls to the corresponding micro-contract
 */
const ContractRouter = {
  _routes: new Map(Object.entries(DEFAULT_ACTION_ROUTES)),

  /**
   * Register a route mapping an action to a micro-contract
   * @param {string} action
   * @param {string} microContract
   */
  registerRoute(action, microContract) {
    if (!action || typeof action !== 'string') {
      throw new TypeError('Action must be a non-empty string');
    }
    if (!microContract || typeof microContract !== 'string') {
      throw new TypeError('Micro-contract name must be a non-empty string');
    }
    this._routes.set(action.trim().toLowerCase(), microContract.trim().toLowerCase());
  },

  /**
   * Determine the target micro-contract for an action
   * @param {string} action
   * @returns {string} Micro-contract identifier
   */
  getContractForAction(action) {
    if (!action || typeof action !== 'string') {
      throw new TypeError('Action must be a non-empty string');
    }
    const cleanAction = action.trim().toLowerCase();

    // If the action matches directly a registered micro-contract name, route directly
    if (ContractRegistry.has(cleanAction)) {
      return cleanAction;
    }

    const microContract = this._routes.get(cleanAction);
    if (!microContract) {
      throw new Error(`No micro-contract route registered for action: "${action}"`);
    }
    return microContract;
  },

  /**
   * Route an action to its micro-contract name and resolved contract address
   * @param {string} action
   * @returns {{ action: string, microContract: string, contractAddress: string }}
   */
  routeAction(action) {
    const microContract = this.getContractForAction(action);
    const contractAddress = ContractRegistry.get(microContract);
    return {
      action,
      microContract,
      contractAddress,
    };
  },

  /**
   * Get all registered routes
   * @returns {Object}
   */
  getAllRoutes() {
    return Object.fromEntries(this._routes.entries());
  },

  /**
   * Reset routes to defaults
   */
  reset() {
    this._routes = new Map(Object.entries(DEFAULT_ACTION_ROUTES));
    return this;
  },
};

/**
 * SorobanRPC: Dispatches RPC invocations to the designated micro-contract
 */
const SorobanRPC = {
  rpcUrl: 'https://soroban-testnet.stellar.org',
  invocationHistory: [],

  /**
   * Set Soroban RPC endpoint URL
   * @param {string} url
   */
  setRpcUrl(url) {
    if (url && typeof url === 'string') {
      this.rpcUrl = url.trim();
    }
  },

  /**
   * Build Soroban JSON-RPC call payload for an action
   * @param {string} action
   * @param {Object} params
   * @param {Object} options
   * @returns {Object} JSON-RPC request structure
   */
  buildPayload(action, params = {}, options = {}) {
    const { microContract, contractAddress } = ContractRouter.routeAction(action);
    return {
      jsonrpc: '2.0',
      id: options.rpcId || `rpc-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      method: options.method || 'simulateTransaction',
      params: {
        contractAddress,
        microContract,
        action,
        functionName: options.functionName || action,
        args: params,
        auth: options.auth || null,
        network: options.network || 'testnet',
      },
    };
  },

  /**
   * Invoke a micro-contract action via Soroban RPC
   * @param {string} action - Action or method name
   * @param {Object} params - Arguments for the contract method
   * @param {Object} options - Invocation options (pendingMsg, successMsg, errorMsg, simulateFailure, rpcUrl, etc.)
   * @returns {Promise<Object>} Execution result
   */
  async invoke(action, params = {}, options = {}) {
    const route = ContractRouter.routeAction(action);
    const { microContract, contractAddress } = route;
    const payload = this.buildPayload(action, params, options);

    const callId = options.callId || `call-${action}-${Date.now()}`;
    const pendingMsg = options.pendingMsg || `Invoking [${microContract}]::${action}...`;
    const successMsg =
      options.successMsg ||
      `[${microContract}] Transaction Confirmed (${truncateMiddle(contractAddress, 8, 6)})`;
    const errorMsg = options.errorMsg || `[${microContract}] Transaction Failed`;

    const execution = await CrossContractCall.execute(
      callId,
      async () => {
        // Simulated network delay or real fetch if custom transport is provided
        const delay = typeof options.delay === 'number' ? options.delay : 800;
        await new Promise((resolve) => setTimeout(resolve, delay));

        if (options.simulateFailure) {
          throw new Error(
            `Micro-contract [${microContract}] reverted: simulated transaction failure`
          );
        }

        const response = {
          success: true,
          microContract,
          contractAddress,
          action,
          payload,
          returnValue:
            options.mockResult !== undefined
              ? options.mockResult
              : { status: 'OK', ledger: 1789210 },
          txHash: `0x${Array.from({ length: 32 }, () => Math.floor(Math.random() * 16).toString(16)).join('')}`,
          timestamp: new Date().toISOString(),
        };

        return response;
      },
      {
        pendingMsg,
        successMsg,
        errorMsg,
        simulateFailure: Boolean(options.simulateFailure),
        failureRate: options.failureRate || 0,
      }
    );

    const record = {
      action,
      microContract,
      contractAddress,
      status: execution.status,
      timestamp: Date.now(),
      error: execution.error || null,
    };
    this.invocationHistory.push(record);

    return {
      ...execution,
      microContract,
      contractAddress,
      action,
      payload,
    };
  },

  /**
   * Get history of RPC invocations
   */
  getHistory() {
    return [...this.invocationHistory];
  },

  /**
   * Clear invocation history
   */
  clearHistory() {
    this.invocationHistory = [];
  },
};

// Addresses are supplied by the deployment page through window.RETINAX_CONTRACTS.
// A wallet integration can install window.RetinaXRPC.invokeContract to submit calls.
const CONTRACT_ACTIONS = Object.freeze({
  grant_access: 'vision_records',
  revoke_access: 'vision_records',
  grant_consent: 'vision_records',
  revoke_consent: 'vision_records',
  register_identity: 'identity',
  verify_identity: 'identity',
  rotate_key: 'key_manager',
  append_audit: 'audit',
});

async function invokeContractAction(action, args = {}) {
  const contract = CONTRACT_ACTIONS[action];
  if (!contract) throw new Error(`Unsupported contract action: ${action}`);
  const rpc = window.RetinaXRPC;
  if (!rpc || typeof rpc.invokeContract !== 'function') return null;

  const registry = window.RETINAX_CONTRACTS || {};
  const contractId = registry[contract];
  if (!contractId) throw new Error(`No deployed address configured for ${contract}`);
  return rpc.invokeContract({
    contract,
    contractId,
    action,
    args,
    rpcUrl: window.RETINAX_RPC_URL || '',
  });
}

if (typeof window !== 'undefined') {
  window.RetinaXContractActions = CONTRACT_ACTIONS;
  window.invokeRetinaXContractAction = invokeContractAction;
}

/* ==========================================================================
   Stellar Networks & Freighter Wallet (Issue #283)
   Uses @stellar/freighter-api, loaded in index.html as window.freighterApi.
   ========================================================================== */
const STELLAR_NETWORKS = Object.freeze({
  testnet: Object.freeze({
    labelKey: 'wallet.testnet',
    label: 'Stellar Testnet',
    passphrase: 'Test SDF Network ; September 2015',
  }),
  mainnet: Object.freeze({
    labelKey: 'wallet.mainnet',
    label: 'Stellar Mainnet',
    passphrase: 'Public Global Stellar Network ; September 2015',
  }),
});
const DEFAULT_NETWORK = 'testnet';
const NETWORK_STORAGE_KEY = 'retinax-network';
// How often Freighter is polled for account or network switches (the library default).
const WALLET_WATCH_INTERVAL_MS = 3000;

function isStellarNetwork(network) {
  return Object.prototype.hasOwnProperty.call(STELLAR_NETWORKS, network);
}

function networkLabel(network) {
  const config = STELLAR_NETWORKS[network];
  return i18nText(config.labelKey, config.label);
}

function readStoredNetwork() {
  try {
    const value = window.localStorage && window.localStorage.getItem(NETWORK_STORAGE_KEY);
    return isStellarNetwork(value) ? value : null;
  } catch (_) {
    return null;
  }
}

function writeStoredNetwork(network) {
  try {
    if (window.localStorage) window.localStorage.setItem(NETWORK_STORAGE_KEY, network);
  } catch (_) {
    // The selection still applies to this page view; it just will not be remembered.
  }
}

/** Error raised by wallet operations; `code` identifies the failure for the UI. */
class WalletError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'WalletError';
    this.code = code;
  }
}

/**
 * Freighter connection state. Listeners passed to subscribe() receive a snapshot after every
 * change, whether it came from this page or from the extension.
 */
const FreighterWallet = {
  address: '',
  walletPassphrase: '',
  network: DEFAULT_NETWORK,
  _watcher: null,
  _listeners: new Set(),

  _api() {
    return typeof window !== 'undefined' ? window.freighterApi : undefined;
  },

  /** Resolves true when the Freighter extension answers; the library gives up after 2 s. */
  async isInstalled() {
    const api = this._api();
    if (!api) return false;
    const result = await api.isConnected();
    return Boolean(result && result.isConnected);
  },

  /** Asks Freighter for access (a popup on first use) and stores the selected account. */
  async connect() {
    if (!(await this.isInstalled())) {
      throw new WalletError('not-installed', 'Freighter is not installed');
    }
    const api = this._api();
    const access = await api.requestAccess();
    if (access.error) throw new WalletError('rejected', access.error.message);
    const details = await api.getNetwork();
    if (details.error) throw new WalletError('network', details.error.message);

    this.address = access.address;
    this.walletPassphrase = details.networkPassphrase;
    this._watch();
    this._emit();
    return this.address;
  },

  /**
   * Ends the session on this page. Freighter has no API to revoke a site's access; users do that
   * in the extension, which the watcher then reports as a disconnection.
   */
  disconnect() {
    if (this._watcher) this._watcher.stop();
    this._watcher = null;
    this.address = '';
    this.walletPassphrase = '';
    this._emit();
  },

  setNetwork(network) {
    if (!isStellarNetwork(network)) throw new TypeError(`Unsupported network: ${network}`);
    this.network = network;
    this._emit();
  },

  /** True when Freighter is set to a different network than the one selected on the page. */
  hasNetworkMismatch() {
    return (
      Boolean(this.address) && this.walletPassphrase !== STELLAR_NETWORKS[this.network].passphrase
    );
  },

  /** Has the connected account sign a transaction for the selected network. */
  async signTransaction(xdr) {
    const api = this._api();
    if (!api || !this.address) throw new WalletError('not-connected', 'Connect Freighter first');
    if (this.hasNetworkMismatch()) {
      throw new WalletError(
        'network-mismatch',
        `Switch Freighter to ${STELLAR_NETWORKS[this.network].label}`
      );
    }
    const result = await api.signTransaction(xdr, {
      networkPassphrase: STELLAR_NETWORKS[this.network].passphrase,
      address: this.address,
    });
    if (result.error) throw new WalletError('rejected', result.error.message);
    return result.signedTxXdr;
  },

  /** Soroban RPC URL configured in Freighter, when it is on the selected network. */
  async sorobanRpcUrl() {
    const api = this._api();
    if (!api || !this.address || this.hasNetworkMismatch()) return '';
    const details = await api.getNetworkDetails();
    return details.error ? '' : details.sorobanRpcUrl || '';
  },

  getState() {
    return {
      address: this.address,
      connected: Boolean(this.address),
      network: this.network,
      networkMismatch: this.hasNetworkMismatch(),
    };
  },

  subscribe(listener) {
    this._listeners.add(listener);
    return () => this._listeners.delete(listener);
  },

  _emit() {
    const state = this.getState();
    this._listeners.forEach((listener) => listener(state));
  },

  // Follows account and network switches made in the extension. An empty address means the
  // wallet was locked or the site's access was revoked.
  _watch() {
    const api = this._api();
    if (this._watcher || !api || typeof api.WatchWalletChanges !== 'function') return;
    this._watcher = new api.WatchWalletChanges(WALLET_WATCH_INTERVAL_MS);
    this._watcher.watch(({ address, networkPassphrase }) => {
      if (!address) {
        this.disconnect();
        return;
      }
      this.address = address;
      this.walletPassphrase = networkPassphrase;
      this._emit();
    });
  },
};

/* ==========================================================================
   Soroban RPC Contract Calls & access_control Client (Issue #284)
   Uses @stellar/stellar-sdk, loaded in index.html as window.StellarSdk. Contract IDs and RPC
   URLs come from window.RETINAX_DEPLOYMENTS, keyed by network.
   ========================================================================== */
// Role and Permission are integer enums in contracts/access_control/src/types.rs, so they are
// passed and returned as u32 values.
const ACCESS_CONTROL_ROLES = Object.freeze([
  'None',
  'Patient',
  'Staff',
  'Optometrist',
  'Ophthalmologist',
  'Admin',
]);
const ACCESS_CONTROL_PERMISSIONS = Object.freeze({
  1: 'ReadAnyRecord',
  2: 'WriteRecord',
  3: 'ManageAccess',
  4: 'ManageUsers',
  5: 'SystemAdmin',
});
// AccessControlError codes, as declared in contracts/access_control/src/types.rs.
const ACCESS_CONTROL_ERRORS = Object.freeze({
  1: 'Unauthorized',
  2: 'InvalidInput',
  3: 'NotFound',
  4: 'DelegationExpired',
  5: 'Paused',
  6: 'AlreadyExists',
  7: 'AlreadyInitialized',
  8: 'DelegationDepthExceeded',
});
const SOROBAN_TX_TIMEOUT_SECONDS = 30;
// pollTransaction waits 1 s between attempts and ledgers close about every 5 s.
const SOROBAN_POLL_ATTEMPTS = 30;

function configuredContractId(network, contractName) {
  const deployments = (typeof window !== 'undefined' && window.RETINAX_DEPLOYMENTS) || {};
  const deployment = deployments[network] || {};
  return (deployment.contracts || {})[contractName] || '';
}

/** Turns a Soroban failure into one readable line, naming access_control error codes. */
function describeAccessControlError(error) {
  const message = String((error && error.message) || error).split('\n')[0];
  const match = /Error\(Contract, #(\d+)\)/.exec(message);
  if (!match) return message;
  const name = ACCESS_CONTROL_ERRORS[match[1]] || 'Unknown';
  return `access_control error ${name} (Error(Contract, #${match[1]}))`;
}

const SorobanClient = {
  _servers: new Map(),

  sdk() {
    const sdk = typeof window !== 'undefined' ? window.StellarSdk : undefined;
    if (!sdk || !sdk.rpc) throw new Error('The Stellar SDK did not load');
    return sdk;
  },

  async _target(network, contractName) {
    const label = STELLAR_NETWORKS[network].label;
    const contractId = configuredContractId(network, contractName);
    if (!contractId) throw new Error(`No ${contractName} contract is configured for ${label}`);
    const deployment = window.RETINAX_DEPLOYMENTS[network];
    const rpcUrl = deployment.rpcUrl || (await FreighterWallet.sorobanRpcUrl());
    if (!rpcUrl) throw new Error(`No Soroban RPC URL is configured for ${label}`);
    return { contractId, rpcUrl, passphrase: STELLAR_NETWORKS[network].passphrase };
  },

  _server(rpcUrl) {
    if (!this._servers.has(rpcUrl)) this._servers.set(rpcUrl, new (this.sdk().rpc.Server)(rpcUrl));
    return this._servers.get(rpcUrl);
  },

  _transaction(account, target, method, args) {
    const sdk = this.sdk();
    return new sdk.TransactionBuilder(account, {
      fee: sdk.BASE_FEE,
      networkPassphrase: target.passphrase,
    })
      .addOperation(new sdk.Contract(target.contractId).call(method, ...args))
      .setTimeout(SOROBAN_TX_TIMEOUT_SECONDS)
      .build();
  },

  /** Simulates a read-only call and returns its decoded value. Nothing is signed or submitted. */
  async read({ network, contract, method, args, source }) {
    const sdk = this.sdk();
    const target = await this._target(network, contract);
    // Simulation never loads the source account, so any valid address works with sequence 0.
    const tx = this._transaction(new sdk.Account(source, '0'), target, method, args);
    const simulation = await this._server(target.rpcUrl).simulateTransaction(tx);
    if (sdk.rpc.Api.isSimulationError(simulation)) throw new Error(simulation.error);
    return { contractId: target.contractId, value: sdk.scValToNative(simulation.result.retval) };
  },

  /**
   * Simulates, signs with the connected `signer`, submits, and waits for a state-changing call
   * to settle.
   */
  async invoke({ network, contract, method, args, signer }) {
    const sdk = this.sdk();
    const target = await this._target(network, contract);
    const server = this._server(target.rpcUrl);

    let account;
    try {
      account = await server.getAccount(signer.address);
    } catch (err) {
      if (!/not found/i.test(err.message)) throw err;
      throw new Error(`${signer.address} has no account on ${STELLAR_NETWORKS[network].label}`);
    }
    // prepareTransaction simulates, then adds the footprint, auth entries and resource fee.
    const prepared = await server.prepareTransaction(
      this._transaction(account, target, method, args)
    );
    const signedXdr = await signer.signTransaction(prepared.toXDR());
    const sent = await server.sendTransaction(
      sdk.TransactionBuilder.fromXDR(signedXdr, target.passphrase)
    );
    if (sent.status !== 'PENDING' && sent.status !== 'DUPLICATE') {
      throw new Error(`The network did not accept the transaction (${sent.status})`);
    }

    const result = await server.pollTransaction(sent.hash, { attempts: SOROBAN_POLL_ATTEMPTS });
    if (result.status !== sdk.rpc.Api.GetTransactionStatus.SUCCESS) {
      const outcome = result.status === 'NOT_FOUND' ? 'was not confirmed in time' : 'failed';
      throw new Error(`Transaction ${sent.hash} ${outcome}`);
    }
    return { contractId: target.contractId, hash: sent.hash, ledger: result.ledger };
  },
};

/** Typed calls to contracts/access_control. */
const AccessControlClient = {
  _account(address) {
    const sdk = SorobanClient.sdk();
    if (!sdk.StrKey.isValidEd25519PublicKey(address)) {
      throw new Error(`Not a valid Stellar account address: ${address || '(empty)'}`);
    }
    return sdk.nativeToScVal(address, { type: 'address' });
  },

  /** get_role(user) -> Role, as its u32 value. */
  async getRole(network, user) {
    const { contractId, value } = await SorobanClient.read({
      network,
      contract: 'access_control',
      method: 'get_role',
      args: [this._account(user)],
      source: user,
    });
    return { contractId, role: Number(value) };
  },

  /** has_permission(user, permission) -> bool. */
  async hasPermission(network, user, permission) {
    const { contractId, value } = await SorobanClient.read({
      network,
      contract: 'access_control',
      method: 'has_permission',
      args: [this._account(user), SorobanClient.sdk().nativeToScVal(permission, { type: 'u32' })],
      source: user,
    });
    return { contractId, granted: Boolean(value) };
  },

  /** assign_role(caller, user, role, expires_at), signed by the connected Freighter account. */
  async assignRole(network, user, role, expiresAt) {
    if (!FreighterWallet.address) {
      throw new WalletError('not-connected', 'Connect Freighter to sign assign_role');
    }
    const sdk = SorobanClient.sdk();
    const args = [
      this._account(FreighterWallet.address),
      this._account(user),
      sdk.nativeToScVal(role, { type: 'u32' }),
      sdk.nativeToScVal(BigInt(expiresAt), { type: 'u64' }),
    ];
    return SorobanClient.invoke({
      network,
      contract: 'access_control',
      method: 'assign_role',
      args,
      signer: FreighterWallet,
    });
  },
};

/* ==========================================================================
   Button Loading State Helper
   ========================================================================== */
function setButtonLoading(
  button,
  loading,
  loadingText = i18nText('common.processing', 'Processing...')
) {
  if (!button) return;

  if (loading) {
    button.classList.add('btn-loading');
    button.dataset.originalText = button.textContent;
    button.innerHTML = `<span class="btn-text">${loadingText}</span>`;
    button.disabled = true;
  } else {
    button.classList.remove('btn-loading');
    if (button.dataset.originalText) {
      button.textContent = button.dataset.originalText;
    }
    button.disabled = false;
  }
}

/* ==========================================================================
   Cross-Contract Status Indicator Helper
   ========================================================================== */
function showCCStatus(containerId, status, message) {
  const container = document.getElementById(containerId);
  if (!container) return;

  const statusClass = `cc-status-${status}`;
  const statusLabel =
    status === 'pending'
      ? i18nText('common.pending', 'PENDING')
      : status === 'success'
        ? i18nText('common.confirmed', 'CONFIRMED')
        : i18nText('common.failed', 'FAILED');

  container.className = `cc-status ${statusClass}`;
  container.innerHTML = `
    <span class="cc-status-label">${statusLabel}</span>
    <span class="cc-status-message">${message}</span>
    ${status === 'pending' ? '<span class="loading-spinner"></span>' : ''}
  `;
  container.style.display = 'flex';
}

function hideCCStatus(containerId) {
  const container = document.getElementById(containerId);
  if (container) {
    container.style.display = 'none';
  }
}

/* ==========================================================================
   Global Loading Spinner
   ========================================================================== */
function initGlobalSpinner() {
  const spinner = document.getElementById('global-spinner');
  if (!spinner) return;

  window.showGlobalSpinner = function (message) {
    if (spinner.show) {
      spinner.show(message || 'Processing...');
    }
  };

  window.hideGlobalSpinner = function () {
    if (spinner.hide) {
      spinner.hide();
    }
  };
}

/* ==========================================================================
   GSAP Mega Animation Timelines
   ========================================================================== */
// Retained for future opt-in page transitions; AOS handles the current page.
// eslint-disable-next-line no-unused-vars
function initGSAPAnimations() {
  if (typeof gsap === 'undefined') return;

  // Hero Section Staggered Entrance
  const heroTl = gsap.timeline();

  heroTl
    .from('.hero-tag', {
      opacity: 0,
      y: -20,
      duration: 0.6,
      ease: 'power2.out',
    })
    .from(
      '.hero-title',
      {
        opacity: 0,
        y: 30,
        duration: 0.8,
        ease: 'power3.out',
      },
      '-=0.3'
    )
    .from(
      '.hero-sub',
      {
        opacity: 0,
        y: 20,
        duration: 0.7,
        ease: 'power2.out',
      },
      '-=0.4'
    )
    .from(
      '.hero-cta-group .btn',
      {
        opacity: 0,
        y: 20,
        stagger: 0.15,
        duration: 0.6,
        ease: 'back.out(1.7)',
      },
      '-=0.4'
    )
    .from(
      '.hero-image-wrapper',
      {
        opacity: 0,
        scale: 0.95,
        duration: 0.9,
        ease: 'power3.out',
      },
      '-=0.8'
    );

  // GSAP Hover Micro-Interactions on Contract Cards
  const cards = document.querySelectorAll('.contract-card, .segment-img-card');
  cards.forEach((card) => {
    card.addEventListener('mouseenter', () => {
      gsap.to(card, { y: -6, duration: 0.25, ease: 'power2.out' });
    });
    card.addEventListener('mouseleave', () => {
      gsap.to(card, { y: 0, duration: 0.25, ease: 'power2.out' });
    });
  });
}

/* ==========================================================================
   Tab Switching Logic
   ========================================================================== */
function initDemoTabs() {
  const tabs = Array.from(document.querySelectorAll('.demo-tab-btn'));
  const panels = document.querySelectorAll('.demo-tab-panel');

  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => {
      activateTab(index);
    });

    // Keyboard accessibility for tablist
    tab.addEventListener('keydown', (e) => {
      let newIndex = index;
      if (e.key === 'ArrowRight') {
        newIndex = (index + 1) % tabs.length;
      } else if (e.key === 'ArrowLeft') {
        newIndex = (index - 1 + tabs.length) % tabs.length;
      } else if (e.key === 'Home') {
        newIndex = 0;
      } else if (e.key === 'End') {
        newIndex = tabs.length - 1;
      } else {
        return;
      }
      e.preventDefault();
      tabs[newIndex].focus();
      activateTab(newIndex);
    });
  });

  function activateTab(index) {
    const tab = tabs[index];
    if (!tab) return;

    tabs.forEach((t) => {
      t.classList.remove('active');
      t.setAttribute('aria-selected', 'false');
      t.setAttribute('tabindex', '-1');
    });
    panels.forEach((p) => {
      p.classList.remove('active');
      p.setAttribute('hidden', '');
    });

    tab.classList.add('active');
    tab.setAttribute('aria-selected', 'true');
    tab.setAttribute('tabindex', '0');

    const targetId = `panel-${tab.dataset.tab}`;
    const targetPanel = document.getElementById(targetId);

    if (targetPanel) {
      targetPanel.classList.add('active');
      targetPanel.removeAttribute('hidden');
      if (typeof gsap !== 'undefined') {
        gsap.fromTo(
          targetPanel,
          { opacity: 0, y: 10 },
          { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }
        );
      }
    }
  }
}

/* ==========================================================================
   i18n helper
   ========================================================================== */
function i18nText(key, fallback) {
  if (
    typeof window !== 'undefined' &&
    window.RetinaXI18n &&
    typeof window.RetinaXI18n.t === 'function'
  ) {
    const value = window.RetinaXI18n.t(key, {}, fallback);
    if (value) return value;
function i18nText(key, fallback, vars) {
  let text = fallback;
  if (typeof window !== 'undefined' && window.RetinaXI18n && typeof window.RetinaXI18n.t === 'function') {
    text = window.RetinaXI18n.t(key, vars || {}, fallback) || fallback;
  }
  // The loader interpolates catalog values but returns the fallback untouched.
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : match
  );
}

/* ==========================================================================
   RBAC Access Control Simulator (Issue #284)
   Calls get_role, has_permission and assign_role on the deployed access_control contract.
   ========================================================================== */
function initRBACSimulator() {
  const form = document.getElementById('rbac-form');
  const actionSelect = document.getElementById('rbac-action');
  const userInput = document.getElementById('input-patient-addr');
  const roleSelect = document.getElementById('rbac-role');
  const permissionSelect = document.getElementById('rbac-permission');
  const roleGroup = document.getElementById('rbac-role-group');
  const permissionGroup = document.getElementById('rbac-permission-group');
  const durationGroup = document.getElementById('rbac-duration-group');
  const durSlider = document.getElementById('dur-slider');
  const durVal = document.getElementById('dur-val');
  const submitBtn = document.getElementById('btn-grant-access');
  const previewCode = document.getElementById('preview-rbac-code');

  function readCall() {
    return {
      action: actionSelect ? actionSelect.value : 'get_role',
      user: userInput ? userInput.value.trim() : '',
      role: Number(roleSelect ? roleSelect.value : 1),
      permission: Number(permissionSelect ? permissionSelect.value : 1),
      hours: Number(durSlider ? durSlider.value : 24),
    };
  }

  function describeCall(call) {
    const user = call.user || 'G...';
    if (call.action === 'has_permission') {
      const permission = ACCESS_CONTROL_PERMISSIONS[call.permission];
      return `has_permission(user: ${user}, permission: ${permission} (${call.permission}))`;
    }
    if (call.action === 'assign_role') {
      const caller = FreighterWallet.address || '<connected wallet>';
      const role = ACCESS_CONTROL_ROLES[call.role];
      return `assign_role(caller: ${caller}, user: ${user}, role: ${role} (${call.role}), expires_at: now + ${call.hours * 3600} s)`;
    }
    return `get_role(user: ${user})`;
  }

  function describeResult(call, result) {
    if (call.action === 'get_role') {
      return `// Result: ${ACCESS_CONTROL_ROLES[result.role] || 'Unknown'} (${result.role})`;
    }
    if (call.action === 'has_permission') return `// Result: ${result.granted}`;
    return `// Result: confirmed in ledger ${result.ledger}\n// Transaction: ${result.hash}`;
  }

  function render(call, status, detail) {
    if (!previewCode) return;
    const network = FreighterWallet.network;
    const contractId = configuredContractId(network, 'access_control') || '(not configured)';
    previewCode.textContent = [
      `// Soroban RPC: contracts/access_control::${call.action}()`,
      `// Network: ${networkLabel(network)}`,
      `// Contract: ${contractId}`,
      describeCall(call),
      '',
      `// Status: ${status}`,
      detail,
    ]
      .filter(Boolean)
      .join('\n');
  }

  function renderReady() {
    render(readCall(), i18nText('demo.rbacReady', 'Ready'));
  }

  function showFieldsForAction() {
    const { action } = readCall();
    if (roleGroup) roleGroup.hidden = action !== 'assign_role';
    if (durationGroup) durationGroup.hidden = action !== 'assign_role';
    if (permissionGroup) permissionGroup.hidden = action !== 'has_permission';
    renderReady();
  }

  function runCall(call) {
    const network = FreighterWallet.network;
    if (call.action === 'has_permission') {
      return AccessControlClient.hasPermission(network, call.user, call.permission);
    }
    if (call.action === 'assign_role') {
      const expiresAt = Math.floor(Date.now() / 1000) + call.hours * 3600;
      return AccessControlClient.assignRole(network, call.user, call.role, expiresAt);
    }
    return AccessControlClient.getRole(network, call.user);
  }

  if (durSlider && durVal) {
    durSlider.addEventListener('input', (e) => {
      const hours = e.target.value;
      durVal.textContent = hours;
      durSlider.setAttribute('aria-valuenow', hours);
      durSlider.setAttribute('aria-valuetext', `${hours} ${i18nText('demo.hours', 'Hours')}`);
      renderReady();
    });
  }
  if (actionSelect) actionSelect.addEventListener('change', showFieldsForAction);
  [userInput, roleSelect, permissionSelect].forEach((el) => {
    if (el) el.addEventListener('change', renderReady);
  });

  // Default the target account to the connected wallet so a first call needs no typing.
  FreighterWallet.subscribe((state) => {
    if (state.connected && userInput && !userInput.value) userInput.value = state.address;
    renderReady();
  });

  if (grantBtn) {
    const form = grantBtn.closest('form');
    if (form) {
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        updateRBACPreview();
        setButtonLoading(
          grantBtn,
          true,
          i18nText('demo.rbacLoading', 'Executing Soroban require_auth()...')
        );
        if (window.showGlobalSpinner) {
          window.showGlobalSpinner(
            i18nText('demo.rbacLoading', 'Executing Soroban require_auth()...')
          );
        }

        let result = null;
        try {
          result = await CrossContractCall.execute(
            'rbac-grant',
            async () => {
              await invokeContractAction('grant_access', {
                patient: patientInput ? patientInput.value : '',
                grantee: doctorSelect ? doctorSelect.value : '',
                durationSeconds: Number(durSlider ? durSlider.value : 24) * 3600,
              });
              updateRBACPreview(true);
              return { granted: true };
            },
            {
              pendingMsg: i18nText(
                'demo.rbacPending',
                'Transaction Pending: Submitting access grant to Soroban...'
              ),
              successMsg: i18nText(
                'demo.rbacSuccess',
                'Access Granted On-Chain — Transaction Confirmed'
              ),
              errorMsg: i18nText('demo.rbacError', 'Access Grant Failed'),
            }
          );
        } catch (err) {
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              i18nText('demo.rbacError', 'Access Grant Failed') +
                ': ' +
                (err.message || String(err))
            );
          }
        } finally {
          setButtonLoading(grantBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        }

        if (result && result.status === 'success') {
          updateRBACPreview(true);
          grantBtn.textContent = i18nText('demo.rbacGranted', '✅ Access Granted On-Chain!');
          window.setTimeout(() => {
            grantBtn.textContent = i18nText('demo.executeAuth', 'Execute Soroban Auth');
          }, 2500);
        } else {
          grantBtn.textContent = i18nText('demo.executeAuth', 'Execute Soroban Auth');
        }
      });
    }
  if (form) {
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const call = readCall();
      setButtonLoading(
        submitBtn,
        true,
        i18nText('demo.rbacLoading', 'Calling access_control via Soroban RPC...')
      );
      if (previewCode) previewCode.setAttribute('aria-busy', 'true');
      render(call, i18nText('demo.rbacPending', 'Pending...'));

      const outcome = await CrossContractCall.execute(`rbac-${call.action}`, () => runCall(call), {
        pendingMsg: i18nText('demo.rbacPending', 'Pending...'),
        successMsg: i18nText('demo.rbacSuccess', 'Contract call confirmed'),
        errorMsg: i18nText('demo.rbacError', 'Contract call failed'),
      });

      setButtonLoading(submitBtn, false);
      if (previewCode) previewCode.removeAttribute('aria-busy');
      if (outcome.status === 'success') {
        render(
          call,
          i18nText('demo.rbacSucceeded', 'Success'),
          describeResult(call, outcome.result)
        );
      } else if (outcome.status === 'error') {
        const reason = describeAccessControlError(outcome.error);
        render(call, i18nText('demo.rbacFailed', 'Failed'), `// Error: ${reason}`);
        if (window.showErrorFeedback) {
          window.showErrorFeedback(
            `${i18nText('demo.rbacError', 'Contract call failed')}: ${reason}`
          );
        }
      }
    });
  }

  showFieldsForAction();
}

/* ==========================================================================
   ZK Proof Simulator
   ========================================================================== */
function initZKSimulator() {
  const acuitySlider = document.getElementById('acuity-slider');
  const acuityVal = document.getElementById('acuity-val');
  const genZkBtn = document.getElementById('btn-gen-zk');
  const previewZkCode = document.getElementById('preview-zk-code');

  if (acuitySlider && acuityVal) {
    acuitySlider.addEventListener('input', (e) => {
      const val = e.target.value;
      acuityVal.textContent = `20/${val}`;
      acuitySlider.setAttribute('aria-valuenow', val);
      acuitySlider.setAttribute('aria-valuetext', `20/${val}`);
      updateZKPreview();
    });
  }

  if (genZkBtn) {
    const form = genZkBtn.closest('form');
    if (form) {
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        setButtonLoading(genZkBtn, true, i18nText('demo.zkLoading', 'Generating Groth16 Proof...'));
        if (window.showGlobalSpinner) {
          window.showGlobalSpinner(i18nText('demo.zkLoading', 'Generating Groth16 Proof...'));
        }

        const score = acuitySlider ? acuitySlider.value : '20';
        const zkContract = ContractRegistry.get('zk_verifier');
        let result = null;
        try {
          result = await SorobanRPC.invoke(
            'zk-proof',
            {
              patientAcuity: `20/${score}`,
              proofBytes: '0xPREPARED_ZK_PROOF_BYTES',
            },
            {
              delay: 1500,
              pendingMsg: `Transaction Pending: Verifying ZK proof on [zk_verifier] (${truncateMiddle(zkContract, 6, 6)})...`,
              successMsg: i18nText(
                'demo.zkSuccess',
                'Proof Verified Valid — On-Chain Confirmation Received'
              ),
              errorMsg: i18nText('demo.zkError', 'ZK Proof Verification Failed'),
            }
          );
        } catch (err) {
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              i18nText('demo.zkError', 'ZK Proof Verification Failed') +
                ': ' +
                (err.message || String(err))
            );
          }
        } finally {
          setButtonLoading(genZkBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        }

        if (result && result.status === 'success') {
          updateZKPreview(true);
          genZkBtn.textContent = i18nText('demo.zkVerified', '✅ Proof Verified Valid!');
          window.setTimeout(() => {
            genZkBtn.textContent = i18nText('demo.generateProof', 'Generate Groth16 zk-SNARK');
          }, 2500);
        } else {
          genZkBtn.textContent = i18nText('demo.generateProof', 'Generate Groth16 zk-SNARK');
        }
      });
    }
  }

  function updateZKPreview(generated = false) {
    const score = acuitySlider ? acuitySlider.value : '20';
    const isValid = parseInt(score, 10) <= 40;
    const proofHex = generated ? '0x98f21a007b82f10d9e4a8b71...' : '0xPREPARED_ZK_PROOF_BYTES';
    const zkContract = ContractRegistry.get('zk_verifier');

    if (previewZkCode) {
      previewZkCode.textContent = `{
  "circuit": "contracts/zk_verifier::visual_acuity_proof",
  "contract_address": "${zkContract}",
  "patient_private_acuity": "20/${score}",
  "public_threshold": "Visual Acuity <= 20/40",
  "proof_bytes": "${proofHex}",
  "evaluation_result": ${isValid ? '"PASS_VERIFIED"' : '"FAIL_REQUIREMENT_NOT_MET"'},
  "status": "${generated ? 'ONCHAIN_VERIFIED' : 'READY_TO_SUBMIT'}",
  "patient_identity_revealed": false
}`;
    }
  }
}

/* ==========================================================================
   AI Oracle Rotation Simulator
   ========================================================================== */
function initAISimulator() {
  const aiStatusSelect = document.getElementById('select-ai-status');
  const testAiBtn = document.getElementById('btn-test-ai');
  const previewAiCode = document.getElementById('preview-ai-code');

  if (testAiBtn) {
    const form = testAiBtn.closest('form');
    if (form) {
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        const status = aiStatusSelect ? aiStatusSelect.value : 'healthy';
        setButtonLoading(testAiBtn, true, i18nText('demo.aiLoading', 'Evaluating Diagnostic Oracles...'));
        if (window.showGlobalSpinner) {
          window.showGlobalSpinner(i18nText('demo.aiLoading', 'Evaluating Diagnostic Oracles...'));
        }

        try {
          const aiContract = ContractRegistry.get('ai_integration');
          const result = await SorobanRPC.invoke(
        const aiContract = ContractRegistry.get('ai_integration');
        try {
          await SorobanRPC.invoke(
            'ai-diagnostic',
            { status },
            {
              delay: 1800,
              pendingMsg: `Transaction Pending: AI oracle cross-contract call on [ai_integration] (${truncateMiddle(aiContract, 6, 6)})...`,
              successMsg: 'Diagnostic Complete — AI Oracle Response Confirmed',
              errorMsg: 'AI Oracle Call Failed',
              simulateFailure: status === 'timeout',
            }
          );
              successMsg: i18nText('demo.aiSuccess', 'Diagnostic Complete — AI Oracle Response Confirmed'),
              errorMsg: i18nText('demo.aiError', 'AI Oracle Call Failed'),
              simulateFailure: status === 'timeout',
            }
          );
        } catch (err) {
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              i18nText('demo.aiError', 'AI Oracle Call Failed') + ': ' + (err.message || String(err))
            );
          }
        } finally {
          setButtonLoading(testAiBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        }

          if (status === 'timeout') {
            if (previewAiCode) {
              previewAiCode.textContent = `[AI_INTEGRATION] Primary Provider "AI_Vision_Alpha" TIMEOUT (500ms Exceeded).
[FAILOVER_TRIGGERED] Executing contracts/ai_integration::rotate_provider()
[PROVIDER_ROTATION] Degrading "AI_Vision_Alpha" Weight: 100 -> 0 (Status: Paused)
[SECONDARY_PROMOTED] Activated Secondary Oracle: "AI_Vision_Beta" (Weight: 90)
[DIAGNOSTIC_RESULT] Analysis Complete via Backup Node.
[EVENT_EMITTED] ProviderRotated(Primary: "AI_Vision_Beta", Reason: SLA_Timeout)`;
            }
          } else {
            if (previewAiCode) {
              previewAiCode.textContent = `[AI_INTEGRATION] Primary Provider Active: "AI_Vision_Alpha" (Weight: 100)
          }
          ToastSystem.warning(i18nText('demo.aiPartial', 'Partial failure: Primary oracle timed out, but failover succeeded.'));
        } else if (previewAiCode) {
          previewAiCode.textContent = `[AI_INTEGRATION] Primary Provider Active: "AI_Vision_Alpha" (Weight: 100)
[ORACLE_CALL] Processing Retinal Scan CID: ipfs://QmX9z82...
[DIAGNOSTIC_RESULT] Diagnostics Confirmed: No Diabetic Retinopathy Detected.
[LATENCY] Response Time: 42ms (Within 100ms SLA Window)
[EVENT_EMITTED] ProviderStatusChecked(Active, Weight: 100)`;
            }
          }

          setButtonLoading(testAiBtn, false);

          if (result.status === 'error' && result.isPartialFailure) {
            ToastSystem.warning(
              'Partial failure: Primary oracle timed out, but failover succeeded.'
            );
          }

          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
          testAiBtn.textContent = 'Execute Diagnostic Check';
        } catch (err) {
          setButtonLoading(testAiBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
          testAiBtn.textContent = 'Execute Diagnostic Check';
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              'RPC Error: ' + (err.message || 'Unknown error during AI oracle evaluation.')
            );
          }
        }
        }

        testAiBtn.textContent = i18nText('demo.simulateDiagnostic', 'Simulate Diagnostic Request');
      });
    }
  }
}

/* ==========================================================================
   FHIR v4 Converter Simulator
   ========================================================================== */
function initFHIRSimulator() {
  const fhirTypeSelect = document.getElementById('select-fhir-type');
  const convertFhirBtn = document.getElementById('btn-convert-fhir');
  const previewFhirCode = document.getElementById('preview-fhir-code');

  if (convertFhirBtn) {
    const form = convertFhirBtn.closest('form');
    if (form) {
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        const type = fhirTypeSelect ? fhirTypeSelect.value : 'refraction';
        setButtonLoading(convertFhirBtn, true, i18nText('demo.fhirLoading', 'Mapping to FHIR v4 JSON...'));
        if (window.showGlobalSpinner) {
          window.showGlobalSpinner(i18nText('demo.fhirLoading', 'Mapping to FHIR v4 JSON...'));
        }

        try {
          const fhirContract = ContractRegistry.get('fhir');
          const result = await SorobanRPC.invoke(
        const fhirContract = ContractRegistry.get('fhir');
        let result = null;
        try {
          result = await SorobanRPC.invoke(
            'fhir-convert',
            { type },
            {
              delay: 1000,
              pendingMsg: `Transaction Pending: FHIR v4 converter on [fhir] (${truncateMiddle(fhirContract, 6, 6)})...`,
              successMsg: 'FHIR v4 Payload Generated — Contract Response Confirmed',
              errorMsg: 'FHIR Conversion Failed',
            }
          );
              successMsg: i18nText('demo.fhirSuccess', 'FHIR v4 Payload Generated — Contract Response Confirmed'),
              errorMsg: i18nText('demo.fhirError', 'FHIR Conversion Failed'),
            }
          );
        } catch (err) {
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              i18nText('demo.fhirError', 'FHIR Conversion Failed') + ': ' + (err.message || String(err))
            );
          }
        } finally {
          setButtonLoading(convertFhirBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        }

          const fhirEffectiveDate =
            typeof window !== 'undefined' &&
            window.RetinaXUtils &&
            window.RetinaXUtils.formatFHIRDate
              ? window.RetinaXUtils.formatFHIRDate(new Date())
              : '2026-08-06T23:45:00Z';

          if (type === 'iop') {
            if (previewFhirCode) {
              previewFhirCode.textContent = `{
  "resourceType": "Observation",
  "status": "final",
  "code": {
    "coding": [{
      "system": "http://loinc.org",
      "code": "56844-4",
      "display": "Intraocular pressure by Tonometry"
    }]
  },
  "subject": { "reference": "Patient/GDC...PATIENT_KEY_99X" },
  "valueQuantity": {
    "value": 14.5,
    "unit": "mmHg",
    "system": "http://unitsofmeasure.org"
  },
  "effectiveDateTime": "${fhirEffectiveDate}"
}`;
            }
          } else {
            if (previewFhirCode) {
              previewFhirCode.textContent = `{
          }
        } else if (previewFhirCode) {
          previewFhirCode.textContent = `{
  "resourceType": "DiagnosticReport",
  "status": "final",
  "code": {
    "coding": [{
      "system": "http://loinc.org",
      "code": "28886-0",
      "display": "Refraction examination"
    }]
  },
  "subject": { "reference": "Patient/GDC...PATIENT_KEY_99X" },
  "conclusion": "Normal refraction. Prescription: Right Eye -1.25 SPH, Left Eye -1.00 SPH",
  "effectiveDateTime": "${fhirEffectiveDate}"
}`;
            }
          }

          setButtonLoading(convertFhirBtn, false);

          if (result.status === 'success') {
            convertFhirBtn.textContent = '✅ Mapped to FHIR v4!';
            setTimeout(() => {
              convertFhirBtn.textContent = 'Map to FHIR v4 JSON';
            }, 2500);
          }

          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        } catch (err) {
          setButtonLoading(convertFhirBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
          convertFhirBtn.textContent = 'Map to FHIR v4 JSON';
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              'RPC Error: ' + (err.message || 'Unknown error during FHIR conversion.')
            );
          }
        }

        if (result && result.status === 'success') {
          convertFhirBtn.textContent = i18nText('demo.fhirMapped', '✅ Mapped to FHIR v4!');
          window.setTimeout(() => {
            convertFhirBtn.textContent = i18nText('demo.generateFhir', 'Generate FHIR v4 Payload');
          }, 2500);
        } else {
          convertFhirBtn.textContent = i18nText('demo.generateFhir', 'Generate FHIR v4 Payload');
        }
      });
    }
  }
}

/* ==========================================================================
   Clinical Data Fetch Simulator
   ========================================================================== */
function initDataFetchSimulator() {
  const fetchBtn = document.getElementById('btn-fetch-data');
  const fetchIdle = document.getElementById('fetch-idle');
  const fetchPlaceholder = document.getElementById('fetch-placeholder');
  const fetchResult = document.getElementById('fetch-result-container');
  const fetchStatusLabel = document.getElementById('fetch-status-label');
  const fetchStatusIndicator = document.getElementById('fetch-status-indicator');

  if (fetchBtn) {
    const form = fetchBtn.closest('form');
    if (form) {
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        if (fetchIdle) fetchIdle.style.display = 'none';
        if (fetchResult) fetchResult.style.display = 'none';
        if (fetchPlaceholder) fetchPlaceholder.style.display = 'flex';

        setButtonLoading(fetchBtn, true, i18nText('demo.fetchLoading', 'Fetching from IPFS...'));
        if (window.showGlobalSpinner) {
          window.showGlobalSpinner(i18nText('demo.fetchLoading', 'Fetching from IPFS...'));
        }
        if (fetchStatusLabel) {
          fetchStatusLabel.textContent = i18nText('demo.fetchProgress', 'DATA_RETRIEVAL IN_PROGRESS');
        }
        if (fetchStatusIndicator) {
          fetchStatusIndicator.textContent = i18nText('demo.fetchCid', 'FETCHING_CID');
        }

        try {
          // 2. Route data fetch via SorobanRPC and ContractRegistry
          const visionContract = ContractRegistry.get('vision_records');
          const result = await SorobanRPC.invoke(
        const visionContract = ContractRegistry.get('vision_records');
        let result = null;
        try {
          result = await SorobanRPC.invoke(
            'data-fetch',
            {},
            {
              delay: 2500,
              pendingMsg: `Transaction Pending: Fetching encrypted data from [vision_records] (${truncateMiddle(visionContract, 6, 6)})...`,
              successMsg: 'Clinical Data Retrieved — Decryption Complete',
              errorMsg: 'Data Fetch Failed',
              failureRate: 0.1,
            }
          );

          setButtonLoading(fetchBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();

          if (result.status === 'success') {
            // 3. Hide placeholder, show result with smooth transition
            if (fetchPlaceholder) fetchPlaceholder.style.display = 'none';
            if (fetchResult) {
              fetchResult.style.display = 'block';
              fetchResult.classList.add('skeleton-fade-in');
            }

            if (fetchStatusLabel) fetchStatusLabel.textContent = 'DATA_RETRIEVAL SUCCESS';
            if (fetchStatusIndicator) fetchStatusIndicator.textContent = 'DECRYPTED_PAYLOAD';

            if (typeof gsap !== 'undefined' && fetchResult) {
              gsap.fromTo(
                fetchResult,
                { opacity: 0, y: 10 },
                { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }
              );
            }
          } else {
            // Handle partial failure gracefully
            if (fetchPlaceholder) fetchPlaceholder.style.display = 'none';
            if (fetchIdle) fetchIdle.style.display = 'block';

            if (fetchStatusLabel) fetchStatusLabel.textContent = 'DATA_RETRIEVAL FAILED';
            if (fetchStatusIndicator) fetchStatusIndicator.textContent = 'RETRY_AVAILABLE';

            if (result.isPartialFailure) {
              ToastSystem.warning(
                'Partial data retrieved: Some records may be incomplete. Retry recommended.'
              );
            }
          }
        } catch (err) {
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
          setButtonLoading(fetchBtn, false);
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              'RPC Error: ' + (err.message || 'Unknown error during data fetch.')
            );
          }
              successMsg: i18nText('demo.fetchSuccess', 'Clinical Data Retrieved — Decryption Complete'),
              errorMsg: i18nText('demo.fetchError', 'Data Fetch Failed'),
              failureRate: 0.1,
            }
          );
        } catch (err) {
          if (window.showErrorFeedback) {
            window.showErrorFeedback(
              i18nText('demo.fetchError', 'Data Fetch Failed') + ': ' + (err.message || String(err))
            );
          }
        } finally {
          setButtonLoading(fetchBtn, false);
          if (window.hideGlobalSpinner) window.hideGlobalSpinner();
        }

        if (result && result.status === 'success') {
          if (fetchPlaceholder) fetchPlaceholder.style.display = 'none';
          if (fetchResult) fetchResult.style.display = 'block';
          if (fetchStatusLabel) {
            fetchStatusLabel.textContent = i18nText('demo.fetchSuccessStatus', 'DATA_RETRIEVAL SUCCESS');
          }
          if (fetchStatusIndicator) {
            fetchStatusIndicator.textContent = i18nText('demo.fetchDecrypted', 'DECRYPTED_PAYLOAD');
          }
          if (typeof gsap !== 'undefined' && fetchResult) {
            gsap.fromTo(
              fetchResult,
              { opacity: 0, y: 10 },
              { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }
            );
          }
        } else {
          if (fetchPlaceholder) fetchPlaceholder.style.display = 'none';
          if (fetchIdle) fetchIdle.style.display = 'block';
          if (fetchStatusLabel) {
            fetchStatusLabel.textContent = i18nText('demo.fetchFailedStatus', 'DATA_RETRIEVAL FAILED');
          }
          if (fetchStatusIndicator) {
            fetchStatusIndicator.textContent = i18nText('demo.fetchRetry', 'RETRY_AVAILABLE');
          }
          if (result && result.isPartialFailure) {
            ToastSystem.warning(
              i18nText('demo.fetchPartial', 'Partial data retrieved: Some records may be incomplete. Retry recommended.')
            );
          }
        }

          fetchBtn.textContent = i18nText('demo.fetchButton', 'Fetch Clinical Data');
        }
      });
    }
  }
}

/* ==========================================================================
   <modal-dialog> Custom Web Component Base Component
   ========================================================================== */
const ModalBase = typeof HTMLElement !== 'undefined' ? HTMLElement : class {};

class ModalDialog extends ModalBase {
  static get observedAttributes() {
    return ['open', 'title', 'size', 'closable'];
  }

  constructor() {
    super();
    this.handleKeydown = this.handleKeydown.bind(this);
    this.handleOverlayClick = this.handleOverlayClick.bind(this);
  }

  connectedCallback() {
    this.render();
    this.setupListeners();
    this.updateVisibility();
  }

  disconnectedCallback() {
    document.removeEventListener('keydown', this.handleKeydown);
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (oldValue === newValue) return;
    if (name === 'open') {
      this.updateVisibility();
    } else if (name === 'title') {
      const titleEl = this.querySelector('.modal-dialog-title');
      if (titleEl) titleEl.textContent = newValue || '';
    } else if (name === 'size') {
      const cardEl = this.querySelector('.modal-dialog-card');
      if (cardEl) {
        cardEl.className = `modal-dialog-card size-${newValue || 'md'}`;
      }
    }
  }

  get open() {
    return this.hasAttribute('open');
  }

  set open(val) {
    if (val) {
      this.setAttribute('open', '');
    } else {
      this.removeAttribute('open');
    }
  }

  get title() {
    return this.getAttribute('title') || '';
  }

  set title(val) {
    this.setAttribute('title', val);
  }

  get size() {
    return this.getAttribute('size') || 'md';
  }

  set size(val) {
    this.setAttribute('size', val);
  }

  get closable() {
    return !this.hasAttribute('closable') || this.getAttribute('closable') !== 'false';
  }

  show() {
    this.open = true;
  }

  openModal() {
    this.open = true;
  }

  close() {
    this.open = false;
  }

  closeModal() {
    this.open = false;
  }

  toggle() {
    this.open = !this.open;
  }

  updateVisibility() {
    const overlay = this.querySelector('.modal-dialog-overlay');
    if (!overlay) return;

    if (this.open) {
      overlay.classList.add('active');
      document.body.style.overflow = 'hidden';
      document.addEventListener('keydown', this.handleKeydown);
      this.dispatchEvent(new CustomEvent('modal-open', { bubbles: true, composed: true }));
    } else {
      overlay.classList.remove('active');
      document.body.style.overflow = '';
      document.removeEventListener('keydown', this.handleKeydown);
      this.dispatchEvent(new CustomEvent('modal-close', { bubbles: true, composed: true }));
    }
  }

  handleKeydown(e) {
    if (e.key === 'Escape' && this.open && this.closable) {
      this.close();
    }
  }

  handleOverlayClick(e) {
    if (e.target.classList.contains('modal-dialog-overlay') && this.closable) {
      this.close();
    }
  }

  render() {
    if (this.querySelector('.modal-dialog-overlay')) return;

    const titleText = this.title;
    const sizeAttr = this.size;
    const isClosable = this.closable;

    const bodyContent = this.innerHTML;

    this.innerHTML = `
      <div class="modal-dialog-overlay" role="dialog" aria-modal="true" aria-label="${titleText}">
        <div class="modal-dialog-card size-${sizeAttr}">
          <div class="modal-dialog-header">
            <h3 class="modal-dialog-title">${titleText}</h3>
            ${isClosable ? `<button class="modal-dialog-close-btn" aria-label="Close modal">✕</button>` : ''}
          </div>
          <div class="modal-dialog-body">
            ${bodyContent}
          </div>
        </div>
      </div>
    `;
  }

  setupListeners() {
    const overlay = this.querySelector('.modal-dialog-overlay');
    const closeBtn = this.querySelector('.modal-dialog-close-btn');

    if (overlay) {
      overlay.addEventListener('click', this.handleOverlayClick);
    }

    if (closeBtn) {
      closeBtn.addEventListener('click', () => this.close());
    }
  }
}

if (typeof customElements !== 'undefined' && !customElements.get('modal-dialog')) {
  customElements.define('modal-dialog', ModalDialog);
}

/* ==========================================================================
   Modal Base Logic (Connect Wallet & Custom <modal-dialog> Triggering)
   ========================================================================== */
function initModal() {
  const modalOverlay = document.getElementById('modal-overlay');
  const customModal = document.getElementById('connect-wallet-dialog');
  const btnConnect = document.getElementById('btn-connect-wallet');
  const btnClose = document.getElementById('modal-close-btn');

  function openModal() {
    if (customModal && typeof customModal.openModal === 'function') {
      customModal.openModal();
    } else if (modalOverlay) {
      modalOverlay.classList.add('active');
      document.body.style.overflow = 'hidden';
    }
  }

  function closeModal() {
    if (customModal && typeof customModal.closeModal === 'function') {
      customModal.closeModal();
    } else if (modalOverlay) {
      modalOverlay.classList.remove('active');
      document.body.style.overflow = '';
    }
  }

  if (btnConnect) {
    btnConnect.addEventListener('wallet-connect', (e) => {
      e.preventDefault();
      openModal();
    });

    btnConnect.addEventListener('wallet-disconnect', (e) => {
      e.preventDefault();
      FreighterWallet.disconnect();
    });
  }

  if (btnClose) {
    btnClose.addEventListener('click', closeModal);
  }

  if (modalOverlay) {
    modalOverlay.addEventListener('click', (e) => {
      if (e.target === modalOverlay) {
        closeModal();
      }
    });
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modalOverlay && modalOverlay.classList.contains('active')) {
      closeModal();
    }
  });
}

/* ==========================================================================
   Freighter Wallet UI (Issue #283)
   Keeps the Connect Wallet button, network pill and wallet dialog in sync with FreighterWallet.
   ========================================================================== */
function initWallet() {
  const button = document.getElementById('btn-connect-wallet');
  const dialog = document.getElementById('connect-wallet-dialog');
  const freighterOption = document.getElementById('wallet-option-freighter');
  const installPrompt = document.getElementById('wallet-install-prompt');
  const status = document.getElementById('wallet-status');
  const networkSelect = document.getElementById('wallet-network-select');
  const networkPill = document.querySelector('.network-pill');
  const networkPillLabel = document.getElementById('network-pill-label');
  let wasConnected = false;
  let hadMismatch = false;

  function setStatus(message) {
    if (!status) return;
    status.textContent = message;
    status.hidden = !message;
  }

  function render(state) {
    const label = networkLabel(state.network);
    const mismatch = i18nText(
      'wallet.mismatch',
      'Freighter is set to a different network. Switch it to {network} in the extension.',
      { network: label }
    );
    if (networkSelect) networkSelect.value = state.network;
    if (networkPillLabel) networkPillLabel.textContent = label;
    if (networkPill) {
      networkPill.setAttribute(
        'aria-label',
        i18nText('wallet.networkStatus', 'Network status: {network}', { network: label })
      );
    }
    if (button) {
      button.network = label;
      if (state.connected) button.connect(state.address);
      else if (button.connected) button.disconnect();
    }
    setStatus(state.networkMismatch ? mismatch : '');

    if (state.networkMismatch && !hadMismatch) ToastSystem.warning(mismatch);
    if (wasConnected && !state.connected) {
      ToastSystem.warning(i18nText('wallet.disconnected', 'Wallet disconnected'));
    }
    wasConnected = state.connected;
    hadMismatch = state.networkMismatch;
  }

  // Shows the install prompt when the extension is missing.
  async function checkFreighter() {
    const installed = await FreighterWallet.isInstalled();
    if (installPrompt) installPrompt.hidden = installed;
    return installed;
  }

  async function connectFreighter() {
    if (!(await checkFreighter())) return;
    setStatus(i18nText('wallet.connecting', 'Approve the connection in Freighter...'));
    if (button) button.setAttribute('loading', '');
    try {
      const address = await FreighterWallet.connect();
      ToastSystem.success(
        i18nText('wallet.connectedAria', 'Wallet connected: {address}', {
          address: truncateMiddle(address, 6, 4),
        })
      );
      if (!FreighterWallet.hasNetworkMismatch() && dialog && dialog.closeModal) dialog.closeModal();
    } catch (err) {
      const reason = err.message || String(err);
      setStatus(`${i18nText('wallet.connectFailed', 'Could not connect Freighter')}: ${reason}`);
      if (window.showErrorFeedback) window.showErrorFeedback(reason);
    } finally {
      if (button) button.removeAttribute('loading');
    }
  }

  const storedNetwork = readStoredNetwork();
  if (storedNetwork) FreighterWallet.network = storedNetwork;
  FreighterWallet.subscribe(render);
  render(FreighterWallet.getState());

  if (dialog) dialog.addEventListener('modal-open', checkFreighter);
  if (freighterOption) {
    freighterOption.addEventListener('click', connectFreighter);
    freighterOption.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter' && e.key !== ' ') return;
      e.preventDefault();
      connectFreighter();
    });
  }
  if (networkSelect) {
    networkSelect.addEventListener('change', (e) => {
      FreighterWallet.setNetwork(e.target.value);
      writeStoredNetwork(e.target.value);
    });
  }
  // The i18n loader resets translated text, including the network pill, on language change.
  document.addEventListener('retinax:languagechange', () => render(FreighterWallet.getState()));
}

/* ==========================================================================
   Site Navigation Accessibility & Active States
   ========================================================================== */
function initNavigationA11y() {
  const navLinks = document.querySelectorAll('.nav-links .nav-link');
  if (!navLinks.length) return;

  // Click handler to set aria-current="page"
  navLinks.forEach((link) => {
    link.addEventListener('click', () => {
      navLinks.forEach((l) => l.removeAttribute('aria-current'));
      link.setAttribute('aria-current', 'page');
    });
  });

  // Track active section on scroll
  const sectionIds = ['overview', 'privacy', 'ai-oracle', 'demo', 'contracts'];
  const sections = sectionIds.map((id) => document.getElementById(id)).filter(Boolean);

  if ('IntersectionObserver' in window && sections.length) {
    const observer = new window.IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            const currentId = entry.target.id;
            navLinks.forEach((link) => {
              const href = link.getAttribute('href');
              if (href === `#${currentId}`) {
                link.setAttribute('aria-current', 'page');
              } else {
                link.removeAttribute('aria-current');
              }
            });
          }
        });
      },
      {
        rootMargin: '-20% 0px -70% 0px',
        threshold: 0,
      }
    );

    sections.forEach((sec) => observer.observe(sec));
  }
}

/* ==========================================================================
   Module Exports (CommonJS / Node and Browser global window.RetinaX)
   ========================================================================== */
if (typeof module !== 'undefined' && module.exports) {
  module.exports = {
    DEFAULT_CONTRACT_REGISTRY,
    DEFAULT_ACTION_ROUTES,
    ContractRegistry,
    ContractRouter,
    SorobanRPC,
    CrossContractCall,
    ToastSystem,
    showCCStatus,
    hideCCStatus,
    setButtonLoading,
    truncateMiddle,
    initDemoTabs,
    initRBACSimulator,
    initZKSimulator,
    initAISimulator,
    initFHIRSimulator,
    initDataFetchSimulator,
    STELLAR_NETWORKS,
    FreighterWallet,
    SorobanClient,
    AccessControlClient,
    describeAccessControlError,
    initWallet,
  };
} else if (typeof window !== 'undefined') {
  window.RetinaX = Object.assign(window.RetinaX || {}, {
    DEFAULT_CONTRACT_REGISTRY,
    DEFAULT_ACTION_ROUTES,
    ContractRegistry,
    ContractRouter,
    SorobanRPC,
    CrossContractCall,
    ToastSystem,
    showCCStatus,
    hideCCStatus,
    setButtonLoading,
    truncateMiddle,
    initDemoTabs,
    initRBACSimulator,
    initZKSimulator,
    initAISimulator,
    initFHIRSimulator,
    initDataFetchSimulator,
    STELLAR_NETWORKS,
    FreighterWallet,
    SorobanClient,
    AccessControlClient,
    describeAccessControlError,
    initWallet,
  });
}
