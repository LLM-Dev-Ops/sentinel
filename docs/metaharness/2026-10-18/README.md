# Sentinel CI conjunction and dependency recovery

Status: EXECUTED and component VALIDATED; repository merge BLOCKED.

The existing PR is resumed, not duplicated:
https://github.com/LLM-Dev-Ops/sentinel/pull/22

The baseline is `16249d00b5ebaca889ac6ea372c8acc1b867579f`.
Main remains `504b0ea466620ffacfd7ea19993d5a62b71babdd`.
Implementation commits are `f89b1e543b3aeea364cdd011947eef82ef4f335e`
and `40cf495d899bbc2cf25a213c85bcd6a46693ac24`.
Final exact-head CI and runtime results are recorded in the PR description.

## Change and frozen evidence

The aggregate previously omitted integration and checked only for failure.
It now waits for every other job and accepts only success. The actual aggregate
Bash body is executed with explicit result fixtures, not a fake GitHub workflow:

| Measurement | Baseline | Candidate |
| --- | ---: | ---: |
| Cancelled/skipped required job or failed integration rejects | 0/19 | 19/19 |
| All-success accepts; original failures reject | 9/9 | 9/9 |
| Missing/unknown results reject | 0/18 | 18/18 |
| Existing HTTP tests | 7/7 | 7/7 |
| Full Node suite | 7/7 | 54/54 |

`contract.json` was frozen before implementation; SHA-256
`59c26d090c7abec219fa5871bb09a4dee06563d506f8c75cf29df110393ee60d`.
Flywheel 0.1.12 executed the real Bash and HTTP evaluators, promoted the component
once, and passed all eight signed replay checks. This is not repository approval.
The compressed complete receipt retains parent workflow bytes for replay/rollback.
No cost, latency or energy gain is claimed.

Darwin 0.10.3 used the real npm-test sandbox, one generation, two children,
concurrency one and seed 2026101814. All three scores tied at 0.985; neither child
was promoted. Nine test processes exited zero. The earlier parser-defective run
was discarded, including its apparent lift. MetaHarness 0.4.17 static fit stayed
76; its compile-confidence estimate is not evidence of Rust compilation.

## Real hosted integration setup

Compose v2 replaces the broken standalone Kafka service and unavailable v1
command. The new file contains only ephemeral CI fixtures with loopback ports,
no host data volumes, fixed container names or shared networks. It adds the
required Kafka listeners, a valid ZooKeeper health probe, explicit fixture-only
RabbitMQ authentication, bounded health waiting, diagnostics and always cleanup.
No production datastore or deployment is added.

The first candidate failed the new ZooKeeper probe because `cub zk-ready` expects
one `host:port` connect string, not separate host and port arguments. The defect
was corrected and independently rerun. On corrected hosted run 39152719633,
all five containers became healthy; startup, readiness (including actual broker
API/user checks) and project-scoped cleanup succeeded. The Rust integration
command then failed authenticating its private `infra-config` Git dependency.
No authentication workaround was attempted. Docker is absent on RuOS; hosted
container execution, not local static parsing, supplies the startup evidence.

## Remaining gates

- Hosted Rust checks cannot fetch private `LLM-Dev-Ops/infra` with the configured
  job credentials. Formatting also fails. No required check is bypassed.
- No Cargo.lock is committed. Initial locked test/build/Clippy commands exit 101.
  Diagnostic resolution generated an uncommitted lock; after installing actual
  native prerequisites on RuOS, baseline tests reached compilation and exposed
  38 alerting-test and 30 API-test errors. This is not passing Rust behavior.
  Corrected-candidate tests expose the same errors; its diagnostic locked
  workspace build succeeds, with baseline-identical uncommitted lock bytes.
- Fresh npm audits exit 1 with 98 findings: 3 critical, 39 high, 53 moderate,
  3 low. The existing Node lockfile is unchanged; no audit suppression is added.
- Cargo-audit 0.22.2 reports seven vulnerabilities and six unmaintained warnings
  in the 574-package diagnostic graph; its advisory revision is recorded in the
  ledger. The generated lock is uncommitted, not a validated dependency repair.
- Rust build, Clippy, security, coverage and integration remain conjunctive merge
  gates. Full application integration was not validated by healthy fixtures.

## Reproduce

Install the exact tool packages recorded in `ledger.json`; set `TOOLS` to their
installation prefix and create separate baseline/candidate worktrees. Both must
have the pinned npm graph installed. From the candidate:

```sh
npm ci
npm test
node tests/ci-gate-probe.cjs .
node tests/evaluate-ci.mjs BASELINE CANDIDATE TOOLS /tmp/sentinel-flywheel.json
node "$TOOLS/node_modules/metaharness/dist/bin.js" score . --json
node "$TOOLS/node_modules/@metaharness/darwin/dist/cli.js" evolve . --generations 1 --children 2 --concurrency 1 --seed 2026101814 --sandbox real --selection pareto
docker compose -f docker-compose.ci.yaml -p sentinel-local-validation up --detach --wait --wait-timeout 180
docker compose -f docker-compose.ci.yaml -p sentinel-local-validation down --volumes --remove-orphans
```

`ledger.json` records the actual offline Ruflo launcher, real MiniLM revision,
versions, task ID and four persisted-learning confirmations. Network-capable
default post-task loading is not used; there is no hash-embedding fallback.
`coverage.json` names all 58 repositories and the exact metadata inspection
depth, latest main/CI SHAs, root manifests/instructions and 118 open PRs.

## Rollback

Revert the correction commit followed by the first implementation commit, or
revert a future squash merge. Parent workflow bytes are in the signed receipt;
reverse-patch applicability was verified. No production rollback was executed.
Cleanup removed only this hosted run's ephemeral fixture containers, anonymous
volumes and project network; these contained disposable test data.

Upstream references used for the implementation:

- https://docs.docker.com/reference/cli/docker/compose/up/
- https://docs.confluent.io/platform/7.5/installation/docker/config-reference.html
- https://github.com/confluentinc/confluent-docker-utils/blob/master/confluent/docker_utils/cub.py
