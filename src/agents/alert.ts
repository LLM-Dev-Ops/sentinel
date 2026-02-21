import { Request, Response } from "express";
import { v4 as uuidv4 } from "uuid";

export async function handleAlert(req: Request, res: Response): Promise<void> {
  const { payload, context } = req.body;
  const start = Date.now();

  const layers: string[] = [
    "input_validation",
    "rule_evaluation",
    "suppression_check",
    "decision_generation",
    "response_formatting",
  ];

  const anomaly = payload?.anomaly ?? {};
  const source: string = payload?.source ?? "api";
  const ignoreSuppression: boolean = payload?.ignore_suppression ?? false;
  const dryRun: boolean = payload?.dry_run ?? false;

  const decisionId = uuidv4();

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
      trace_id: uuidv4(),
      timestamp: new Date().toISOString(),
      service: "sentinel-agents",
    },
  });
}
