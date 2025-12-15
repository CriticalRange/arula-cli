//! Async optimizations using Rust 1.75+ features
//!
//! This module demonstrates and provides optimized async patterns
//! using async fn in traits and other recent improvements.

use std::future::Future;
use anyhow::Result;

/// Async trait with async fn in traits (stable since Rust 1.75)
/// This is more ergonomic than the old async-trait approach
#[allow(async_fn_in_trait)]
pub trait AsyncProcessor {
    type Input;
    type Output;

    async fn process(&self, input: Self::Input) -> Result<Self::Output>;
    async fn batch_process(&self, inputs: Vec<Self::Input>) -> Result<Vec<Self::Output>>
    where
        Self: Clone,
    {
        // Process in parallel using async closures (Rust 2024)
        let tasks: Vec<_> = inputs
            .into_iter()
            .map(|input| {
                let processor = self.clone();
                async move { processor.process(input).await }
            })
            .collect();

        // Run all tasks concurrently
        futures::future::join_all(tasks).await.into_iter().collect()
    }
}

/// Optimized streaming processor using async fn in traits
#[allow(async_fn_in_trait)]
pub trait AsyncStreamProcessor {
    type Item;
    type Result;

    async fn process_stream(
        &self,
        items: impl futures::Stream<Item = Self::Item> + Send + 'static,
    ) -> impl futures::Stream<Item = Self::Result> + Send + '_;
}

/// Example implementation for text processing
#[derive(Clone)]
pub struct TextProcessor {
    pub model: String,
}

impl AsyncProcessor for TextProcessor {
    type Input = String;
    type Output = String;

    async fn process(&self, input: Self::Input) -> Result<Self::Output> {
        // Simulate async processing (e.g., calling an AI API)
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        Ok(format!("[{}] {}", self.model, input))
    }
}

/// Optimized tool execution pattern
pub struct OptimizedToolExecutor {
    /// Using OnceLock instead of lazy_static for better performance
    config: std::sync::OnceLock<ToolConfig>,
}

#[derive(Clone)]
struct ToolConfig {
    timeout: std::time::Duration,
    retry_count: usize,
}

impl OptimizedToolExecutor {
    pub fn new() -> Self {
        Self {
            config: std::sync::OnceLock::new(),
        }
    }

    pub fn get_config(&self) -> &ToolConfig {
        self.config.get_or_init(|| ToolConfig {
            timeout: std::time::Duration::from_secs(30),
            retry_count: 3,
        })
    }

    /// Execute tool with automatic retries using async closures
    pub async fn execute_with_retry<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>> + Send,
    {
        let config = self.get_config();
        let mut last_error = None;

        for attempt in 0..=config.retry_count {
            match tokio::time::timeout(config.timeout, f()).await {
                Ok(Ok(result)) => return Ok(result),
                Ok(Err(e)) => last_error = Some(e),
                Err(_) => last_error = Some(anyhow::anyhow!("Tool execution timed out")),
            }

            if attempt < config.retry_count {
                tokio::time::sleep(std::time::Duration::from_millis(100 * (attempt + 1) as u64)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retries failed")))
    }
}

/// Optimized async iterator for processing large datasets
pub struct AsyncIterator<I, F, Fut>
where
    F: Fn(I) -> Fut,
    Fut: Future,
{
    items: Vec<I>,
    processor: F,
    concurrency: usize,
}

impl<I, F, Fut> AsyncIterator<I, F, Fut>
where
    F: Fn(I) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future + Send + 'static,
{
    pub fn new(items: Vec<I>, processor: F) -> Self {
        Self {
            items,
            processor,
            concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4) as usize,
        }
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency;
        self
    }

    pub async fn process_all<T>(self) -> Vec<T>
    where
        Fut: Future<Output = T>,
    {
        use futures::stream::StreamExt;

        futures::stream::iter(self.items)
            .map(|item| {
                let processor = self.processor.clone();
                async move { processor(item).await }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await
    }
}

/// Performance improvements for async channel usage
pub mod channels {
    use tokio::sync::mpsc;

    /// Create a bounded channel with optimal buffer size based on runtime
    pub fn optimal_bounded_channel<T>() -> (mpsc::Sender<T>, mpsc::Receiver<T>) {
        // Use runtime's thread count as buffer size for optimal performance
        let buffer_size = std::thread::available_parallelism()
            .map(|n| n.get() * 2)
            .unwrap_or(8);

        mpsc::channel(buffer_size)
    }

    /// Optimized batch processor that collects items up to a size or timeout
    pub async fn batch_collector<T: Send + 'static>(
        receiver: mpsc::Receiver<T>,
        batch_size: usize,
        timeout_duration: std::time::Duration,
    ) -> impl futures::Stream<Item = Vec<T>> + Send + 'static {
        use futures::stream::unfold;

        unfold(
            (Vec::<T>::new(), receiver),
            move |(_batch, mut rx)| async move {
                use tokio::time::timeout;

                // Try to collect a batch with timeout
                let mut items = Vec::with_capacity(batch_size);

                // Collect first item with timeout
                match timeout(timeout_duration, rx.recv()).await {
                    Ok(Some(item)) => items.push(item),
                    _ => {
                        // Either timeout or channel closed
                        return if items.is_empty() { None } else { Some((items, (Vec::<T>::new(), rx))) };
                    }
                }

                // Collect remaining items without additional timeout
                while items.len() < batch_size {
                    match rx.recv().await {
                        Some(item) => items.push(item),
                        None => break, // Channel closed
                    }
                }

                Some((items, (Vec::<T>::new(), rx)))
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_text_processor() {
        let processor = TextProcessor {
            model: "gpt-4".to_string(),
        };

        let input = "Hello, world!".to_string();
        let result = processor.process(input).await.unwrap();
        assert_eq!(result, "[gpt-4] Hello, world!");
    }

    #[tokio::test]
    async fn test_batch_processing() {
        let processor = TextProcessor {
            model: "gpt-4".to_string(),
        };

        let inputs = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let results = processor.batch_process(inputs).await.unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.starts_with("[gpt-4]")));
    }

    #[tokio::test]
    async fn test_optimized_tool_executor() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let executor = OptimizedToolExecutor::new();
        let call_count = AtomicUsize::new(0);

        let result = executor
            .execute_with_retry(|| {
                let count = call_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 3 {
                        Err(anyhow::anyhow!("Simulated failure"))
                    } else {
                        Ok("success".to_string())
                    }
                }
            })
            .await
            .unwrap();

        assert_eq!(result, "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_async_iterator() {
        let items = vec![1, 2, 3, 4, 5];
        let processor = |x| async move { x * 2 };

        let results = AsyncIterator::new(items, processor)
            .with_concurrency(2)
            .process_all()
            .await;

        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }

    #[tokio::test]
    async fn test_batch_collector() {
        use tokio::sync::mpsc;
        use futures::StreamExt;

        let (tx, rx) = mpsc::channel(10);

        // Send some items
        for i in 0..5 {
            tx.send(i).await.unwrap();
        }
        drop(tx); // Close the sender

        let stream = channels::batch_collector(rx, 3, std::time::Duration::from_millis(100));
        let first_batch = stream.next().await.unwrap();

        assert_eq!(first_batch.len(), 3);
    }
}