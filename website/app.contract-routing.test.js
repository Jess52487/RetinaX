const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function loadContractRouting() {
  const window = {};
  const context = {
    window,
    document: { addEventListener() {} },
    Map,
    Object,
    Error,
  };
  vm.runInNewContext(fs.readFileSync(require.resolve('./app.js'), 'utf8'), context);
  return window;
}

test('routes actions to their matching micro-contract address', async () => {
  const app = loadContractRouting();
  const calls = [];
  app.RETINAX_CONTRACTS = {
    audit: 'CAUDIT',
    identity: 'CIDENTITY',
    key_manager: 'CKEYS',
    vision_records: 'CRECORDS',
  };
  app.RETINAX_RPC_URL = 'https://rpc.example.test';
  app.RetinaXRPC = { invokeContract: async (call) => (calls.push(call), 'submitted') };

  assert.equal(
    await app.invokeRetinaXContractAction('grant_access', { patient: 'G1' }),
    'submitted'
  );
  assert.equal(await app.invokeRetinaXContractAction('verify_identity'), 'submitted');
  assert.equal(await app.invokeRetinaXContractAction('rotate_key'), 'submitted');
  assert.equal(await app.invokeRetinaXContractAction('append_audit'), 'submitted');
  assert.deepEqual(
    calls.map(({ contract, contractId }) => [contract, contractId]),
    [
      ['vision_records', 'CRECORDS'],
      ['identity', 'CIDENTITY'],
      ['key_manager', 'CKEYS'],
      ['audit', 'CAUDIT'],
    ]
  );
  assert.equal(calls[0].rpcUrl, 'https://rpc.example.test');
});

test('leaves the simulator usable when no RPC adapter is installed', async () => {
  const app = loadContractRouting();
  assert.equal(await app.invokeRetinaXContractAction('grant_access'), null);
});

test('rejects unknown actions and missing deployed addresses', async () => {
  const app = loadContractRouting();
  app.RetinaXRPC = { invokeContract: async () => 'submitted' };
  await assert.rejects(
    app.invokeRetinaXContractAction('unknown_action'),
    /Unsupported contract action/
  );
  await assert.rejects(
    app.invokeRetinaXContractAction('grant_access'),
    /No deployed address configured for vision_records/
  );
});
