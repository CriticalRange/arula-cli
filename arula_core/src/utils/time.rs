//! Time utilities for ARULA
//!
//! Provides human-readable time formatting shared across CLI and Desktop.

use chrono::{DateTime, Utc};

/// Convert a timestamp to a human-readable relative time string.
///
/// # Examples
///
/// - "Just now" (less than 60 seconds ago)
/// - "5m ago" (5 minutes ago)
/// - "2h ago" (2 hours ago)
/// - "3d ago" (3 days ago)
pub fn relative_time(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(timestamp);

    if diff.num_seconds() < 60 {
        "Just now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_just_now() {
        let now = Utc::now();
        assert_eq!(relative_time(now), "Just now");
    }

    #[test]
    fn test_minutes_ago() {
        let timestamp = Utc::now() - Duration::minutes(5);
        assert_eq!(relative_time(timestamp), "5m ago");
    }

    #[test]
    fn test_hours_ago() {
        let timestamp = Utc::now() - Duration::hours(3);
        assert_eq!(relative_time(timestamp), "3h ago");
    }

    #[test]
    fn test_days_ago() {
        let timestamp = Utc::now() - Duration::days(2);
        assert_eq!(relative_time(timestamp), "2d ago");
    }
}
