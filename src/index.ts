import express, { Request, Response, NextFunction } from "express";
import cors from "cors";
import { handleAnomaly } from "./agents/anomaly";
import { handleDrift } from "./agents/drift";
import { handleAlert } from "./agents/alert";
import { handleCorrelation } from "./agents/correlation";
import { handleRca } from "./agents/rca";

const app = express();

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

app.use(express.json());

app.use(
  cors({
    origin: "*",
    methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"],
    allowedHeaders: [
      "X-Correlation-ID",
      "X-API-Version",
      "Content-Type",
      "Authorization",
    ],
  })
);

// ---------------------------------------------------------------------------
// Health endpoint
// ---------------------------------------------------------------------------

app.get("/health", (_req: Request, res: Response) => {
  res.json({
    healthy: true,
    service: "sentinel-agents",
    agents: 5,
  });
});

// ---------------------------------------------------------------------------
// Agent routes — POST /v1/sentinel/:agent
// ---------------------------------------------------------------------------

const agentHandlers: Record<string, (req: Request, res: Response) => Promise<void>> = {
  anomaly: handleAnomaly,
  drift: handleDrift,
  alert: handleAlert,
  correlation: handleCorrelation,
  rca: handleRca,
};

app.post("/v1/sentinel/:agent", async (req: Request, res: Response, next: NextFunction) => {
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
  } catch (err) {
    next(err);
  }
});

// ---------------------------------------------------------------------------
// Export for Cloud Functions
// ---------------------------------------------------------------------------

export const api = app;
