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
use llm_sentinel_alerting::{prelude::*, rabbitmq::RetryConfig};
use llm_sentinel_api::prelude::*;
use llm_sentinel_core::config::Config;
use llm_sentinel_detection::prelude::*;
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
