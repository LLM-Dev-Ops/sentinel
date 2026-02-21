"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.handleAlert = handleAlert;
const uuid_1 = require("uuid");
async function handleAlert(req, res) {
    const { payload, context } = req.body;
    const start = Date.now();
    const layers = [
        "input_validation",
        "rule_evaluation",
        "suppression_check",
        "decision_generation",
        "response_formatting",
    ];
    const anomaly = payload?.anomaly ?? {};
    const source = payload?.source ?? "api";
    const ignoreSuppression = payload?.ignore_suppression ?? false;
    const dryRun = payload?.dry_run ?? false;
    const decisionId = (0, uuid_1.v4)();
    const result = {
        agent: "alert",
        decision: {
            decision_id: decisionId,
            alert_raised: false,
            confidence: 0.0,
            source,
        },
        alert_raised: false,
        status: "no_alert",
        dry_run: dryRun,
        processing_ms: Date.now() - start,
    };
    res.json({
        ...result,
        layers_executed: layers,
        execution_metadata: {
            trace_id: (0, uuid_1.v4)(),
            timestamp: new Date().toISOString(),
            service: "sentinel-agents",
        },
    });
}
//# sourceMappingURL=alert.js.map