import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';
const [baseline, candidate, modulePath, output] = process.argv.slice(2);
const fw = await import(pathToFileURL(modulePath));
const parent = '510a128bc9792d420776a60d435ca151dc54f534';
const head = '85490b99a8bd1d0ba142a9f3bdea596c8691af6d';
function measure(root) {
  const started = performance.now();
  const command = ['node', `${candidate}/tests/drift-samples-probe.cjs`, root];
  const r = spawnSync(command[0], command.slice(1), { encoding: 'utf8', timeout: 15000 });
  if (![0, 1].includes(r.status)) throw Error(r.stderr || 'Native HTTP probe incomplete');
  return { command, exit: r.status, elapsedMs: performance.now() - started,
    ...JSON.parse(r.stdout.trim().split('\n').at(-1)) };
}
const trials = [];
for (let i = 0; i < 5; i++) {
  for (const revision of (i % 2 ? [head, parent] : [parent, head])) {
    trials.push({ trial: i, revision, ...measure(revision === head ? candidate : baseline) });
  }
}
const blocked = ['Npm audit remains red', 'Missing Rust lock; full Rust validation incomplete',
  'Required CI/approvals unproven', 'Ruflo agent_execute has no configured provider',
  'Detection algorithms remain stubbed; boundary success is not detector conformance'];
const inverse = { path: 'src/agents/drift.ts', parentBytes: readFileSync(`${baseline}/src/agents/drift.ts`, 'utf8') };
inverse.hash = createHash('sha256').update(inverse.parentBytes).digest('hex');
const evaluations = [];
const result = await fw.runFlywheelGenerations({
  rootPolicy: { revision: parent },
  proposer: async () => ({ value: head, summary: 'Reject malformed and nonfinite drift samples', inverse }),
  evaluator: async (policy, suite) => {
    const value = measure(policy.revision === head ? candidate : baseline);
    evaluations.push({ revision: policy.revision, suite: suite.id, ...value });
    const itemWins = (suite.id === 'holdouts' ? value.holdout : value.anchors).map(x => x.passed);
    return { primary: itemWins.filter(Boolean).length / itemWins.length, itemWins,
      noopRate: 0, costPerWin: 0,
      // Fail-closed full-acceptance flag; not a newly observed component regression.
      regressed: blocked.length > 0 };
  }, promotionRule: fw.meetsPromotionRule,
  holdout: { id: 'holdouts', items: [0,1,2,3,4,5,6,7] },
  anchor: { id: 'anchors', items: [0,1,2,3,4,5,6,7] },
  maxGenerations: 1, signer: fw.makeSigner(), dataSource: 'LIVE_NATIVE_HTTP',
});
const gateFingerprint = fw.gateFingerprint(fw.meetsPromotionRule);
const replay = fw.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint: gateFingerprint });
writeFileSync(output, JSON.stringify({ parent, head, blocked, inverse, trials, evaluations,
  gateFingerprint, replay, promotions: result.promotions, replayBundle: result.replayBundle,
  notes: ['All timings retained, no speedup claimed', 'Noop/cost axes neutral unmeasured zero, not savings',
    'Replay is root-only, zero promotions, not production acceptance'] }, null, 2) + '\n');
console.log(JSON.stringify({ trials: trials.length, replay, promotions: result.promotions.length }));
