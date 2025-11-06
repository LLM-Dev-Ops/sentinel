# llm-sentinel-detection

Multi-algorithm anomaly detection engine for LLM telemetry data.

## Overview

This crate implements four complementary statistical anomaly detection algorithms:

- **Z-Score Detection**: Parametric detection for normally distributed metrics
- **IQR Detection**: Non-parametric outlier detection using interquartile range
- **MAD Detection**: Robust detection using median absolute deviation
- **CUSUM Detection**: Cumulative sum change point detection for drift

## Features

- Sub-5ms P50 detection latency
- Multi-dimensional baseline tracking (per service × model × metric)
- Adaptive baselines with automatic updates
- Lock-free concurrent baseline updates using DashMap
- Baseline persistence to disk
- Configurable thresholds and sensitivity

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
llm-llm-sentinel-detection = "0.1.0"
```

## Example

```rust
use llm_sentinel_detection::{DetectionEngine, DetectionConfig, ZScoreDetector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DetectionConfig {
        enabled_detectors: vec!["zscore".to_string(), "iqr".to_string()],
        baseline_window_size: 1000,
        ..Default::default()
    };

    let engine = DetectionEngine::new(config);

    // Detect anomalies in telemetry
    if let Some(anomaly) = engine.detect(&event).await? {
        println!("Anomaly detected: {:?}", anomaly);
    }

    Ok(())
}
```

## Algorithms

### Z-Score Detection
Detects values that deviate significantly from the mean (default: 3σ threshold).

### IQR Detection
Identifies outliers beyond Q1 - 1.5×IQR and Q3 + 1.5×IQR.

### MAD Detection
Ultra-robust detection using median and median absolute deviation.

### CUSUM Detection
Detects sustained shifts and gradual drift in metrics.

## License

Apache-2.0
