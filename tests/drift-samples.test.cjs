const { test, before } = require('node:test');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { resolve } = require('node:path');
let report;
before(() => {
  report = JSON.parse(execFileSync(process.execPath,
    [resolve(__dirname, 'drift-samples-probe.cjs'), resolve(__dirname, '..')],
    { encoding: 'utf8', timeout: 15000 }));
});
for (let i = 0; i < 8; i++) {
  test(`drift rejects malformed numeric sample case ${i}`, () => {
    assert.equal(report.holdout[i].passed, true);
  });
  test(`drift boundary preserves compatibility anchor ${i}`, () => {
    assert.equal(report.anchors[i].passed, true);
  });
}
