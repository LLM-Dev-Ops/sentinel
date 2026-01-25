//! Performance Budget Enforcement
//!
//! Enforces Phase 2 performance constraints:
//! - MAX_TOKENS=1000
//! - MAX_LATENCY_MS=2000
//! - MAX_CALLS_PER_RUN=3

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

/// Default maximum tokens per run
pub const DEFAULT_MAX_TOKENS: u32 = 1000;

/// Default maximum latency in milliseconds
pub const DEFAULT_MAX_LATENCY_MS: u64 = 2000;

/// Default maximum external calls per run
pub const DEFAULT_MAX_CALLS_PER_RUN: u32 = 3;

/// Budget violation types
#[derive(Debug, Clone, Error)]
pub enum BudgetViolation {
    /// Token budget exceeded
    #[error("Token budget exceeded: used {used}, max {max}")]
    TokensExceeded { used: u32, max: u32 },

    /// Latency budget exceeded
    #[error("Latency budget exceeded: took {actual_ms}ms, max {max_ms}ms")]
    LatencyExceeded { actual_ms: u64, max_ms: u64 },

    /// Call count exceeded
    #[error("Call budget exceeded: made {used} calls, max {max}")]
    CallsExceeded { used: u32, max: u32 },
}

/// Performance budget configuration
#[derive(Debug, Clone)]
pub struct BudgetConfig {
    /// Maximum tokens allowed per run
    pub max_tokens: u32,
    /// Maximum latency allowed in milliseconds
    pub max_latency_ms: u64,
    /// Maximum external calls allowed per run
    pub max_calls_per_run: u32,
    /// Whether to enforce budgets strictly (fail on violation)
    pub strict: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_MAX_TOKENS,
            max_latency_ms: DEFAULT_MAX_LATENCY_MS,
            max_calls_per_run: DEFAULT_MAX_CALLS_PER_RUN,
            strict: true,
        }
    }
}

impl BudgetConfig {
    /// Create a budget config from environment variables
    pub fn from_env() -> Self {
        let max_tokens = std::env::var("MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);

        let max_latency_ms = std::env::var("MAX_LATENCY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_LATENCY_MS);

        let max_calls_per_run = std::env::var("MAX_CALLS_PER_RUN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_CALLS_PER_RUN);

        let strict = std::env::var("BUDGET_STRICT")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        Self {
            max_tokens,
            max_latency_ms,
            max_calls_per_run,
            strict,
        }
    }
}

/// Performance budget tracker for a single run
#[derive(Debug)]
pub struct PerformanceBudget {
    /// Configuration
    config: BudgetConfig,
    /// Token usage counter
    tokens_used: AtomicU32,
    /// Call counter
    calls_made: AtomicU32,
    /// Run start time
    start_time: Instant,
    /// Accumulated violations (non-strict mode)
    violations: std::sync::Mutex<Vec<BudgetViolation>>,
}

impl PerformanceBudget {
    /// Create a new performance budget tracker
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            tokens_used: AtomicU32::new(0),
            calls_made: AtomicU32::new(0),
            start_time: Instant::now(),
            violations: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BudgetConfig::default())
    }

    /// Create from environment configuration
    pub fn from_env() -> Self {
        Self::new(BudgetConfig::from_env())
    }

    /// Record token usage
    pub fn record_tokens(&self, count: u32) -> Result<(), BudgetViolation> {
        let total = self.tokens_used.fetch_add(count, Ordering::SeqCst) + count;

        if total > self.config.max_tokens {
            let violation = BudgetViolation::TokensExceeded {
                used: total,
                max: self.config.max_tokens,
            };

            if self.config.strict {
                return Err(violation);
            } else {
                warn!(
                    used = total,
                    max = self.config.max_tokens,
                    "Token budget exceeded (non-strict mode)"
                );
                self.violations.lock().unwrap().push(violation);
            }
        }

        debug!(tokens = count, total = total, "Tokens recorded");
        Ok(())
    }

    /// Record an external call
    pub fn record_call(&self) -> Result<(), BudgetViolation> {
        let total = self.calls_made.fetch_add(1, Ordering::SeqCst) + 1;

        if total > self.config.max_calls_per_run {
            let violation = BudgetViolation::CallsExceeded {
                used: total,
                max: self.config.max_calls_per_run,
            };

            if self.config.strict {
                return Err(violation);
            } else {
                warn!(
                    calls = total,
                    max = self.config.max_calls_per_run,
                    "Call budget exceeded (non-strict mode)"
                );
                self.violations.lock().unwrap().push(violation);
            }
        }

        debug!(calls = total, "Call recorded");
        Ok(())
    }

    /// Check if latency budget is still available
    pub fn check_latency(&self) -> Result<(), BudgetViolation> {
        let elapsed = self.start_time.elapsed().as_millis() as u64;

        if elapsed > self.config.max_latency_ms {
            let violation = BudgetViolation::LatencyExceeded {
                actual_ms: elapsed,
                max_ms: self.config.max_latency_ms,
            };

            if self.config.strict {
                return Err(violation);
            } else {
                warn!(
                    elapsed_ms = elapsed,
                    max_ms = self.config.max_latency_ms,
                    "Latency budget exceeded (non-strict mode)"
                );
                self.violations.lock().unwrap().push(violation);
            }
        }

        Ok(())
    }

    /// Get remaining token budget
    pub fn tokens_remaining(&self) -> u32 {
        let used = self.tokens_used.load(Ordering::SeqCst);
        self.config.max_tokens.saturating_sub(used)
    }

    /// Get remaining call budget
    pub fn calls_remaining(&self) -> u32 {
        let used = self.calls_made.load(Ordering::SeqCst);
        self.config.max_calls_per_run.saturating_sub(used)
    }

    /// Get remaining latency budget in milliseconds
    pub fn latency_remaining_ms(&self) -> u64 {
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        self.config.max_latency_ms.saturating_sub(elapsed)
    }

    /// Get elapsed time since budget started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get current token usage
    pub fn tokens_used(&self) -> u32 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    /// Get current call count
    pub fn calls_made(&self) -> u32 {
        self.calls_made.load(Ordering::SeqCst)
    }

    /// Check if any budget is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.tokens_remaining() == 0
            || self.calls_remaining() == 0
            || self.latency_remaining_ms() == 0
    }

    /// Get all violations (non-strict mode)
    pub fn violations(&self) -> Vec<BudgetViolation> {
        self.violations.lock().unwrap().clone()
    }

    /// Check if there were any violations
    pub fn has_violations(&self) -> bool {
        !self.violations.lock().unwrap().is_empty()
    }

    /// Get budget summary
    pub fn summary(&self) -> BudgetSummary {
        BudgetSummary {
            tokens_used: self.tokens_used(),
            tokens_max: self.config.max_tokens,
            calls_made: self.calls_made(),
            calls_max: self.config.max_calls_per_run,
            latency_ms: self.start_time.elapsed().as_millis() as u64,
            latency_max_ms: self.config.max_latency_ms,
            violations: self.violations(),
        }
    }

    /// Reset the budget for a new run
    pub fn reset(&self) {
        self.tokens_used.store(0, Ordering::SeqCst);
        self.calls_made.store(0, Ordering::SeqCst);
        self.violations.lock().unwrap().clear();
        // Note: start_time cannot be reset; create a new budget for new runs
    }
}

/// Summary of budget usage
#[derive(Debug, Clone)]
pub struct BudgetSummary {
    /// Tokens used
    pub tokens_used: u32,
    /// Maximum tokens allowed
    pub tokens_max: u32,
    /// Calls made
    pub calls_made: u32,
    /// Maximum calls allowed
    pub calls_max: u32,
    /// Elapsed latency in milliseconds
    pub latency_ms: u64,
    /// Maximum latency allowed
    pub latency_max_ms: u64,
    /// Any violations that occurred
    pub violations: Vec<BudgetViolation>,
}

impl BudgetSummary {
    /// Check if budget was within limits
    pub fn within_limits(&self) -> bool {
        self.tokens_used <= self.tokens_max
            && self.calls_made <= self.calls_max
            && self.latency_ms <= self.latency_max_ms
    }
}

/// Guard for automatic call tracking
pub struct CallGuard<'a> {
    budget: &'a PerformanceBudget,
}

impl<'a> CallGuard<'a> {
    /// Create a new call guard (records the call)
    pub fn new(budget: &'a PerformanceBudget) -> Result<Self, BudgetViolation> {
        budget.record_call()?;
        Ok(Self { budget })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_budget() {
        let budget = PerformanceBudget::with_defaults();

        // Should succeed
        assert!(budget.record_tokens(500).is_ok());
        assert_eq!(budget.tokens_used(), 500);
        assert_eq!(budget.tokens_remaining(), 500);

        // Should succeed (at limit)
        assert!(budget.record_tokens(500).is_ok());
        assert_eq!(budget.tokens_used(), 1000);

        // Should fail (over limit)
        let result = budget.record_tokens(1);
        assert!(matches!(result, Err(BudgetViolation::TokensExceeded { .. })));
    }

    #[test]
    fn test_call_budget() {
        let budget = PerformanceBudget::with_defaults();

        // Should succeed for 3 calls
        assert!(budget.record_call().is_ok());
        assert!(budget.record_call().is_ok());
        assert!(budget.record_call().is_ok());
        assert_eq!(budget.calls_made(), 3);

        // Should fail on 4th call
        let result = budget.record_call();
        assert!(matches!(result, Err(BudgetViolation::CallsExceeded { .. })));
    }

    #[test]
    fn test_non_strict_mode() {
        let config = BudgetConfig {
            max_tokens: 100,
            max_latency_ms: 1000,
            max_calls_per_run: 1,
            strict: false,
        };
        let budget = PerformanceBudget::new(config);

        // Should not error in non-strict mode
        assert!(budget.record_tokens(200).is_ok());
        assert!(budget.has_violations());
        assert_eq!(budget.violations().len(), 1);
    }

    #[test]
    fn test_budget_summary() {
        let budget = PerformanceBudget::with_defaults();
        budget.record_tokens(100).unwrap();
        budget.record_call().unwrap();

        let summary = budget.summary();
        assert_eq!(summary.tokens_used, 100);
        assert_eq!(summary.calls_made, 1);
        assert!(summary.within_limits());
    }
}
