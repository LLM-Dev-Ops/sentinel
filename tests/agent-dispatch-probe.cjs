const {resolve} = require('node:path');
const {api} = require(resolve(process.argv[2] || '.', 'lib/index.js'));
(async () => {
  const server = api.listen(0, '127.0.0.1');
  await new Promise(resolve => server.once('listening', resolve));
  const root = `http://127.0.0.1:${server.address().port}`;
  const names = ['anomaly', 'drift', 'alert', 'correlation', 'rca'];
  const report = {holdout: [], anchors: []};
  try {
    for (const [group, cases, expected] of [['holdout', ['constructor', '__proto__', 'hasOwnProperty', 'isPrototypeOf', 'propertyIsEnumerable'], 404], ['anchors', names, 200]]) {
      for (const name of cases) {
        try {
          const response = await fetch(`${root}/v1/sentinel/${name}`, {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({payload: {}}), signal: AbortSignal.timeout(2000)});
          const body = await response.json();
          const envelope = group === 'anchors' || (body.error === `Unknown agent: ${name}` && JSON.stringify(body.available_agents) === JSON.stringify(names));
          report[group].push({name, status: response.status, passed: response.status === expected && envelope});
        } catch (error) { report[group].push({name, passed: false, error: error.name}); }
      }
    }
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
  console.log(JSON.stringify(report));
  process.exitCode = report.holdout.every(x => x.passed) && report.anchors.every(x => x.passed) ? 0 : 1;
})().catch(error => {console.error(error); process.exitCode = 1;});
