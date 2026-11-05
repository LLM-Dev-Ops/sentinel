const assert = require('node:assert/strict');
const { test } = require('node:test');
const { join } = require('node:path');
const { inspect, probe } = require('./ci-gate-probe.cjs');
const contract = require('../docs/metaharness/2026-10-18/contract.json');
const root = join(__dirname, '..');
const report = probe(root, contract);
for (const s of [...report.holdout, ...report.anchor, ...report.extra]) {
  test(`aggregate ${s.name}`, () => assert.equal(s.accepts, s.expected, s.stdout + s.stderr));
}
test('aggregate waits for every other job, including integration, even after failure', () => {
  const r = inspect(root);
  assert.deepEqual([...r.dependencies].sort(), [...r.allJobs].sort());
  assert.deepEqual([...r.dependencies].sort(), [...contract.requiredJobs].sort());
  assert.match(r.block, /^    if: always\(\)$/m);
});
