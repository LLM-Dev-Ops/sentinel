//! Statistical utility functions for anomaly detection.

use statrs::statistics::{Data, Distribution, OrderStatistics};

/// Calculate mean of a slice
pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Calculate standard deviation of a slice
pub fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }

    let data_obj = Data::new(data.to_vec());
    data_obj.std_dev().unwrap_or(0.0)
}

/// Calculate median of a slice
pub fn median(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Calculate median absolute deviation (MAD)
pub fn mad(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let med = median(data);
    let deviations: Vec<f64> = data.iter().map(|x| (x - med).abs()).collect();
    median(&deviations)
}

/// Calculate interquartile range (IQR)
pub fn iqr(data: &[f64]) -> (f64, f64, f64) {
    if data.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut data_obj = Data::new(sorted);
    let q1 = data_obj.lower_quartile();
    let q3 = data_obj.upper_quartile();
    let iqr_value = q3 - q1;

    (q1, q3, iqr_value)
}

/// Calculate percentile
pub fn percentile(data: &[f64], p: f64) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut data_obj = Data::new(sorted);
    data_obj.percentile(p as usize)
}

/// Z-score calculation
pub fn zscore(value: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev == 0.0 {
        return 0.0;
    }
    (value - mean) / std_dev
}

/// Check if value is outlier using Z-score
pub fn is_zscore_outlier(value: f64, mean: f64, std_dev: f64, threshold: f64) -> bool {
    zscore(value, mean, std_dev).abs() > threshold
}

/// Check if value is outlier using IQR method
pub fn is_iqr_outlier(value: f64, q1: f64, q3: f64, iqr: f64, multiplier: f64) -> bool {
    let lower_bound = q1 - multiplier * iqr;
    let upper_bound = q3 + multiplier * iqr;
    value < lower_bound || value > upper_bound
}

/// Check if value is outlier using MAD method
pub fn is_mad_outlier(value: f64, median: f64, mad: f64, threshold: f64) -> bool {
    if mad == 0.0 {
        return false;
    }
    // Modified Z-score using MAD
    let modified_zscore = 0.6745 * (value - median).abs() / mad;
    modified_zscore > threshold
}

/// Rolling window statistics
#[derive(Debug, Clone)]
pub struct RollingWindow {
    data: Vec<f64>,
    capacity: usize,
}

impl RollingWindow {
    /// Create a new rolling window
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Add a value to the window
    pub fn push(&mut self, value: f64) {
        if self.data.len() >= self.capacity {
            self.data.remove(0);
        }
        self.data.push(value);
    }

    /// Get the current data
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Check if window is full
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.capacity
    }

    /// Get window size
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if window is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Calculate mean of window
    pub fn mean(&self) -> f64 {
        mean(&self.data)
    }

    /// Calculate standard deviation of window
    pub fn std_dev(&self) -> f64 {
        std_dev(&self.data)
    }

    /// Calculate median of window
    pub fn median(&self) -> f64 {
        median(&self.data)
    }

    /// Calculate MAD of window
    pub fn mad(&self) -> f64 {
        mad(&self.data)
    }

    /// Clear the window
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[5.0]), 5.0);
    }

    #[test]
    fn test_std_dev() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&data);
        assert_relative_eq!(sd, 2.0, epsilon = 0.1);
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[5.0]), 5.0);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn test_mad() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mad_value = mad(&data);
        assert_eq!(mad_value, 1.0); // Median is 3, deviations are [2,1,0,1,2], median is 1
    }

    #[test]
    fn test_iqr() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let (q1, q3, iqr_value) = iqr(&data);
        assert!(q1 > 0.0);
        assert!(q3 > q1);
        assert_eq!(iqr_value, q3 - q1);
    }

    #[test]
    fn test_zscore() {
        assert_eq!(zscore(5.0, 3.0, 2.0), 1.0);
        assert_eq!(zscore(1.0, 3.0, 2.0), -1.0);
        assert_eq!(zscore(3.0, 3.0, 2.0), 0.0);
    }

    #[test]
    fn test_is_zscore_outlier() {
        assert!(is_zscore_outlier(10.0, 3.0, 2.0, 3.0));
        assert!(!is_zscore_outlier(5.0, 3.0, 2.0, 3.0));
    }

    #[test]
    fn test_is_iqr_outlier() {
        assert!(is_iqr_outlier(100.0, 2.0, 8.0, 6.0, 1.5));
        assert!(!is_iqr_outlier(5.0, 2.0, 8.0, 6.0, 1.5));
    }

    #[test]
    fn test_rolling_window() {
        let mut window = RollingWindow::new(3);
        assert!(window.is_empty());
        assert!(!window.is_full());

        window.push(1.0);
        window.push(2.0);
        window.push(3.0);
        assert!(window.is_full());
        assert_eq!(window.len(), 3);
        assert_eq!(window.mean(), 2.0);

        window.push(4.0); // Should remove 1.0
        assert_eq!(window.data(), &[2.0, 3.0, 4.0]);
        assert_eq!(window.mean(), 3.0);
    }

    #[test]
    fn test_rolling_window_clear() {
        let mut window = RollingWindow::new(5);
        window.push(1.0);
        window.push(2.0);
        window.push(3.0);
        assert_eq!(window.len(), 3);

        window.clear();
        assert!(window.is_empty());
        assert_eq!(window.len(), 0);
    }
}
