//! Ingestion pipeline orchestration.

use crate::{Ingester, otlp::OtlpParser, validation::EventValidator};
use crossfire::mpsc::{TxFuture, RxFuture};
use llm_sentinel_core::{
    config::IngestionConfig,
    events::TelemetryEvent,
    Result, Error,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Buffer size for the pipeline
    pub buffer_size: usize,
    /// Number of workers for parallel processing
    pub workers: usize,
    /// Enable event validation
    pub enable_validation: bool,
    /// Enable event sanitization
    pub enable_sanitization: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10000,
            workers: 4,
            enable_validation: true,
            enable_sanitization: true,
        }
    }
}

/// Ingestion pipeline that coordinates ingestion, validation, and routing
pub struct IngestionPipeline {
    config: PipelineConfig,
    validator: Arc<EventValidator>,
    parser: Arc<OtlpParser>,
    tx: Option<TxFuture<TelemetryEvent>>,
    rx: Option<RxFuture<TelemetryEvent>>,
    worker_handles: Vec<JoinHandle<()>>,
}

impl IngestionPipeline {
    /// Create a new ingestion pipeline
    pub fn new(config: PipelineConfig) -> Self {
        let (tx, rx) = crossfire::mpsc::unbounded_tx_future_rx();

        Self {
            config,
            validator: Arc::new(EventValidator::default()),
            parser: Arc::new(OtlpParser::default()),
            tx: Some(tx),
            rx: Some(rx),
            worker_handles: Vec::new(),
        }
    }

    /// Get a sender for pushing events into the pipeline
    pub fn sender(&self) -> Result<TxFuture<TelemetryEvent>> {
        self.tx
            .as_ref()
            .map(|tx| tx.clone())
            .ok_or_else(|| Error::internal("Pipeline sender not available"))
    }

    /// Get a receiver for consuming processed events
    pub fn receiver(&mut self) -> Result<RxFuture<TelemetryEvent>> {
        self.rx
            .take()
            .ok_or_else(|| Error::internal("Pipeline receiver already taken"))
    }

    /// Start the pipeline
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting ingestion pipeline with {} workers", self.config.workers);

        let rx = self.receiver()?;

        // Spawn worker tasks
        for worker_id in 0..self.config.workers {
            let rx_clone = rx.clone();
            let validator = Arc::clone(&self.validator);
            let enable_validation = self.config.enable_validation;
            let enable_sanitization = self.config.enable_sanitization;

            let handle = tokio::spawn(async move {
                Self::worker_task(
                    worker_id,
                    rx_clone,
                    validator,
                    enable_validation,
                    enable_sanitization,
                )
                .await;
            });

            self.worker_handles.push(handle);
        }

        info!("Ingestion pipeline started successfully");
        Ok(())
    }

    /// Worker task for processing events
    async fn worker_task(
        worker_id: usize,
        mut rx: RxFuture<TelemetryEvent>,
        validator: Arc<EventValidator>,
        enable_validation: bool,
        enable_sanitization: bool,
    ) {
        debug!("Worker {} started", worker_id);

        loop {
            match rx.recv().await {
                Ok(mut event) => {
                    // Validate event
                    if enable_validation {
                        if let Err(e) = validator.validate(&event) {
                            error!(
                                worker_id,
                                event_id = %event.event_id,
                                "Event validation failed: {}",
                                e
                            );
                            metrics::counter!("sentinel_events_dropped_total",
                                "reason" => "validation_failed"
                            )
                            .increment(1);
                            continue;
                        }
                    }

                    // Sanitize event
                    if enable_sanitization {
                        if let Err(e) = validator.sanitize(&mut event) {
                            warn!(
                                worker_id,
                                event_id = %event.event_id,
                                "Event sanitization failed: {}",
                                e
                            );
                        }
                    }

                    debug!(
                        worker_id,
                        event_id = %event.event_id,
                        "Event processed successfully"
                    );

                    metrics::counter!("sentinel_events_processed_total").increment(1);

                    // Event is ready for detection pipeline
                    // In a full implementation, this would forward to detection engine
                }
                Err(e) => {
                    error!(worker_id, "Worker receive error: {}", e);
                    break;
                }
            }
        }

        debug!("Worker {} stopped", worker_id);
    }

    /// Stop the pipeline gracefully
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping ingestion pipeline");

        // Drop sender to signal workers
        self.tx = None;

        // Wait for workers to complete
        for handle in self.worker_handles.drain(..) {
            if let Err(e) = handle.await {
                error!("Worker join error: {}", e);
            }
        }

        info!("Ingestion pipeline stopped");
        Ok(())
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            workers: self.config.workers,
            buffer_size: self.config.buffer_size,
        }
    }
}

/// Pipeline statistics
#[derive(Debug, Clone)]
pub struct PipelineStats {
    /// Number of worker threads
    pub workers: usize,
    /// Buffer size
    pub buffer_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_sentinel_core::{
        events::{PromptInfo, ResponseInfo},
        types::{ModelId, ServiceId},
    };

    fn create_test_event() -> TelemetryEvent {
        TelemetryEvent::new(
            ServiceId::new("test"),
            ModelId::new("gpt-4"),
            PromptInfo {
                text: "Test".to_string(),
                tokens: 10,
                embedding: None,
            },
            ResponseInfo {
                text: "Response".to_string(),
                tokens: 20,
                finish_reason: "stop".to_string(),
                embedding: None,
            },
            100.0,
            0.001,
        )
    }

    #[tokio::test]
    async fn test_pipeline_creation() {
        let config = PipelineConfig::default();
        let pipeline = IngestionPipeline::new(config);
        assert_eq!(pipeline.config.workers, 4);
    }

    #[tokio::test]
    async fn test_pipeline_sender() {
        let pipeline = IngestionPipeline::new(PipelineConfig::default());
        let sender = pipeline.sender();
        assert!(sender.is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_stats() {
        let pipeline = IngestionPipeline::new(PipelineConfig::default());
        let stats = pipeline.stats();
        assert_eq!(stats.workers, 4);
        assert_eq!(stats.buffer_size, 10000);
    }
}
