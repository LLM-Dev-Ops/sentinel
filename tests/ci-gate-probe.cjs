const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { spawnSync } = require('node:child_process');
const { resolve, join } = require('node:path');

// Execute the actual aggregate Bash body with explicit GitHub result fixtures.
// This tests the decision boundary, not GitHub's scheduler or Docker startup.
function inspect(root) {
  const workflow = readFileSync(join(root, '.github/workflows/ci.yaml'), 'utf8');
  const block = workflow.split(/^  ci-success:\s*$/m)[1];
  assert(block, 'aggregate job must exist');
  const needs = block.match(/    needs:\n((?:      - [\w-]+\n)+)/);
  assert(needs, 'aggregate needs must remain an explicit list');
  const dependencies = [...needs[1].matchAll(/- ([\w-]+)/g)].map(x => x[1]);
  const body = block.match(/        run: \|\n((?:          .*\n|\n)*)/);
  assert(body, 'aggregate must execute Bash');
  const script = body[1].replace(/^          /gm, '');
  const jobs = workflow.split(/^jobs:\s*$/m)[1];
  assert(jobs, 'workflow must have jobs');
  const allJobs = [...jobs.matchAll(/^  ([\w-]+):\s*$/gm)].map(x => x[1]).filter(x => x !== 'ci-success');
  return { workflow, block, dependencies, allJobs, script };
}

function gate(root, values) {
  const { dependencies, script } = inspect(root);
  const expanded = script
    .replace(/\$\{\{ contains\(needs\.\*\.result, 'failure'\) \}\}/g,
      () => String(dependencies.some(id => values[id] === 'failure')))
    .replace(/\$\{\{ needs\.([\w-]+)\.result \}\}/g, (_, id) => {
      const value = dependencies.includes(id) ? values[id] : '';
      assert(value === undefined || /^[a-z_]*$/.test(value), 'fixtures cannot inject Bash');
      return value || '';
    });
  assert(!expanded.includes('${{'), 'unrecognized expressions must fail closed in the evaluator');
  const r = spawnSync('bash', ['--noprofile', '--norc', '-e', '-o', 'pipefail', '-c', expanded], {
    encoding: 'utf8', timeout: 5000, env: { PATH: process.env.PATH },
  });
  assert.equal(r.signal, null);
  assert.notEqual(r.status, null);
  return { accepts: r.status === 0, exit: r.status, stdout: r.stdout, stderr: r.stderr };
}

function probe(root, contract) {
  const good = Object.fromEntries(contract.requiredJobs.map(id => [id, 'success']));
  const scenario = (name, values, expected) => ({ name, expected, ...gate(root, values) });
  const holdout = contract.requiredJobs.flatMap(id => ['cancelled', 'skipped'].map(result =>
    scenario(`${id}:${result}`, { ...good, [id]: result }, false)));
  holdout.push(scenario('integration-tests:failure', { ...good, 'integration-tests': 'failure' }, false));
  const anchor = [scenario('all-success', good, true), ...contract.requiredJobs.filter(id => id !== 'integration-tests')
    .map(id => scenario(`${id}:failure`, { ...good, [id]: 'failure' }, false))];
  const extra = contract.requiredJobs.flatMap(id => [undefined, 'unknown'].map(result =>
    scenario(`${id}:${result === undefined ? 'missing' : result}`, { ...good, [id]: result }, false)));
  for (const s of [...holdout, ...anchor, ...extra]) s.pass = s.accepts === s.expected;
  return { holdout, anchor, extra, structure: inspect(root).dependencies };
}
module.exports = { inspect, gate, probe };
if (require.main === module) {
  const root = resolve(process.argv[2] || '.');
  const contract = JSON.parse(readFileSync(process.argv[3] || join(__dirname, '../docs/metaharness/2026-10-18/contract.json')));
  const r = probe(root, contract);
  console.log(JSON.stringify(r, null, 2));
  process.exitCode = [...r.holdout, ...r.anchor, ...r.extra].every(x => x.pass) ? 0 : 1;
}
