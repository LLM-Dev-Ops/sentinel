//! I/O operations for benchmark results.
//!
//! This module provides functionality for reading and writing benchmark
//! results to the filesystem in the canonical output directories.

use crate::result::BenchmarkResult;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default output directory for benchmark results.
pub const DEFAULT_OUTPUT_DIR: &str = "benchmarks/output";

/// Subdirectory for raw benchmark results.
pub const RAW_OUTPUT_DIR: &str = "benchmarks/output/raw";

/// Summary markdown file name.
pub const SUMMARY_FILE: &str = "summary.md";

/// I/O handler for benchmark results.
#[derive(Debug, Clone)]
pub struct BenchmarkIO {
    /// Base output directory.
    output_dir: PathBuf,

    /// Raw results subdirectory.
    raw_dir: PathBuf,
}

impl Default for BenchmarkIO {
    fn default() -> Self {
        Self::new(DEFAULT_OUTPUT_DIR)
    }
}

impl BenchmarkIO {
    /// Create a new BenchmarkIO with the specified output directory.
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        let output_dir = output_dir.as_ref().to_path_buf();
        let raw_dir = output_dir.join("raw");

        Self { output_dir, raw_dir }
    }

    /// Get the output directory path.
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Get the raw output directory path.
    pub fn raw_dir(&self) -> &Path {
        &self.raw_dir
    }

    /// Get the summary file path.
    pub fn summary_path(&self) -> PathBuf {
        self.output_dir.join(SUMMARY_FILE)
    }

    /// Ensure all output directories exist.
    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.output_dir)
            .with_context(|| format!("Failed to create output directory: {:?}", self.output_dir))?;

        fs::create_dir_all(&self.raw_dir)
            .with_context(|| format!("Failed to create raw output directory: {:?}", self.raw_dir))?;

        debug!("Ensured benchmark output directories exist");
        Ok(())
    }

    /// Write a single benchmark result to a JSON file.
    pub fn write_result(&self, result: &BenchmarkResult) -> Result<PathBuf> {
        self.ensure_directories()?;

        let filename = format!(
            "{}_{}.json",
            result.target_id,
            result.timestamp.format("%Y%m%d_%H%M%S")
        );
        let path = self.raw_dir.join(&filename);

        let json = result.to_json()?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write result to {:?}", path))?;

        info!("Wrote benchmark result to {:?}", path);
        Ok(path)
    }

    /// Write multiple benchmark results to JSON files.
    pub fn write_results(&self, results: &[BenchmarkResult]) -> Result<Vec<PathBuf>> {
        results.iter().map(|r| self.write_result(r)).collect()
    }

    /// Write all results to a single combined JSON file.
    pub fn write_combined_results(&self, results: &[BenchmarkResult]) -> Result<PathBuf> {
        self.ensure_directories()?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("combined_{}.json", timestamp);
        let path = self.output_dir.join(&filename);

        let json = serde_json::to_string_pretty(results)?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write combined results to {:?}", path))?;

        info!("Wrote combined benchmark results to {:?}", path);
        Ok(path)
    }

    /// Read a benchmark result from a JSON file.
    pub fn read_result(&self, path: impl AsRef<Path>) -> Result<BenchmarkResult> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read result from {:?}", path))?;

        BenchmarkResult::from_json(&content)
            .with_context(|| format!("Failed to parse result from {:?}", path))
    }

    /// Read all benchmark results from the raw directory.
    pub fn read_all_results(&self) -> Result<Vec<BenchmarkResult>> {
        if !self.raw_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for entry in fs::read_dir(&self.raw_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json") {
                match self.read_result(&path) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        debug!("Skipping invalid result file {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(results)
    }

    /// Write the summary markdown file.
    pub fn write_summary(&self, content: &str) -> Result<PathBuf> {
        self.ensure_directories()?;

        let path = self.summary_path();
        fs::write(&path, content)
            .with_context(|| format!("Failed to write summary to {:?}", path))?;

        info!("Wrote benchmark summary to {:?}", path);
        Ok(path)
    }

    /// Read the summary markdown file if it exists.
    pub fn read_summary(&self) -> Result<Option<String>> {
        let path = self.summary_path();

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read summary from {:?}", path))?;

        Ok(Some(content))
    }

    /// Clean all benchmark results from the raw directory.
    pub fn clean_results(&self) -> Result<usize> {
        if !self.raw_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;

        for entry in fs::read_dir(&self.raw_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json") {
                fs::remove_file(&path)?;
                count += 1;
            }
        }

        info!("Cleaned {} benchmark result files", count);
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_benchmark_io_directories() {
        let temp = tempdir().unwrap();
        let io = BenchmarkIO::new(temp.path().join("benchmarks/output"));

        io.ensure_directories().unwrap();

        assert!(io.output_dir().exists());
        assert!(io.raw_dir().exists());
    }

    #[test]
    fn test_benchmark_io_write_read() {
        let temp = tempdir().unwrap();
        let io = BenchmarkIO::new(temp.path().join("benchmarks/output"));

        let result = BenchmarkResult::new("test_target", json!({"latency_ms": 10.5}));
        let path = io.write_result(&result).unwrap();

        assert!(path.exists());

        let read_result = io.read_result(&path).unwrap();
        assert_eq!(result.target_id(), read_result.target_id());
    }

    #[test]
    fn test_benchmark_io_write_summary() {
        let temp = tempdir().unwrap();
        let io = BenchmarkIO::new(temp.path().join("benchmarks/output"));

        let content = "# Benchmark Summary\n\nTest content";
        let path = io.write_summary(content).unwrap();

        assert!(path.exists());

        let read_content = io.read_summary().unwrap().unwrap();
        assert_eq!(content, read_content);
    }
}
