//! LLM-Sentinel Benchmarks Library
//!
//! This crate provides the canonical benchmark interface for LLM-Sentinel.
//! It implements the standardized benchmark framework required across all
//! benchmark-target repositories.
//!
//! ## Canonical Components
//!
//! - **`BenchmarkResult`**: Standardized struct with `target_id: String`,
//!   `metrics: serde_json::Value`, and `timestamp: chrono::DateTime<chrono::Utc>`
//!
//! - **`run_all_benchmarks()`**: Entrypoint function that returns `Vec<BenchmarkResult>`
//!
//! - **`BenchTarget`**: Trait with `id()` and `run()` methods for benchmark adapters
//!
//! - **`all_targets()`**: Registry function returning `Vec<Box<dyn BenchTarget>>`
//!
//! ## Module Structure
//!
//! - `result.rs` - BenchmarkResult struct definition
//! - `io.rs` - I/O operations for benchmark results
//! - `markdown.rs` - Markdown summary generation
//! - `adapters.rs` - BenchTarget trait and implementations
//!
//! ## Output Directories
//!
//! - `benchmarks/output/` - Main output directory
//! - `benchmarks/output/raw/` - Individual JSON result files
//! - `benchmarks/output/summary.md` - Generated summary
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use llm_sentinel_benchmarks::{run_all_benchmarks, BenchmarkResult};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Run all benchmarks
//!     let results: Vec<BenchmarkResult> = run_all_benchmarks().await;
//!
//!     // Print results
//!     for result in &results {
//!         println!("Target: {}", result.target_id);
//!         println!("Metrics: {}", result.metrics);
//!         println!("Timestamp: {}", result.timestamp);
//!         println!();
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Writing Results
//!
//! ```rust,ignore
//! use llm_sentinel_benchmarks::{run_and_save_benchmarks};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let (results, summary_path) = run_and_save_benchmarks().await?;
//!     println!("Summary written to: {:?}", summary_path);
//!     Ok(())
//! }
//! ```

// Module declarations
mod adapters;
mod io;
mod markdown;
mod result;

// Re-export the main module's contents
pub use adapters::{
    all_targets, all_targets_arc, BenchTarget, CusumDetectorTarget, DetectionEngineTarget,
    EventProcessingTarget, IqrDetectorTarget, MadDetectorTarget, ZScoreDetectorTarget,
};
pub use io::{BenchmarkIO, DEFAULT_OUTPUT_DIR, RAW_OUTPUT_DIR, SUMMARY_FILE};
pub use markdown::{generate_comparison, generate_summary};
pub use result::BenchmarkResult;

use tracing::{debug, error, info};

/// Canonical entrypoint that runs all benchmarks and returns results.
///
/// This function is the main entry point for the benchmark interface. It:
/// 1. Retrieves all registered benchmark targets via `all_targets()`
/// 2. Executes each benchmark target sequentially
/// 3. Collects and returns all `BenchmarkResult` instances
///
/// # Returns
///
/// A `Vec<BenchmarkResult>` containing results from all executed benchmarks.
/// Failed benchmarks are logged but do not stop execution of remaining benchmarks.
///
/// # Example
///
/// ```rust,ignore
/// use llm_sentinel_benchmarks::run_all_benchmarks;
///
/// #[tokio::main]
/// async fn main() {
///     let results = run_all_benchmarks().await;
///     println!("Executed {} benchmarks", results.len());
/// }
/// ```
pub async fn run_all_benchmarks() -> Vec<BenchmarkResult> {
    info!("Starting benchmark execution...");

    let targets = all_targets();
    info!("Found {} benchmark targets", targets.len());

    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        let target_id = target.id().to_string();
        debug!("Running benchmark: {}", target_id);

        match target.run().await {
            Ok(result) => {
                info!("Benchmark '{}' completed successfully", result.target_id);
                results.push(result);
            }
            Err(e) => {
                error!("Benchmark '{}' failed: {}", target_id, e);
            }
        }
    }

    info!(
        "Benchmark execution complete: {}/{} succeeded",
        results.len(),
        all_targets().len()
    );

    results
}

/// Run all benchmarks in parallel for faster execution.
///
/// This function executes all benchmark targets concurrently using tokio::spawn.
/// Use this when benchmarks are independent and you want faster overall execution.
pub async fn run_all_benchmarks_parallel() -> Vec<BenchmarkResult> {
    info!("Starting parallel benchmark execution...");

    let targets = all_targets_arc();
    info!("Found {} benchmark targets", targets.len());

    let handles: Vec<_> = targets
        .into_iter()
        .map(|target| {
            tokio::spawn(async move {
                let target_id = target.id().to_string();
                debug!("Running benchmark: {}", target_id);

                match target.run().await {
                    Ok(result) => {
                        info!("Benchmark '{}' completed successfully", result.target_id);
                        Some(result)
                    }
                    Err(e) => {
                        error!("Benchmark '{}' failed: {}", target_id, e);
                        None
                    }
                }
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());

    for handle in handles {
        match handle.await {
            Ok(Some(result)) => results.push(result),
            Ok(None) => {} // Benchmark failed, already logged
            Err(e) => error!("Task join error: {}", e),
        }
    }

    info!(
        "Parallel benchmark execution complete: {} succeeded",
        results.len()
    );

    results
}

/// Run benchmarks and write results to the canonical output directories.
///
/// This function:
/// 1. Runs all benchmarks via `run_all_benchmarks()`
/// 2. Writes individual results to `benchmarks/output/raw/`
/// 3. Generates and writes a summary to `benchmarks/output/summary.md`
///
/// # Returns
///
/// A tuple of (results, summary_path) on success.
pub async fn run_and_save_benchmarks() -> anyhow::Result<(Vec<BenchmarkResult>, std::path::PathBuf)>
{
    let results = run_all_benchmarks().await;

    let io = BenchmarkIO::default();

    // Write individual results
    io.write_results(&results)?;

    // Generate and write summary
    let summary = generate_summary(&results);
    let summary_path = io.write_summary(&summary)?;

    Ok((results, summary_path))
}

/// Prelude module for convenient imports.
pub mod prelude {
    pub use super::adapters::{all_targets, BenchTarget};
    pub use super::io::BenchmarkIO;
    pub use super::markdown::generate_summary;
    pub use super::result::BenchmarkResult;
    pub use super::run_all_benchmarks;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_all_benchmarks() {
        let results = run_all_benchmarks().await;
        assert!(!results.is_empty());

        for result in &results {
            assert!(!result.target_id.is_empty());
            assert!(!result.metrics.is_null());
        }
    }

    #[tokio::test]
    async fn test_run_all_benchmarks_parallel() {
        let results = run_all_benchmarks_parallel().await;
        assert!(!results.is_empty());

        for result in &results {
            assert!(!result.target_id.is_empty());
            assert!(!result.metrics.is_null());
        }
    }
}
