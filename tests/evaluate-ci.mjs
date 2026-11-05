import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

// node tests/evaluate-ci.mjs BASELINE CANDIDATE PINNED_TOOLS OUTPUT
const [baseline, candidate, tools, output] = process.argv.slice(2).map(x => resolve(x));
const fw = await import(pathToFileURL(join(tools, 'node_modules/@metaharness/flywheel/dist/index.js')));
assert.equal(JSON.parse(readFileSync(join(tools, 'node_modules/@metaharness/flywheel/package.json'))).version, '0.1.12');
const { inspect, probe } = createRequire(import.meta.url)('./ci-gate-probe.cjs');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const source = '.github/workflows/ci.yaml';
const sourceHash = root => hash(readFileSync(join(root, source)));
const rootHash = sourceHash(baseline), candidateHash = sourceHash(candidate);
assert.notEqual(rootHash, candidateHash);
const contractBytes = readFileSync(join(candidate, 'docs/metaharness/2026-10-18/contract.json'));
const contract = JSON.parse(contractBytes);
assert.equal(hash(readFileSync(join(baseline, 'package-lock.json'))), hash(readFileSync(join(candidate, 'package-lock.json'))));
const measurements = [];
let retainedNames;
async function evaluator(policy, suite) {
  const root = policy.aggregate === rootHash ? baseline : candidate;
  assert.equal(sourceHash(root), policy.aggregate);
  const report = probe(root, contract);
  assert.equal(report.holdout.length, 19);
  assert.equal(report.anchor.length, 9);
  let wins, tests;
  if (suite.id === 'holdout') wins = report.holdout.map(x => x.pass);
  else {
    // Execute the same seven original HTTP tests on each revision's actual JS.
    const result = spawnSync(process.execPath, ['--test', '--test-reporter=tap', 'tests/http-boundary.test.cjs'], {
      cwd: root, encoding: 'utf8', timeout: 30000,
      env: { ...process.env },
    });
    assert.equal(result.signal, null);
    assert.equal(result.status, 0);
    assert.match(result.stdout, /# pass 7\b/);
    assert.match(result.stdout, /# fail 0\b/);
    assert.match(result.stdout, /# skipped 0\b/);
    const names = result.stdout.split('\n').filter(x => x.startsWith('# Subtest:')).sort();
    assert.equal(names.length, 7);
    if (root === baseline) retainedNames = names;
    assert.deepEqual(names, retainedNames);
    tests = { command: [process.execPath, '--test', '--test-reporter=tap', 'tests/http-boundary.test.cjs'], exit: result.status, stdout: result.stdout, names };
    wins = [...report.anchor.map(x => x.pass), true];
  }
  const primary = wins.filter(Boolean).length / wins.length;
  const score = { primary, noopRate: 1 - primary, costPerWin: 0, regressed: suite.id === 'anchor' && wins.some(x => !x), itemWins: wins };
  measurements.push({ source: policy.aggregate, suite: suite.id, report, tests, score });
  assert.equal(sourceHash(root), policy.aggregate);
  return score;
}
const fingerprint = fw.gateFingerprint(fw.meetsPromotionRule);
const result = await fw.runFlywheelGenerations({
  rootPolicy: { aggregate: rootHash },
  proposer: async () => ({ value: candidateHash, summary: 'Require every CI job, reject non-success results, use healthy project-isolated integration fixtures', inverse: { path: source, parentBytes: readFileSync(join(baseline, source), 'utf8'), parentSha256: rootHash } }),
  evaluator,
  holdout: { id: 'holdout', items: Array.from({ length: 19 }, (_, i) => `non-success-${i}`) },
  anchor: { id: 'anchor', items: Array.from({ length: 10 }, (_, i) => `retained-${i}`) },
  maxGenerations: 1, signer: fw.makeSigner(), promotionRule: fw.meetsPromotionRule,
  dataSource: 'LIVE_AGGREGATE_BASH_PROCESSES_WITH_EXPLICIT_RESULT_FIXTURES_AND_REAL_HTTP_TESTS',
});
const replay = fw.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint: fingerprint });
assert.equal(replay.pass, true);
assert.equal(result.promotions.length, 1);
const structure = inspect(candidate);
assert.deepEqual([...structure.dependencies].sort(), [...structure.allJobs].sort());
assert(measurements.filter(x => x.source === candidateHash).every(x => x.report.extra.every(x => x.pass)));
const evidence = {
  contract, contractSha256: hash(contractBytes), fixtureSha256: hash(readFileSync(new URL('./ci-gate-probe.cjs', import.meta.url))),
  baselineSource: rootHash, candidateSource: candidateHash, measurements,
  promotions: result.promotions.length, fingerprint, replay, replayBundle: result.replayBundle,
  boundary: 'Component aggregate decision boundary only, not real GitHub event scheduling or Docker/Rust integration execution. Expressions are expanded only for the documented fixture result fields, then the actual Bash body executes. Hosted startup, cleanup, full Rust, security and CI gates remain independent. No production rollback executed; inverse parent workflow bytes retained. Cost/latency/energy not measured; zero costPerWin is a neutral gate input, not a measurement.',
};
writeFileSync(output, JSON.stringify(evidence, null, 2) + '\n');
console.log(JSON.stringify({ promotions: evidence.promotions, replay, measurements: measurements.map(m => ({ suite: m.suite, primary: m.score.primary, extra: m.report.extra.filter(x => x.pass).length, tests: m.tests?.exit })) }));
