const { resolve } = require('node:path');
const { api } = require(resolve(process.argv[2] || '.', 'lib/index.js'));

(async () => {
  const server = api.listen(0, '127.0.0.1');
  await new Promise(resolve => server.once('listening', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const valid = { reference_data: Array(10).fill(1), current_data: Array(10).fill(2) };
  const malformed = [
    { ...valid, reference_data: '1234567890' },
    { ...valid, current_data: '1234567890' },
    { ...valid, reference_data: { length: 10 } },
    { ...valid, current_data: Array(10).fill('2') },
    { ...valid, reference_data: [null, ...Array(9).fill(1)] },
    { ...valid, current_data: Array(10).fill([2]) },
    { ...valid, reference_data: [true, ...Array(9).fill(1)] },
    // This JSON numeric literal parses as Infinity, unlike JSON.stringify(Infinity).
    'overflow',
  ];
  const report = { holdout: [], anchors: [] };
  async function post(agent, payload, raw) {
    const r = await fetch(`${base}/v1/sentinel/${agent}`, { method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: raw || JSON.stringify({ payload }), signal: AbortSignal.timeout(2000) });
    return { status: r.status, body: await r.json() };
  }
  try {
    for (let i = 0; i < malformed.length; i++) {
      const r = await post('drift', malformed[i], malformed[i] === 'overflow'
        ? '{"payload":{"reference_data":[1e400,1,1,1,1,1,1,1,1,1],"current_data":[2,2,2,2,2,2,2,2,2,2]}}' : undefined);
      report.holdout.push({ id: i, status: r.status, passed: r.status === 400
        && r.body.agent === 'drift' && r.body.drift_severity === 'error'
        && JSON.stringify(r.body.layers_executed) === '["input_validation"]'
        && typeof r.body.execution_metadata?.trace_id === 'string' });
    }
    for (const agent of ['anomaly', 'drift', 'alert', 'correlation', 'rca']) {
      const r = await post(agent, { ...valid, signals: [{ id: 'public-fixture' }], dry_run: true });
      report.anchors.push({ id: agent, passed: r.status === 200 && r.body.agent === agent
        && r.body.dry_run === true && typeof r.body.execution_metadata?.trace_id === 'string' });
    }
    const short = await post('drift', { reference_data: [1], current_data: [2] });
    report.anchors.push({ id: 'insufficient', passed: short.status === 400
      && short.body.status === 'Insufficient data: minimum 10 samples required' });
    const unknown = await post('unknown', {});
    report.anchors.push({ id: 'unknown', passed: unknown.status === 404 });
    const health = await fetch(`${base}/health`);
    report.anchors.push({ id: 'health', passed: health.status === 200 && (await health.json()).agents === 5 });
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
  console.log(JSON.stringify(report));
  process.exitCode = [...report.holdout, ...report.anchors].every(x => x.passed) ? 0 : 1;
})().catch(error => { console.error(error); process.exitCode = 2; });
