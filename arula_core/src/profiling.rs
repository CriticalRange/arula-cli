//! Performance profiling utilities for ARULA
//!
//! This module provides simple profiling tools to identify hot paths
//! and optimize performance-critical code sections.

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple performance profiler that tracks execution times
pub struct Profiler {
    measurements: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
}

impl Profiler {
    /// Create a new profiler instance
    pub fn new() -> Self {
        Self {
            measurements: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start timing a new operation
    pub async fn start_timing(&self, name: &str) -> TimingGuard {
        TimingGuard {
            name: name.to_string(),
            start: Instant::now(),
            measurements: self.measurements.clone(),
        }
    }

    /// Get statistics for all measured operations
    pub async fn get_stats(&self) -> HashMap<String, ProfilerStats> {
        let measurements = self.measurements.read().await;
        let mut stats = HashMap::new();

        for (name, durations) in measurements.iter() {
            if durations.is_empty() {
                continue;
            }

            let total: Duration = durations.iter().sum();
            let min = *durations.iter().min().unwrap();
            let max = *durations.iter().max().unwrap();
            let avg = total / durations.len() as u32;

            stats.insert(name.clone(), ProfilerStats {
                count: durations.len(),
                total,
                average: avg,
                min,
                max,
            });
        }

        stats
    }

    /// Clear all measurements
    pub async fn clear(&self) {
        self.measurements.write().await.clear();
    }
}

/// RAII guard that automatically records timing when dropped
pub struct TimingGuard {
    name: String,
    start: Instant,
    measurements: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        // Use try_write to avoid blocking in Drop
        if let Ok(mut measurements) = self.measurements.try_write() {
            measurements
                .entry(self.name.clone())
                .or_insert_with(Vec::new)
                .push(duration);
        }
    }
}

/// Statistics for a profiled operation
#[derive(Debug, Clone)]
pub struct ProfilerStats {
    /// Number of times the operation was measured
    pub count: usize,
    /// Total time spent in the operation
    pub total: Duration,
    /// Average time per operation
    pub average: Duration,
    /// Minimum time recorded
    pub min: Duration,
    /// Maximum time recorded
    pub max: Duration,
}

impl ProfilerStats {
    /// Format statistics for display
    pub fn format(&self) -> String {
        format!(
            "Count: {}, Total: {:?}, Avg: {:?}, Min: {:?}, Max: {:?}",
            self.count,
            self.total,
            self.average,
            self.min,
            self.max
        )
    }
}

/// Macro for easy profiling of code blocks
#[macro_export]
macro_rules! profile_async {
    ($profiler:expr, $name:expr, $body:expr) => {{
        let _guard = $profiler.start_timing($name).await;
        $body.await
    }};
}

/// Macro for easy profiling of sync code blocks
#[macro_export]
macro_rules! profile {
    ($profiler:expr, $name:expr, $body:block) => {{
        let _guard = $profiler.start_timing($name).await;
        $body
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_profiler() {
        let profiler = Profiler::new();

        // Simulate some work
        {
            let _guard = profiler.start_timing("test_operation").await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        {
            let _guard = profiler.start_timing("test_operation").await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let stats = profiler.get_stats().await;
        let test_stats = stats.get("test_operation").unwrap();

        assert_eq!(test_stats.count, 2);
        assert!(test_stats.total >= Duration::from_millis(30));
        assert!(test_stats.min >= Duration::from_millis(10));
        assert!(test_stats.max >= Duration::from_millis(20));
    }

    #[tokio::test]
    async fn test_profiler_macro() {
        let profiler = Profiler::new();

        profile_async!(profiler, "macro_test", async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });

        let stats = profiler.get_stats().await;
        assert!(stats.contains_key("macro_test"));
    }
}