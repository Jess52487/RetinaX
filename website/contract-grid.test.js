'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const grid = require('./components/contract-grid.js');
const root = __dirname;
const repoRoot = path.resolve(root, '..');
const en = JSON.parse(fs.readFileSync(path.join(root, 'i18n', 'en.json'), 'utf8'));
const es = JSON.parse(fs.readFileSync(path.join(root, 'i18n', 'es.json'), 'utf8'));
const contractDirs = fs
  .readdirSync(path.join(repoRoot, 'contracts'), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .filter((name) => !['benches', 'tests', 'timelock'].includes(name));

test('registry covers every workspace contract directory', () => {
  const registered = new Set(grid.CONTRACTS.map((contract) => contract.id));
  for (const directory of contractDirs) {
    assert.ok(registered.has(directory), `contract not registered: ${directory}`);
  }
  for (const contract of grid.CONTRACTS) {
    assert.ok(contractDirs.includes(contract.id), `registry entry has no contract directory: ${contract.id}`);
  }
});

test('registry exposes all 24 contracts with unique ids and source paths', () => {
  assert.equal(grid.CONTRACTS.length, 24);
  assert.equal(new Set(grid.CONTRACTS.map((contract) => contract.id)).size, 24);
  for (const contract of grid.CONTRACTS) {
    assert.equal(contract.path, `contracts/${contract.id}`);
    assert.ok(contract.name && contract.description, `incomplete metadata: ${contract.id}`);
    assert.ok(fs.existsSync(path.join(repoRoot, contract.path)), `missing source path: ${contract.path}`);
  }
});

test('all category filters are represented and partition the registry', () => {
  const expected = ['core', 'security', 'governance', 'clinical', 'infrastructure'];
  const actual = grid.CATEGORIES.map((category) => category.id).filter((id) => id !== 'all');
  assert.deepEqual(actual, expected);

  let total = 0;
  for (const category of expected) {
    const contracts = grid.filterContracts(grid.CONTRACTS, category);
    assert.ok(contracts.length > 0, `empty category: ${category}`);
    assert.ok(contracts.every((contract) => contract.category === category));
    total += contracts.length;
  }
  assert.equal(total, grid.CONTRACTS.length);
  assert.equal(grid.filterContracts(grid.CONTRACTS, 'all').length, grid.CONTRACTS.length);
});

test('deployment status resolves configured, demo, and undeployed contracts', () => {
  const contract = grid.CONTRACTS.find((entry) => entry.id === 'vision_records');
  assert.deepEqual(grid.resolveContractStatus(contract, {}, {}), {
    status: 'not-deployed',
    address: '',
  });
  assert.deepEqual(
    grid.resolveContractStatus(contract, { vision_records: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM' }, {}),
    {
      status: 'deployed',
      address: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM',
    }
  );
  assert.deepEqual(grid.resolveContractStatus(contract, {}, { vision_records: 'CDEMO' }), {
    status: 'demo',
    address: 'CDEMO',
  });
});

test('documentation links point at existing files', () => {
  for (const contract of grid.CONTRACTS) {
    if (!contract.docsPath) continue;
    assert.ok(
      fs.existsSync(path.join(repoRoot, contract.docsPath)),
      `missing docs for ${contract.id}: ${contract.docsPath}`
    );
  }
});

test('each contract has a name and description in both locales', () => {
  for (const contract of grid.CONTRACTS) {
    for (const catalog of [en, es]) {
      const item = catalog.contracts.items[contract.id];
      assert.ok(item, `missing contract translation: ${contract.id}`);
      assert.ok(item.name, `missing translated name: ${contract.id}`);
      assert.ok(item.description, `missing translated description: ${contract.id}`);
    }
  }
});
