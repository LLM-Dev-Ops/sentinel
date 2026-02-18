//! LLM-Sentinel Main Binary
//!
//! Orchestrates all components of the sentinel system:
//! - Ingestion: Kafka consumer for telemetry
//! - Detection: Multi-detector anomaly detection engine
//! - Storage: InfluxDB time-series storage
//! - Alerting: RabbitMQ alert publisher
//! - API: REST API server
//! - Benchmarks: Canonical benchmark interface

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use llm_sentinel_alerting::{prelude::*, rabbitmq::RetryConfig};
use llm_sentinel_api::prelude::*;
use llm_sentinel_benchmarks::{
    run_all_benchmarks, run_all_benchmarks_parallel, BenchmarkIO, generate_summary,
};
use llm_sentinel_core::config::Config;
use llm_sentinel_core::events::TelemetryEvent;
use llm_sentinel_detection::{
    agents::{
        anomaly_detection::{
            AnomalyDetectionAgent, AnomalyDetectionAgentConfig, DiagnoseCheck,
        },
    },
    prelude::*,
};
use llm_sentinel_ingestion::prelude::*;
use llm_sentinel_storage::prelude::*;
use std::{path::PathBuf, sync::Arc};
use tokio::{signal, sync::Mutex};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// LLM-Sentinel CLI arguments
#[derive(Debug, Parser)]
#[clap(name = "sentinel", version, about = "LLM observability and anomaly detection")]
struct Cli {
    /// Configuration file path
    #[clap(short, long, default_value = "config/sentinel.yaml", global = true)]
    config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[clap(long, env = "SENTINEL_LOG_LEVEL", default_value = "info", global = true)]
    log_level: String,

    /// Enable JSON logging
    #[clap(long, env = "SENTINEL_LOG_JSON", global = true)]
    log_json: bool,

    /// Dry run mode (don't start services)
    #[clap(long, global = true)]
    dry_run: bool,

    /// API-only mode (skip storage and alerting initialization)
    #[clap(long, env = "SENTINEL_API_ONLY", global = true)]
    api_only: bool,

    /// Subcommand to execute
    #[clap(subcommand)]
    command: Option<Commands>,
}

/// Available subcommands
#[derive(Debug, Subcommand)]
enum Commands {
    /// Run benchmarks and write results to canonical output directories
    Run {
        /// Run benchmarks in parallel
        #[clap(long)]
        parallel: bool,

        /// Output directory for benchmark results
        #[clap(long, default_value = "benchmarks/output")]
        output: PathBuf,

        /// Only print results, don't write to files
        #[clap(long)]
        dry_run: bool,

        /// Output results as JSON to stdout
        #[clap(long)]
        json: bool,
    },
    /// Start the sentinel service (default if no subcommand given)
    Serve,
    /// Agent management commands (inspect, replay, diagnose)
    Agent {
        #[clap(subcommand)]
        command: AgentCommands,
    },
}

/// Agent subcommands following agentics-cli specification
#[derive(Debug, Subcommand)]
enum AgentCommands {
    /// Inspect agent state, baselines, and configuration
    Inspect {
        /// Agent name (e.g., "anomaly-detection")
        #[clap(default_value = "anomaly-detection")]
        agent: String,

        /// Filter by service name
        #[clap(long)]
        service: Option<String>,

        /// Filter by model name
        #[clap(long)]
        model: Option<String>,

        /// Output as JSON
        #[clap(long)]
        json: bool,
    },
    /// Replay a telemetry event through an agent
    Replay {
        /// Agent name (e.g., "anomaly-detection")
        #[clap(default_value = "anomaly-detection")]
        agent: String,

        /// Event file path (JSON)
        #[clap(long)]
        event_file: PathBuf,

        /// Don't persist DecisionEvent
        #[clap(long)]
        dry_run: bool,

        /// Show detailed processing steps
        #[clap(long)]
        verbose: bool,
    },
    /// Run diagnostics on agent health and configuration
    Diagnose {
        /// Agent name (e.g., "anomaly-detection")
        #[clap(default_value = "anomaly-detection")]
        agent: String,

        /// Specific check to run (baselines, detectors, ruvector, all)
        #[clap(long, default_value = "all")]
        check: String,

        /// Output as JSON
        #[clap(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli)?;

    info!("Starting LLM-Sentinel v{}", env!("CARGO_PKG_VERSION"));

    // Handle subcommands
    match cli.command {
        Some(Commands::Run {
            parallel,
            output,
            dry_run,
            json,
        }) => {
            run_benchmarks_command(parallel, output, dry_run, json).await
        }
        Some(Commands::Agent { command }) => {
            run_agent_command(command).await
        }
        Some(Commands::Serve) | None => {
            run_serve_command(&cli).await
        }
    }
}

/// Run the benchmarks subcommand
async fn run_benchmarks_command(
    parallel: bool,
    output: PathBuf,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    info!("Running benchmarks...");

    // Execute benchmarks
    let results = if parallel {
        info!("Running benchmarks in parallel mode");
        run_all_benchmarks_parallel().await
    } else {
        info!("Running benchmarks sequentially");
        run_all_benchmarks().await
    };

    info!("Completed {} benchmarks", results.len());

    // Output results
    if json_output {
        let json = serde_json::to_string_pretty(&results)?;
        println!("{}", json);
    } else {
        // Print summary table
        println!("\n{}", "=".repeat(80));
        println!("LLM-Sentinel Benchmark Results");
        println!("{}", "=".repeat(80));
        println!(
            "{:<25} {:<20} {:<30}",
            "Target", "Duration (ms)", "Key Metrics"
        );
        println!("{}", "-".repeat(80));

        for result in &results {
            let duration = result
                .metrics
                .get("total_duration_ms")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string());

            let ops_per_sec = result
                .metrics
                .get("ops_per_second")
                .or_else(|| result.metrics.get("events_per_second"))
                .and_then(|v| v.as_f64())
                .map(|v| format!("{:.0} ops/s", v))
                .unwrap_or_else(|| "N/A".to_string());

            println!("{:<25} {:<20} {:<30}", result.target_id, duration, ops_per_sec);
        }

        println!("{}", "=".repeat(80));
        println!(
            "Total: {} benchmarks executed at {}",
            results.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!();
    }

    // Write results to files unless dry_run
    if !dry_run {
        let io = BenchmarkIO::new(&output);
        io.ensure_directories()?;

        // Write individual results
        io.write_results(&results)?;

        // Write combined results
        io.write_combined_results(&results)?;

        // Generate and write summary
        let summary = generate_summary(&results);
        let summary_path = io.write_summary(&summary)?;

        info!("Results written to: {:?}", output);
        info!("Summary written to: {:?}", summary_path);

        if !json_output {
            println!("Results written to: {}", output.display());
            println!("Summary: {}/summary.md", output.display());
        }
    }

    Ok(())
}

/// Run agent commands (inspect, replay, diagnose)
async fn run_agent_command(command: AgentCommands) -> Result<()> {
    match command {
        AgentCommands::Inspect {
            agent,
            service,
            model,
            json,
        } => {
            run_agent_inspect(&agent, service.as_deref(), model.as_deref(), json).await
        }
        AgentCommands::Replay {
            agent,
            event_file,
            dry_run,
            verbose,
        } => {
            run_agent_replay(&agent, &event_file, dry_run, verbose).await
        }
        AgentCommands::Diagnose {
            agent,
            check,
            json,
        } => {
            run_agent_diagnose(&agent, &check, json).await
        }
    }
}

/// Run agent inspect command
async fn run_agent_inspect(
    agent_name: &str,
    service: Option<&str>,
    model: Option<&str>,
    json_output: bool,
) -> Result<()> {
    match agent_name {
        "anomaly-detection" => {
            let config = AnomalyDetectionAgentConfig::default();
            let agent = AnomalyDetectionAgent::new(config)
                .context("Failed to create Anomaly Detection Agent")?;

            let result = agent.inspect(service, model).await;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("\n{}", "=".repeat(70));
                println!("ANOMALY DETECTION AGENT INSPECTION");
                println!("{}", "=".repeat(70));
                println!("Agent ID:      {}", result.agent_id);
                println!("Version:       {}", result.agent_version);
                println!();
                println!("CONFIGURATION:");
                println!("  Z-score:  {} (threshold: {})",
                    if result.config.enable_zscore { "enabled" } else { "disabled" },
                    result.config.zscore_threshold);
                println!("  IQR:      {} (multiplier: {})",
                    if result.config.enable_iqr { "enabled" } else { "disabled" },
                    result.config.iqr_multiplier);
                println!("  MAD:      {} (threshold: {})",
                    if result.config.enable_mad { "enabled" } else { "disabled" },
                    result.config.mad_threshold);
                println!("  CUSUM:    {} (threshold: {})",
                    if result.config.enable_cusum { "enabled" } else { "disabled" },
                    result.config.cusum_threshold);
                println!();
                println!("STATISTICS:");
                println!("  Invocations:       {}", result.stats.invocations);
                println!("  Anomalies:         {}", result.stats.anomalies_detected);
                println!("  Decisions saved:   {}", result.stats.decisions_persisted);
                println!("  Avg processing:    {:.2}ms", result.stats.avg_processing_ms);
                println!();
                println!("BASELINES:");
                println!("  Total:   {}", result.baseline_stats.total_baselines);
                println!("  Valid:   {}", result.baseline_stats.valid_baselines);
                println!("  Window:  {}", result.baseline_stats.window_size);
                println!();

                if !result.baselines.is_empty() {
                    println!("{:<20} {:<15} {:<15} {:<10} {:<8} {:<6}",
                        "Service", "Model", "Metric", "Mean", "StdDev", "Valid");
                    println!("{}", "-".repeat(70));
                    for baseline in &result.baselines {
                        println!("{:<20} {:<15} {:<15} {:<10.2} {:<8.2} {:<6}",
                            baseline.service,
                            baseline.model,
                            baseline.metric,
                            baseline.mean,
                            baseline.std_dev,
                            if baseline.is_valid { "yes" } else { "no" });
                    }
                } else {
                    println!("  (no baselines established yet)");
                }
                println!("{}", "=".repeat(70));
            }
        }
        other => {
            return Err(anyhow::anyhow!("Unknown agent: {}. Available: anomaly-detection", other));
        }
    }

    Ok(())
}

/// Run agent replay command
async fn run_agent_replay(
    agent_name: &str,
    event_file: &PathBuf,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    match agent_name {
        "anomaly-detection" => {
            // Read event from file
            let content = std::fs::read_to_string(event_file)
                .context("Failed to read event file")?;
            let event: TelemetryEvent = serde_json::from_str(&content)
                .context("Failed to parse event JSON")?;

            info!(
                event_id = %event.event_id,
                service = %event.service_name,
                model = %event.model,
                "Replaying event through Anomaly Detection Agent"
            );

            let mut config = AnomalyDetectionAgentConfig::default();
            config.dry_run = dry_run;
            let agent = AnomalyDetectionAgent::new(config)
                .context("Failed to create Anomaly Detection Agent")?;

            let result = agent.replay(&event, dry_run).await
                .context("Replay failed")?;

            if verbose {
                println!("\n{}", "=".repeat(70));
                println!("REPLAY RESULT");
                println!("{}", "=".repeat(70));
                println!("Decision ID:     {}", result.decision_event.decision_id);
                println!("Anomaly:         {}", result.decision_event.outputs.anomaly_detected);
                println!("Confidence:      {:.2}%", result.decision_event.confidence * 100.0);
                println!("Detectors:       {:?}", result.decision_event.outputs.detectors_evaluated);
                println!("Triggered by:    {:?}", result.decision_event.outputs.triggered_by);
                println!("Processing:      {}ms", result.decision_event.outputs.metadata.processing_ms);
                println!("Persisted:       {}", result.persisted);
                println!();
                println!("CONSTRAINTS APPLIED:");
                for constraint in &result.decision_event.constraints_applied {
                    println!("  - {} ({}): {} - {}",
                        constraint.constraint_id,
                        format!("{:?}", constraint.constraint_type).to_lowercase(),
                        constraint.value,
                        if constraint.passed { "PASSED" } else { "FAILED" });
                }
                println!("{}", "=".repeat(70));
            } else {
                println!("{}", serde_json::to_string_pretty(&result.decision_event)?);
            }

            if let Some(anomaly) = &result.anomaly_event {
                if verbose {
                    println!("\nANOMALY DETECTED:");
                    println!("  Type:       {}", anomaly.anomaly_type);
                    println!("  Severity:   {}", anomaly.severity);
                    println!("  Metric:     {}", anomaly.details.metric);
                    println!("  Value:      {:.2}", anomaly.details.value);
                    println!("  Baseline:   {:.2}", anomaly.details.baseline);
                    println!("  Threshold:  {:.2}", anomaly.details.threshold);
                }
            }
        }
        other => {
            return Err(anyhow::anyhow!("Unknown agent: {}. Available: anomaly-detection", other));
        }
    }

    Ok(())
}

/// Run agent diagnose command
async fn run_agent_diagnose(agent_name: &str, check: &str, json_output: bool) -> Result<()> {
    match agent_name {
        "anomaly-detection" => {
            let config = AnomalyDetectionAgentConfig::default();
            let agent = AnomalyDetectionAgent::new(config)
                .context("Failed to create Anomaly Detection Agent")?;

            let check_type: DiagnoseCheck = check.parse()
                .map_err(|e: String| anyhow::anyhow!(e))?;

            let result = agent.diagnose(check_type).await;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("\n{}", "=".repeat(70));
                println!("ANOMALY DETECTION AGENT DIAGNOSTICS");
                println!("{}", "=".repeat(70));
                println!("Agent ID:       {}", result.agent_id);
                println!("Timestamp:      {}", result.timestamp);
                println!("Overall Status: {:?}", result.overall_status);
                println!();
                println!("HEALTH CHECKS:");
                for check in &result.checks {
                    let status_icon = match check.status {
                        llm_sentinel_detection::agents::anomaly_detection::CheckStatus::Healthy => "[OK]",
                        llm_sentinel_detection::agents::anomaly_detection::CheckStatus::Warning => "[!!]",
                        llm_sentinel_detection::agents::anomaly_detection::CheckStatus::Unhealthy => "[XX]",
                    };
                    println!("  {} {}: {}", status_icon, check.name, check.message);
                }
                println!("{}", "=".repeat(70));
            }
        }
        other => {
            return Err(anyhow::anyhow!("Unknown agent: {}. Available: anomaly-detection", other));
        }
    }

    Ok(())
}

/// Run the serve subcommand (default behavior)
async fn run_serve_command(cli: &Cli) -> Result<()> {
    info!("Loading configuration from: {:?}", cli.config);

    // Load configuration
    let config = Config::from_file(&cli.config).context("Failed to load configuration")?;

    info!("Configuration loaded successfully");

    if cli.dry_run {
        info!("Dry run mode - configuration validated, exiting");
        return Ok(());
    }

    // Check for API-only mode (for Cloud Run deployments without full infrastructure)
    if cli.api_only {
        info!("API-only mode enabled - starting minimal API server without storage/alerting");
        return run_api_only_server(&config).await;
    }

    // Initialize full sentinel with all components
    let sentinel = Sentinel::new(config).await?;

    // Run the sentinel
    sentinel.run().await?;

    Ok(())
}

/// Run API-only server without storage or alerting infrastructure
async fn run_api_only_server(config: &Config) -> Result<()> {
    use std::net::SocketAddr;
    use llm_sentinel_storage::NoopStorage;

    // Cloud Run injects PORT env var — always prefer it over config file
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(config.server.port);

    let addr: SocketAddr = format!("{}:{}", config.server.host, port)
        .parse()
        .context("Invalid server address")?;

    info!("Starting API-only server on {}", addr);

    // Build a minimal router with health endpoints and agent endpoints
    let api_config = llm_sentinel_api::ApiConfig {
        bind_addr: addr,
        enable_cors: true,
        cors_origins: vec!["*".to_string()],
        timeout_secs: config.server.request_timeout_secs,
        max_body_size: 10 * 1024 * 1024, // 10MB
        enable_logging: true,
        metrics_path: "/metrics".to_string(),
    };

    // Create states for health, metrics, and query endpoints
    let noop_storage: Arc<dyn llm_sentinel_storage::Storage> = Arc::new(NoopStorage::new());

    let health_state = Arc::new(llm_sentinel_api::handlers::health::HealthState::new(
        env!("CARGO_PKG_VERSION").to_string(),
        Arc::new(|| Ok(())), // Always healthy in API-only mode
    ));
    let metrics_state = Arc::new(llm_sentinel_api::handlers::metrics::MetricsState::new());
    let query_state = Arc::new(llm_sentinel_api::handlers::query::QueryState::new(noop_storage));

    let app = llm_sentinel_api::routes::create_router(
        api_config,
        health_state,
        metrics_state,
        query_state,
    );

    info!("API server listening on {}", addr);

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let server = axum::serve(listener, app);

    // Use tokio select for graceful shutdown
    tokio::select! {
        result = server => {
            result.context("API server failed")?;
        }
        _ = shutdown_signal() => {
            info!("Shutdown signal received");
        }
    }

    info!("API server shut down gracefully");
    Ok(())
}

/// Wait for shutdown signal
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("Received Ctrl+C, shutting down..."); },
        _ = terminate => { info!("Received SIGTERM, shutting down..."); },
    }
}

/// Initialize logging based on CLI arguments
fn init_logging(cli: &Cli) -> Result<()> {
    let log_level = cli
        .log_level
        .parse::<tracing::Level>()
        .context("Invalid log level")?;

    if cli.log_json {
        // JSON structured logging
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(true)
                    .with_current_span(true)
                    .with_span_list(true),
            )
            .with(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(log_level.into()),
            )
            .init();
    } else {
        // Human-readable logging
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_line_number(true),
            )
            .with(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(log_level.into()),
            )
            .init();
    }

    info!("Logging initialized at level: {}", log_level);

    Ok(())
}

/// Main Sentinel orchestrator
struct Sentinel {
    config: Config,
    storage: Arc<InfluxDbStorage>,
    detection_engine: Arc<Mutex<DetectionEngine>>,
    alerter: Arc<RabbitMqAlerter>,
    deduplicator: Arc<AlertDeduplicator>,
}

impl Sentinel {
    /// Create a new Sentinel instance
    async fn new(config: Config) -> Result<Self> {
        info!("Initializing Sentinel components...");

        // Initialize storage
        info!("Connecting to InfluxDB...");
        let core_influxdb_config = config.storage.influxdb.clone()
            .context("InfluxDB configuration is required")?;

        // Convert core InfluxDbConfig to storage InfluxDbConfig
        let influxdb_config = llm_sentinel_storage::influxdb::InfluxDbConfig {
            url: core_influxdb_config.url,
            org: core_influxdb_config.org,
            telemetry_bucket: core_influxdb_config.bucket.clone(),
            anomaly_bucket: format!("{}-anomalies", core_influxdb_config.bucket),
            token: core_influxdb_config.token,
            batch_size: 100,
            timeout_secs: core_influxdb_config.timeout_secs,
        };

        let storage = InfluxDbStorage::new(influxdb_config)
            .await
            .context("Failed to initialize storage")?;
        let storage = Arc::new(storage);
        info!("InfluxDB connected");

        // Initialize detection engine
        info!("Initializing detection engine...");

        // Convert DetectionConfig to EngineConfig
        // For now, use default EngineConfig - in production this should be configured
        let engine_config = EngineConfig::default();

        let detection_engine = Arc::new(Mutex::new(
            DetectionEngine::new(engine_config)
                .context("Failed to create detection engine")?,
        ));
        info!("Detection engine initialized");

        // Initialize alerting
        info!("Connecting to RabbitMQ...");
        let core_rabbitmq_config = config.alerting.rabbitmq.clone()
            .context("RabbitMQ configuration is required")?;

        // Convert core RabbitMqConfig to alerting RabbitMqConfig
        let rabbitmq_config = llm_sentinel_alerting::rabbitmq::RabbitMqConfig {
            url: core_rabbitmq_config.url,
            exchange: core_rabbitmq_config.exchange,
            exchange_type: core_rabbitmq_config.exchange_type,
            routing_key_prefix: "alert".to_string(),
            persistent: core_rabbitmq_config.durable,
            timeout_secs: 10,
            retry_config: RetryConfig {
                max_attempts: core_rabbitmq_config.retry_attempts,
                initial_delay_ms: core_rabbitmq_config.retry_delay_ms,
                backoff_multiplier: 2.0,
                max_delay_ms: 30000,
            },
        };

        let alerter = RabbitMqAlerter::new(rabbitmq_config)
            .await
            .context("Failed to initialize RabbitMQ alerter")?;
        let alerter = Arc::new(alerter);
        info!("RabbitMQ connected");

        // Initialize deduplicator
        let dedup_config = DeduplicationConfig {
            window_secs: config.alerting.dedup_window_secs,
            enabled: true,
            cleanup_interval_secs: 60,
        };
        let deduplicator = Arc::new(AlertDeduplicator::new(dedup_config));

        // Start cleanup task
        deduplicator.clone().start_cleanup_task();

        info!("All components initialized successfully");

        Ok(Self {
            config,
            storage,
            detection_engine,
            alerter,
            deduplicator,
        })
    }

    /// Run the sentinel system
    async fn run(self) -> Result<()> {
        info!("Starting Sentinel services...");

        let sentinel = Arc::new(self);

        // Start API server in background
        let api_server = {
            let sentinel = sentinel.clone();
            tokio::spawn(async move {
                sentinel.start_api_server().await
            })
        };

        // Start ingestion pipeline
        let ingestion_pipeline = {
            let sentinel = sentinel.clone();
            tokio::spawn(async move {
                sentinel.start_ingestion_pipeline().await
            })
        };

        // Wait for shutdown signal
        let shutdown = tokio::spawn(async {
            wait_for_shutdown().await;
            info!("Shutdown signal received");
        });

        // Run all tasks concurrently
        tokio::select! {
            result = api_server => {
                error!("API server exited: {:?}", result);
            }
            result = ingestion_pipeline => {
                error!("Ingestion pipeline exited: {:?}", result);
            }
            _ = shutdown => {
                info!("Initiating graceful shutdown...");
            }
        }

        info!("Sentinel stopped");

        Ok(())
    }

    /// Start API server
    async fn start_api_server(self: &Self) -> Result<()> {
        let api_config = ApiConfig {
            bind_addr: format!("{}:{}", self.config.server.host, self.config.server.port)
                .parse()
                .context("Invalid server bind address")?,
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            timeout_secs: self.config.server.request_timeout_secs,
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_logging: true,
            metrics_path: "/metrics".to_string(),
        };
        let storage: Arc<dyn Storage> = self.storage.clone();

        let server = ApiServer::new(
            api_config,
            storage,
            env!("CARGO_PKG_VERSION").to_string(),
        );

        server.serve().await
            .map_err(|e| anyhow::anyhow!("API server error: {}", e))?;

        Ok(())
    }

    /// Start ingestion and detection pipeline
    async fn start_ingestion_pipeline(self: &Self) -> Result<()> {
        info!("Starting Kafka ingestion pipeline...");

        let kafka_config = self.config.ingestion.kafka.as_ref()
            .context("Kafka configuration is required")?;
        let mut ingester = KafkaIngester::new(
            kafka_config,
            self.config.ingestion.batch_size,
            self.config.ingestion.batch_timeout_ms,
        ).context("Failed to create Kafka ingester")?;

        info!("Ingestion pipeline ready, consuming from Kafka...");

        loop {
            match ingester.next_batch().await {
                Ok(events) => {
                    if events.is_empty() {
                        continue;
                    }

                    let event_count = events.len();
                    info!("Received batch of {} telemetry events", event_count);

                    // Process each event
                    for event in &events {
                        // Store telemetry
                        if let Err(e) = self.storage.write_telemetry(&event).await {
                            error!("Failed to write telemetry: {}", e);
                            ::metrics::counter!("sentinel_storage_errors_total")
                                .increment(1);
                        }

                        // Run detection
                        match self.detection_engine.lock().await.process(&event).await {
                            Ok(Some(anomaly)) => {
                                info!(
                                    alert_id = %anomaly.alert_id,
                                    severity = ?anomaly.severity,
                                    anomaly_type = ?anomaly.anomaly_type,
                                    "Anomaly detected"
                                );

                                // Store anomaly
                                if let Err(e) = self.storage.write_anomaly(&anomaly).await {
                                    error!("Failed to write anomaly: {}", e);
                                }

                                // Check deduplication
                                if self.deduplicator.should_send(&anomaly) {
                                    // Send alert
                                    if let Err(e) = self.alerter.send(&anomaly).await {
                                        error!("Failed to send alert: {}", e);
                                        ::metrics::counter!("sentinel_alert_failures_total")
                                            .increment(1);
                                    }
                                } else {
                                    info!(
                                        alert_id = %anomaly.alert_id,
                                        "Alert deduplicated"
                                    );
                                }
                            }
                            Ok(None) => {
                                // No anomaly detected
                                ::metrics::counter!("sentinel_events_normal_total")
                                    .increment(1);
                            }
                            Err(e) => {
                                error!("Detection failed: {}", e);
                                ::metrics::counter!("sentinel_detection_errors_total")
                                    .increment(1);
                            }
                        }
                    }

                    ::metrics::counter!("sentinel_events_processed_total")
                        .increment(event_count as u64);
                }
                Err(e) => {
                    error!("Ingestion error: {}", e);
                    ::metrics::counter!("sentinel_ingestion_errors_total").increment(1);

                    // Backoff on errors
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}

/// Wait for shutdown signal (SIGTERM or CTRL+C)
async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received CTRL+C");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
    }
}
