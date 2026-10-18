const assert = require("node:assert/strict");
const { after, before, test } = require("node:test");
const { api } = require("../lib/index.js");

let server;
let baseUrl;

before(async () => {
  await new Promise((resolve) => {
    server = api.listen(0, "127.0.0.1", () => {
      baseUrl = `http://127.0.0.1:${server.address().port}`;
      resolve();
    });
  });
});

after(async () => {
  if (server) {
    await new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  }
});

async function post(path, body, contentType = "application/json") {
  return fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: { "content-type": contentType },
    body,
  });
}

test("health endpoint remains available", async () => {
  const response = await fetch(`${baseUrl}/health`);
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {
    healthy: true,
    service: "sentinel-agents",
    agents: 5,
  });
});

test("valid agent envelope is accepted", async () => {
  const response = await post(
    "/v1/sentinel/anomaly",
    JSON.stringify({ payload: { telemetry: {}, dry_run: true }, context: {} })
  );
  assert.equal(response.status, 200);
  assert.equal((await response.json()).dry_run, true);
});

test("missing JSON content type is rejected", async () => {
  const response = await post("/v1/sentinel/anomaly", "payload={}", "text/plain");
  assert.equal(response.status, 415);
  assert.deepEqual(await response.json(), { error: "Content-Type must be application/json" });
});

test("missing object payload is rejected", async () => {
  const response = await post("/v1/sentinel/anomaly", JSON.stringify({}));
  assert.equal(response.status, 400);
  assert.deepEqual(await response.json(), { error: "Request body must contain an object payload" });
});

test("malformed JSON returns a bounded JSON error", async () => {
  const response = await post("/v1/sentinel/anomaly", "{");
  assert.equal(response.status, 400);
  assert.match(response.headers.get("content-type"), /^application\/json/);
  assert.deepEqual(await response.json(), { error: "Malformed JSON request body" });
});

test("oversized JSON is rejected", async () => {
  const response = await post(
    "/v1/sentinel/anomaly",
    JSON.stringify({ payload: { padding: "x".repeat(70 * 1024) } })
  );
  assert.equal(response.status, 413);
  assert.deepEqual(await response.json(), { error: "JSON request body exceeds 64kb" });
});

test("CORS preflight advertises only implemented methods", async () => {
  const response = await fetch(`${baseUrl}/v1/sentinel/anomaly`, {
    method: "OPTIONS",
    headers: {
      origin: "https://example.com",
      "access-control-request-method": "DELETE",
    },
  });
  assert.equal(response.status, 204);
  assert.equal(response.headers.get("access-control-allow-methods"), "GET,POST,OPTIONS");
});
