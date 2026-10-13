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

const MAX_JSON_BODY = "64kb";

app.use(express.json({ limit: MAX_JSON_BODY, strict: true }));

app.use(
  cors({
    origin: "*",
    methods: ["GET", "POST", "OPTIONS"],
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validateAgentEnvelope(req: Request, res: Response, next: NextFunction): void {
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

app.post("/v1/sentinel/:agent", validateAgentEnvelope, async (req: Request, res: Response, next: NextFunction) => {
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

interface HttpError extends Error {
  status?: number;
  type?: string;
}

app.use((err: HttpError, _req: Request, res: Response, _next: NextFunction) => {
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

export const api = app;
