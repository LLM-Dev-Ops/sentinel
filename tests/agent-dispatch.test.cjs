const {test} = require('node:test');
const assert = require('node:assert/strict');
const {api} = require('../lib/index.js');

test('agent dispatch accepts only own registered handlers', async () => {
  const server = api.listen(0, '127.0.0.1');
  await new Promise(resolve => server.once('listening', resolve));
  const root = `http://127.0.0.1:${server.address().port}`;
  try {
    for (const name of ['constructor', '__proto__', 'hasOwnProperty', 'isPrototypeOf', 'propertyIsEnumerable']) {
      const response = await fetch(`${root}/v1/sentinel/${name}`, {
        method: 'POST', headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({payload: {}}), signal: AbortSignal.timeout(2000),
      });
      assert.equal(response.status, 404, name);
      const body = await response.json();
      assert.deepEqual(body.available_agents, ['anomaly', 'drift', 'alert', 'correlation', 'rca']);
      assert.equal(body.error, `Unknown agent: ${name}`);
    }
    for (const name of ['anomaly', 'drift', 'alert', 'correlation', 'rca']) {
      const response = await fetch(`${root}/v1/sentinel/${name}`, {
        method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({payload: {}}),
      });
      assert.equal(response.status, 200, name);
    }
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
});
