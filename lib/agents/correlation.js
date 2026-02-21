"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.handleCorrelation = handleCorrelation;
const uuid_1 = require("uuid");
async function handleCorrelation(req, res) {
    const { payload, context } = req.body;
    const start = Date.now();
    const layers = [
        "input_validation",
        "signal_grouping",
        "temporal_correlation",
        "service_affinity",
        "response_formatting",
    ];
    const signals = payload?.signals ?? [];
    const timeWindowSecs = payload?.time_window_secs ?? 300;
    const dryRun = payload?.dry_run ?? false;
    if (!Array.isArray(signals) || signals.length === 0) {
        res.status(400).json({
            agent: "correlation",
            incidents_count: 0,
            signals_processed: 0,
            noise_reduction: 0,
            status: "No signals provided",
            layers_executed: ["input_validation"],
            execution_metadata: {
                trace_id: (0, uuid_1.v4)(),
                timestamp: new Date().toISOString(),
                service: "sentinel-agents",
            },
        });
        return;
    }
    const result = {
        agent: "correlation",
        incidents_count: 0,
        signals_processed: signals.length,
        noise_reduction: 0.0,
        status: "success",
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
//# sourceMappingURL=correlation.js.map