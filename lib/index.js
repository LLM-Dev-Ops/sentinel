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
app.use(express_1.default.json());
app.use((0, cors_1.default)({
    origin: "*",
    methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
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
app.post("/v1/sentinel/:agent", async (req, res, next) => {
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
// ---------------------------------------------------------------------------
// Export for Cloud Functions
// ---------------------------------------------------------------------------
exports.api = app;
//# sourceMappingURL=index.js.map