/* ==========================================================================
   Freighter wallet (#283) and access_control Soroban client (#284) tests
   Runs with Node.js test runner: node --test website/wallet.test.js
   ========================================================================== */

const { test, describe, beforeEach } = require('node:test');
const assert = require('node:assert/strict');

const ADMIN = 'GDZRQ4BDEZLU4S7PZ7M3B5YDJHKGKUI27QPBKNVD3MWXWGA3KU7E7RKO';
const USER = 'GBFNRDONXDU6ETMXV4N7WD6K74NNNJ26VRO2SSXBBLCZMREUG427JCHE';
const TESTNET = 'Test SDF Network ; September 2015';
const PUBLIC = 'Public Global Stellar Network ; September 2015';

/** Fake @stellar/freighter-api with the response shapes documented at docs.freighter.app. */
function fakeFreighter({ installed = true, address = ADMIN, passphrase = TESTNET } = {}) {
  const api = {
    signed: [],
    watcher: null,
    isConnected: async () => ({ isConnected: installed }),
    requestAccess: async () => ({ address }),
    getNetwork: async () => ({ network: 'TESTNET', networkPassphrase: passphrase }),
    getNetworkDetails: async () => ({
      networkPassphrase: passphrase,
      sorobanRpcUrl: 'https://rpc.wallet.test',
    }),
    signTransaction: async (xdr, opts) => {
      api.signed.push({ xdr, opts });
      return { signedTxXdr: `signed:${xdr}`, signerAddress: opts.address };
    },
    WatchWalletChanges: class {
      constructor(timeout) {
        this.timeout = timeout;
        this.stopped = false;
        api.watcher = this;
      }
      watch(callback) {
        this.callback = callback;
      }
      stop() {
        this.stopped = true;
      }
    },
  };
  return api;
}

/** Fake @stellar/stellar-sdk recording the calls SorobanClient makes. */
function fakeSdk(overrides = {}) {
  const log = [];
  const server = {
    simulateTransaction: async (tx) => (
      log.push(['simulate', tx]),
      { result: { retval: { native: 3 } } }
    ),
    getAccount: async (address) => (
      log.push(['getAccount', address]),
      { id: address, sequence: '7' }
    ),
    prepareTransaction: async (tx) => (
      log.push(['prepare', tx]),
      { ...tx, toXDR: () => 'prepared-xdr' }
    ),
    sendTransaction: async (tx) => (log.push(['send', tx]), { status: 'PENDING', hash: 'abc123' }),
    pollTransaction: async (hash, opts) => (
      log.push(['poll', hash, opts]),
      { status: 'SUCCESS', ledger: 42 }
    ),
    ...overrides,
  };
  const sdk = {
    log,
    BASE_FEE: '100',
    rpc: {
      Server: class {
        constructor(url) {
          log.push(['server', url]);
          return server;
        }
      },
      Api: {
        isSimulationError: (sim) => typeof sim.error === 'string',
        GetTransactionStatus: { SUCCESS: 'SUCCESS', NOT_FOUND: 'NOT_FOUND', FAILED: 'FAILED' },
      },
    },
    Account: class {
      constructor(id, sequence) {
        Object.assign(this, { id, sequence });
      }
    },
    Contract: class {
      constructor(id) {
        this.id = id;
      }
      call(method, ...args) {
        return { contract: this.id, method, args };
      }
    },
    TransactionBuilder: class {
      constructor(source, opts) {
        Object.assign(this, { source, opts, ops: [] });
      }
      addOperation(op) {
        this.ops.push(op);
        return this;
      }
      setTimeout(seconds) {
        this.timeout = seconds;
        return this;
      }
      build() {
        return { source: this.source, opts: this.opts, ops: this.ops, timeout: this.timeout };
      }
      static fromXDR(xdr, passphrase) {
        return { xdr, passphrase };
      }
    },
    nativeToScVal: (value, { type }) => ({ value, type }),
    scValToNative: (scVal) => scVal.native,
    StrKey: { isValidEd25519PublicKey: (address) => /^G[A-Z2-7]{55}$/.test(address) },
  };
  return sdk;
}

function loadApp(windowProps) {
  global.window = {
    RETINAX_DEPLOYMENTS: {
      testnet: {
        rpcUrl: 'https://soroban-testnet.stellar.org',
        contracts: { access_control: 'CACCESS' },
      },
      mainnet: { rpcUrl: '', contracts: { access_control: 'CMAIN' } },
    },
    ...windowProps,
  };
  delete require.cache[require.resolve('./app.js')];
  return require('./app.js');
}

describe('Freighter wallet connection (#283)', () => {
  let freighter;
  let app;

  beforeEach(() => {
    freighter = fakeFreighter();
    app = loadApp({ freighterApi: freighter });
  });

  test('detects whether the Freighter extension is installed', async () => {
    assert.equal(await app.FreighterWallet.isInstalled(), true);

    app = loadApp({ freighterApi: fakeFreighter({ installed: false }) });
    assert.equal(await app.FreighterWallet.isInstalled(), false);
    await assert.rejects(app.FreighterWallet.connect(), { code: 'not-installed' });

    app = loadApp({});
    assert.equal(await app.FreighterWallet.isInstalled(), false);
  });

  test('connect requests access and publishes the connected address', async () => {
    const states = [];
    app.FreighterWallet.subscribe((state) => states.push(state));

    assert.equal(await app.FreighterWallet.connect(), ADMIN);
    assert.deepEqual(states.at(-1), {
      address: ADMIN,
      connected: true,
      network: 'testnet',
      networkMismatch: false,
    });
    assert.equal(freighter.watcher.timeout, 3000);
  });

  test('a rejected access request leaves the wallet disconnected', async () => {
    freighter.requestAccess = async () => ({
      address: '',
      error: { code: -4, message: 'The user rejected this request.' },
    });
    await assert.rejects(app.FreighterWallet.connect(), {
      code: 'rejected',
      message: 'The user rejected this request.',
    });
    assert.equal(app.FreighterWallet.getState().connected, false);
  });

  test('flags a network mismatch and signs with the selected network passphrase', async () => {
    app = loadApp({ freighterApi: (freighter = fakeFreighter({ passphrase: PUBLIC })) });
    await app.FreighterWallet.connect();
    assert.equal(app.FreighterWallet.hasNetworkMismatch(), true);
    await assert.rejects(app.FreighterWallet.signTransaction('xdr'), { code: 'network-mismatch' });

    app.FreighterWallet.setNetwork('mainnet');
    assert.equal(app.FreighterWallet.hasNetworkMismatch(), false);
    assert.equal(await app.FreighterWallet.signTransaction('xdr'), 'signed:xdr');
    assert.deepEqual(freighter.signed[0].opts, { networkPassphrase: PUBLIC, address: ADMIN });
    assert.throws(() => app.FreighterWallet.setNetwork('futurenet'), TypeError);
  });

  test('disconnects when the wallet locks or the site access is revoked', async () => {
    await app.FreighterWallet.connect();
    freighter.watcher.callback({ address: '', network: '', networkPassphrase: '' });

    assert.equal(app.FreighterWallet.getState().connected, false);
    assert.equal(freighter.watcher.stopped, true);
    await assert.rejects(app.FreighterWallet.signTransaction('xdr'), { code: 'not-connected' });
  });

  test('follows account and network switches made in the extension', async () => {
    await app.FreighterWallet.connect();
    freighter.watcher.callback({ address: USER, network: 'PUBLIC', networkPassphrase: PUBLIC });

    assert.equal(app.FreighterWallet.address, USER);
    assert.equal(app.FreighterWallet.hasNetworkMismatch(), true);
  });

  test('disconnect ends the session and stops watching the extension', async () => {
    await app.FreighterWallet.connect();
    app.FreighterWallet.disconnect();
    assert.equal(app.FreighterWallet.getState().connected, false);
    assert.equal(freighter.watcher.stopped, true);
  });
});

describe('access_control contract calls over Soroban RPC (#284)', () => {
  test('get_role simulates a read-only call and decodes the u32 role', async () => {
    const sdk = fakeSdk();
    const app = loadApp({ StellarSdk: sdk });

    assert.deepEqual(await app.AccessControlClient.getRole('testnet', USER), {
      contractId: 'CACCESS',
      role: 3,
    });
    assert.deepEqual(sdk.log[0], ['server', 'https://soroban-testnet.stellar.org']);
    const [, tx] = sdk.log[1];
    assert.equal(tx.opts.networkPassphrase, TESTNET);
    assert.deepEqual({ ...tx.source }, { id: USER, sequence: '0' });
    assert.deepEqual(tx.ops, [
      { contract: 'CACCESS', method: 'get_role', args: [{ value: USER, type: 'address' }] },
    ]);
    // Reads never sign or submit.
    assert.equal(sdk.log.length, 2);
  });

  test('has_permission passes the permission as a u32', async () => {
    const sdk = fakeSdk({
      simulateTransaction: async (tx) => (
        sdk.log.push(['simulate', tx]),
        { result: { retval: { native: true } } }
      ),
    });
    const app = loadApp({ StellarSdk: sdk });

    const result = await app.AccessControlClient.hasPermission('testnet', USER, 2);
    assert.deepEqual(result, { contractId: 'CACCESS', granted: true });
    assert.deepEqual(sdk.log[1][1].ops[0].args, [
      { value: USER, type: 'address' },
      { value: 2, type: 'u32' },
    ]);
  });

  test('assign_role is prepared, signed by Freighter, submitted and polled', async () => {
    const sdk = fakeSdk();
    const freighter = fakeFreighter();
    const app = loadApp({ StellarSdk: sdk, freighterApi: freighter });
    await app.FreighterWallet.connect();

    const result = await app.AccessControlClient.assignRole('testnet', USER, 3, 1_800_000_000);
    assert.deepEqual(result, { contractId: 'CACCESS', hash: 'abc123', ledger: 42 });

    assert.deepEqual(
      sdk.log.map(([step]) => step),
      ['server', 'getAccount', 'prepare', 'send', 'poll']
    );
    const prepared = sdk.log[2][1];
    assert.deepEqual(prepared.source, { id: ADMIN, sequence: '7' });
    assert.deepEqual(prepared.ops[0].args, [
      { value: ADMIN, type: 'address' },
      { value: USER, type: 'address' },
      { value: 3, type: 'u32' },
      { value: 1_800_000_000n, type: 'u64' },
    ]);
    assert.deepEqual(freighter.signed[0], {
      xdr: 'prepared-xdr',
      opts: { networkPassphrase: TESTNET, address: ADMIN },
    });
    assert.deepEqual(sdk.log[3][1], { xdr: 'signed:prepared-xdr', passphrase: TESTNET });
    assert.deepEqual(sdk.log[4], ['poll', 'abc123', { attempts: 30 }]);
  });

  test('assign_role requires a connected wallet', async () => {
    const app = loadApp({ StellarSdk: fakeSdk(), freighterApi: fakeFreighter() });
    await assert.rejects(app.AccessControlClient.assignRole('testnet', USER, 3, 1), {
      code: 'not-connected',
    });
  });

  test('rejected and failed transactions are reported', async () => {
    const freighter = fakeFreighter();
    let app = loadApp({
      StellarSdk: fakeSdk({ sendTransaction: async () => ({ status: 'ERROR', hash: 'h1' }) }),
      freighterApi: freighter,
    });
    await app.FreighterWallet.connect();
    await assert.rejects(
      app.AccessControlClient.assignRole('testnet', USER, 3, 1),
      /did not accept the transaction \(ERROR\)/
    );

    app = loadApp({
      StellarSdk: fakeSdk({ pollTransaction: async () => ({ status: 'FAILED' }) }),
      freighterApi: fakeFreighter(),
    });
    await app.FreighterWallet.connect();
    await assert.rejects(
      app.AccessControlClient.assignRole('testnet', USER, 3, 1),
      /abc123 failed/
    );

    app = loadApp({
      StellarSdk: fakeSdk({
        getAccount: async (address) => {
          throw new Error(`Account not found: ${address}`);
        },
      }),
      freighterApi: fakeFreighter(),
    });
    await app.FreighterWallet.connect();
    await assert.rejects(
      app.AccessControlClient.assignRole('testnet', USER, 3, 1),
      /has no account on Stellar Testnet/
    );
  });

  test('simulation errors, bad addresses and missing configuration are reported', async () => {
    let app = loadApp({
      StellarSdk: fakeSdk({
        simulateTransaction: async () => ({ error: 'HostError: Error(Contract, #3)' }),
      }),
    });
    await assert.rejects(app.AccessControlClient.getRole('testnet', USER), /Error\(Contract, #3\)/);
    await assert.rejects(
      app.AccessControlClient.getRole('testnet', 'GDC...PATIENT'),
      /Not a valid Stellar account address/
    );

    app = loadApp({ StellarSdk: fakeSdk() });
    window.RETINAX_DEPLOYMENTS.testnet.contracts.access_control = '';
    await assert.rejects(
      app.AccessControlClient.getRole('testnet', USER),
      /No access_control contract is configured for Stellar Testnet/
    );

    // Mainnet has no RPC URL configured and no wallet to borrow one from.
    await assert.rejects(
      app.AccessControlClient.getRole('mainnet', USER),
      /No Soroban RPC URL is configured for Stellar Mainnet/
    );

    app = loadApp({});
    await assert.rejects(
      app.AccessControlClient.getRole('testnet', USER),
      /The Stellar SDK did not load/
    );
  });

  test('mainnet falls back to the Soroban RPC URL configured in Freighter', async () => {
    const sdk = fakeSdk();
    const app = loadApp({ StellarSdk: sdk, freighterApi: fakeFreighter({ passphrase: PUBLIC }) });
    app.FreighterWallet.setNetwork('mainnet');
    await app.FreighterWallet.connect();

    await app.AccessControlClient.getRole('mainnet', USER);
    assert.deepEqual(sdk.log[0], ['server', 'https://rpc.wallet.test']);
    assert.equal(sdk.log[1][1].opts.networkPassphrase, PUBLIC);
  });

  test('contract error codes are named after AccessControlError', () => {
    const app = loadApp({});
    assert.equal(
      app.describeAccessControlError(new Error('HostError: Error(Contract, #6)\nEvent log...')),
      'access_control error AlreadyExists (Error(Contract, #6))'
    );
    assert.equal(app.describeAccessControlError(new Error('network down')), 'network down');
  });
});
