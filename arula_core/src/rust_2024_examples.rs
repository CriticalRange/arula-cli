//! Practical examples of Rust 2024 features in Arula

/// Example: RPIT (Return Position Impl Trait) lifetime capture improvements
///
/// Before Rust 2024, you needed explicit lifetime capture:
fn old_style_iterator<'a>(data: &'a [String]) -> impl Iterator<Item = &'a str> + 'a {
    data.iter().map(|s| s.as_str())
}

/// In Rust 2024, lifetimes are captured automatically:
fn new_style_iterator(data: &[String]) -> impl Iterator<Item = &str> {
    data.iter().map(|s| s.as_str())
}

/// Example: Future is now in the prelude (no need to import)
pub async fn example_future_in_prelude() -> String {
    // std::future::Future is no longer needed to import
    async { "Hello from Rust 2024!".to_string() }.await
}

/// Example: Async closure demonstration
pub async fn demonstrate_async_closures() {
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Async closure that captures from environment
    let increment = {
        let count = count.clone();
        async move || {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        }
    };

    // Call the async closure
    let result = increment().await;
    println!("Count after async closure: {}", result);
}

/// Example: Box<[T]> now implements IntoIterator by value
pub fn box_into_iterator_example() {
    let boxed: Box<[i32]> = Box::new([1, 2, 3, 4, 5]);

    // In Rust 2024, this iterates by value (consumes the box)
    for item in boxed {
        println!("Item: {}", item);
    }
}

/// Example: New tuple FromIterator implementations
pub fn tuple_collector_example() {
    let numbers = 0..5;

    // Collect into multiple collections at once
    let (evens, odds): (Vec<_>, Vec<_>) = numbers
        .map(|n| (n * 2, n * 2 + 1))
        .unzip();

    println!("Evens: {:?}", evens);
    println!("Odds: {:?}", odds);
}

/// Example: improved error handling with more precise diagnostics
pub use std::fmt::Display;

#[derive(Debug)]
pub struct ArulaError {
    message: String,
}

impl Display for ArulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Arula Error: {}", self.message)
    }
}

impl std::error::Error for ArulaError {}

/// Example: Using the new diagnostic attribute
#[diagnostic::do_not_recommend]
impl From<String> for ArulaError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

/// Example: Never type fallback change
// Note: The never type ! is still experimental, so we'll use Infallible instead
pub fn never_type_example() -> Result<String, std::convert::Infallible> {
    // In Rust 2024, never type coercion fallback has changed
    Ok("Success".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_style_iterator() {
        let data = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result: Vec<_> = new_style_iterator(&data).collect();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn test_future_in_prelude() {
        let result = example_future_in_prelude().await;
        assert_eq!(result, "Hello from Rust 2024!");
    }

    #[tokio::test]
    async fn test_async_closures() {
        let mut vector = Vec::new();

        let closure = async || {
            vector.push("Added by async closure");
        };

        closure().await;
        assert_eq!(vector.len(), 1);
    }

    #[test]
    fn test_tuple_collector() {
        let (squares, cubes): (Vec<_>, Vec<_>) = (1..=3)
            .map(|n| (n * n, n * n * n))
            .unzip();

        assert_eq!(squares, vec![1, 4, 9]);
        assert_eq!(cubes, vec![1, 8, 27]);
    }
}