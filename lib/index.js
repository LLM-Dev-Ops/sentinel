"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.api = void 0;
const express_1 = __importDefault(require("express"));
const cors_1 = __importDefault(require("cors"));
const anomaly_1 = require("./agents/anomaly");
const drift_1 = require("./agents/drift");
const alert_1 = require("./agents/alert");
const correlation_1 = require("./agents/correlation");
const rca_1 = require("./agents/rca");
const app = (0, express_1.default)();
// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------
const MAX_JSON_BODY = "64kb";
app.use(express_1.default.json({ limit: MAX_JSON_BODY, strict: true }));
app.use((0, cors_1.default)({
    origin: "*",
    methods: ["GET", "POST", "OPTIONS"],
    allowedHeaders: [
        "X-Correlation-ID",
        "X-API-Version",
        "Content-Type",
        "Authorization",
    ],
}));
// ---------------------------------------------------------------------------
// Health endpoint
// ---------------------------------------------------------------------------
app.get("/health", (_req, res) => {
    res.json({
        healthy: true,
        service: "sentinel-agents",
        agents: 5,
    });
});
// ---------------------------------------------------------------------------
// Agent routes — POST /v1/sentinel/:agent
// ---------------------------------------------------------------------------
const agentHandlers = {
    anomaly: anomaly_1.handleAnomaly,
    drift: drift_1.handleDrift,
    alert: alert_1.handleAlert,
    correlation: correlation_1.handleCorrelation,
    rca: rca_1.handleRca,
};
function isRecord(value) {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}
function validateAgentEnvelope(req, res, next) {
    if (!req.is("application/json")) {
        res.status(415).json({ error: "Content-Type must be application/json" });
        return;
    }
    if (!isRecord(req.body) || !isRecord(req.body.payload)) {
        res.status(400).json({ error: "Request body must contain an object payload" });
        return;
    }
    if (req.body.context !== undefined && !isRecord(req.body.context)) {
        res.status(400).json({ error: "context must be an object when provided" });
        return;
    }
    next();
}
app.post("/v1/sentinel/:agent", validateAgentEnvelope, async (req, res, next) => {
    const { agent } = req.params;
    const handler = agentHandlers[agent];
    if (!handler) {
        res.status(404).json({
            error: `Unknown agent: ${agent}`,
            available_agents: Object.keys(agentHandlers),
        });
        return;
    }
    try {
        await handler(req, res);
    }
    catch (err) {
        next(err);
    }
});
app.use((err, _req, res, _next) => {
    if (err.type === "entity.too.large") {
        res.status(413).json({ error: `JSON request body exceeds ${MAX_JSON_BODY}` });
        return;
    }
    if (err.type === "entity.parse.failed" || (err instanceof SyntaxError && err.status === 400)) {
        res.status(400).json({ error: "Malformed JSON request body" });
        return;
    }
    res.status(500).json({ error: "Internal server error" });
});
// ---------------------------------------------------------------------------
// Export for Cloud Functions
// ---------------------------------------------------------------------------
exports.api = app;
//# sourceMappingURL=index.js.map