//! LLM-Sentinel Main Binary
//!
//! Orchestrates all components of the sentinel system:
//! - Ingestion: Kafka consumer for telemetry
//! - Detection: Multi-detector anomaly detection engine
//! - Storage: InfluxDB time-series storage
//! - Alerting: RabbitMQ alert publisher
//! - API: REST API server

use anyhow::{Context, Result};
use clap::Parser;
use llm_sentinel_alerting::prelude::*;
use llm_sentinel_api::prelude::*;
use llm_sentinel_core::{config::Config, prelude::*};
use llm_sentinel_detection::prelude::*;
use llm_sentinel_ingestion::prelude::*;
use llm_sentinel_storage::prelude::*;
use std::{path::PathBuf, sync::Arc};
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// LLM-Sentinel CLI arguments
#[derive(Debug, Parser)]
#[clap(name = "sentinel", version, about = "LLM observability and anomaly detection")]
struct Cli {
    /// Configuration file path
    #[clap(short, long, default_value = "config/sentinel.yaml")]
    config: PathBuf,

    /// Log level (trace, debug, info, warn, error)
    #[clap(long, env = "SENTINEL_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Enable JSON logging
    #[clap(long, env = "SENTINEL_LOG_JSON")]
    log_json: bool,

    /// Dry run mode (don't start services)
    #[clap(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli)?;

    info!("Starting LLM-Sentinel v{}", env!("CARGO_PKG_VERSION"));
    info!("Loading configuration from: {:?}", cli.config);

    // Load configuration
    let config = Config::from_file(&cli.config)
        .context("Failed to load configuration")?;

    info!("Configuration loaded successfully");

    if cli.dry_run {
        info!("Dry run mode - configuration validated, exiting");
        return Ok(());
    }

    // Initialize components
    let sentinel = Sentinel::new(config).await?;

    // Run the sentinel
    sentinel.run().await?;

    Ok(())
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
    detection_engine: Arc<DetectionEngine>,
    alerter: Arc<RabbitMqAlerter>,
    deduplicator: Arc<AlertDeduplicator>,
}

impl Sentinel {
    /// Create a new Sentinel instance
    async fn new(config: Config) -> Result<Self> {
        info!("Initializing Sentinel components...");

        // Initialize storage
        info!("Connecting to InfluxDB...");
        let storage = InfluxDbStorage::new(config.storage.influxdb.clone())
            .await
            .context("Failed to initialize storage")?;
        let storage = Arc::new(storage);
        info!("InfluxDB connected");

        // Initialize detection engine
        info!("Initializing detection engine...");
        let detection_engine = Arc::new(
            DetectionEngine::from_config(config.detection.clone())
                .context("Failed to create detection engine")?,
        );
        info!("Detection engine initialized with {} detectors",
            config.detection.enabled_detectors.len());

        // Initialize alerting
        info!("Connecting to RabbitMQ...");
        let alerter = RabbitMqAlerter::new(config.alerting.rabbitmq.clone())
            .await
            .context("Failed to initialize RabbitMQ alerter")?;
        let alerter = Arc::new(alerter);
        info!("RabbitMQ connected");

        // Initialize deduplicator
        let deduplicator = Arc::new(AlertDeduplicator::new(
            config.alerting.deduplication.clone(),
        ));

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

        // Start API server in background
        let api_server = self.start_api_server();

        // Start ingestion pipeline
        let ingestion_pipeline = self.start_ingestion_pipeline();

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
        let api_config = self.config.api.clone();
        let storage: Arc<dyn Storage> = self.storage.clone();

        let server = ApiServer::new(
            api_config,
            storage,
            env!("CARGO_PKG_VERSION").to_string(),
        );

        server.serve().await?;

        Ok(())
    }

    /// Start ingestion and detection pipeline
    async fn start_ingestion_pipeline(self: Self) -> Result<()> {
        info!("Starting Kafka ingestion pipeline...");

        let mut ingester = KafkaIngester::new(self.config.ingestion.kafka.clone())
            .await
            .context("Failed to create Kafka ingester")?;

        let parser = OtlpParser::new(self.config.ingestion.parsing.clone());
        let validator = EventValidator::new(self.config.ingestion.validation.clone());

        info!("Ingestion pipeline ready, consuming from Kafka...");

        loop {
            match ingester.next_batch().await {
                Ok(events) => {
                    if events.is_empty() {
                        continue;
                    }

                    info!("Received batch of {} telemetry events", events.len());

                    // Process each event
                    for event in events {
                        // Validate event
                        if let Err(e) = validator.validate(&event) {
                            warn!("Event validation failed: {}", e);
                            metrics::counter!("sentinel_validation_failures_total")
                                .increment(1);
                            continue;
                        }

                        // Store telemetry
                        if let Err(e) = self.storage.write_telemetry(&event).await {
                            error!("Failed to write telemetry: {}", e);
                            metrics::counter!("sentinel_storage_errors_total")
                                .increment(1);
                        }

                        // Run detection
                        match self.detection_engine.process(&event).await {
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
                                        metrics::counter!("sentinel_alert_failures_total")
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
                                metrics::counter!("sentinel_events_normal_total")
                                    .increment(1);
                            }
                            Err(e) => {
                                error!("Detection failed: {}", e);
                                metrics::counter!("sentinel_detection_errors_total")
                                    .increment(1);
                            }
                        }
                    }

                    metrics::counter!("sentinel_events_processed_total")
                        .increment(events.len() as u64);
                }
                Err(e) => {
                    error!("Ingestion error: {}", e);
                    metrics::counter!("sentinel_ingestion_errors_total").increment(1);

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
