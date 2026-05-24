//! Relative date formatting service.
//! Implements "X minutes ago" style timestamps per DESIGN_SPEC.md section 3.4.

use std::time::{Duration, SystemTime};

/// Formats a timestamp as a relative string like "Just now", "5 minutes ago", etc.
/// Per DESIGN_SPEC.md:
/// - < 1 min: "Just now"
/// - < 1 hour: "X minutes ago"
/// - < 24 hours: "X hours ago"
/// - < 7 days: "X days ago"
/// - > 7 days: standard date format ("D")
#[derive(Debug, Clone)]
pub struct RelativeDateFormatter {
    // Could add locale support here in the future
}

impl RelativeDateFormatter {
    pub fn new() -> Self {
        RelativeDateFormatter {}
    }

    /// Format a SystemTime as a relative date string.
    pub fn format(&self, time: SystemTime) -> String {
        let now = SystemTime::now();

        match now.duration_since(time) {
            Ok(duration) => self.format_duration(duration),
            Err(_) => {
                // Time is in the future - show standard date
                self.format_standard_date(time)
            }
        }
    }

    /// Format a duration since now as a relative string.
    /// Duration is assumed to be in the past (no error handling needed).
    pub fn format_duration(&self, duration: Duration) -> String {
        let total_seconds = duration.as_secs();

        // < 1 minute: "Just now"
        if total_seconds < 60 {
            return "Just now".to_string();
        }

        // < 1 hour: "X minutes ago"
        if total_seconds < 3600 {
            let minutes = total_seconds / 60;
            return Self::minutes_ago(minutes);
        }

        // < 24 hours: "X hours ago"
        if total_seconds < 86400 {
            let hours = total_seconds / 3600;
            return Self::hours_ago(hours);
        }

        // < 7 days: "X days ago"
        if total_seconds < 604800 {
            let days = total_seconds / 86400;
            return Self::days_ago(days);
        }

        // >= 7 days: standard date - this would need a DateTime to format
        // For now, return "X weeks ago"
        let weeks = total_seconds / 604800;
        Self::weeks_ago(weeks)
    }

    fn minutes_ago(minutes: u64) -> String {
        if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", minutes)
        }
    }

    fn hours_ago(hours: u64) -> String {
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    }

    fn days_ago(days: u64) -> String {
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    }

    fn weeks_ago(weeks: u64) -> String {
        if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{} weeks ago", weeks)
        }
    }

    fn format_standard_date(&self, _time: SystemTime) -> String {
        // In a full implementation, this would format using a DateTime library
        // For now, return a placeholder
        "Earlier".to_string()
    }
}

impl Default for RelativeDateFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatter_new() {
        let formatter = RelativeDateFormatter::new();
        // Should not panic
        drop(formatter);
    }

    #[test]
    fn test_formatter_default() {
        let formatter = RelativeDateFormatter::default();
        drop(formatter);
    }

    #[test]
    fn test_just_now() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        // 30 seconds ago should be "Just now"
        let thirty_secs_ago = now - Duration::from_secs(30);
        assert_eq!(formatter.format(thirty_secs_ago), "Just now");
    }

    #[test]
    fn test_one_minute_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let one_min_ago = now - Duration::from_secs(60);
        assert_eq!(formatter.format(one_min_ago), "1 minute ago");
    }

    #[test]
    fn test_minutes_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let five_mins_ago = now - Duration::from_secs(300);
        assert_eq!(formatter.format(five_mins_ago), "5 minutes ago");
    }

    #[test]
    fn test_one_hour_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        assert_eq!(formatter.format(one_hour_ago), "1 hour ago");
    }

    #[test]
    fn test_hours_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let three_hours_ago = now - Duration::from_secs(10800);
        assert_eq!(formatter.format(three_hours_ago), "3 hours ago");
    }

    #[test]
    fn test_one_day_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let one_day_ago = now - Duration::from_secs(86400);
        assert_eq!(formatter.format(one_day_ago), "1 day ago");
    }

    #[test]
    fn test_days_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let three_days_ago = now - Duration::from_secs(259200);
        assert_eq!(formatter.format(three_days_ago), "3 days ago");
    }

    #[test]
    fn test_one_week_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let one_week_ago = now - Duration::from_secs(604800);
        assert_eq!(formatter.format(one_week_ago), "1 week ago");
    }

    #[test]
    fn test_weeks_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let two_weeks_ago = now - Duration::from_secs(1209600);
        assert_eq!(formatter.format(two_weeks_ago), "2 weeks ago");
    }

    #[test]
    fn test_format_duration_just_now() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(
            formatter.format_duration(Duration::from_secs(0)),
            "Just now"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(30)),
            "Just now"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(59)),
            "Just now"
        );
    }

    #[test]
    fn test_format_duration_minutes() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(
            formatter.format_duration(Duration::from_secs(60)),
            "1 minute ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(120)),
            "2 minutes ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(300)),
            "5 minutes ago"
        );
    }

    #[test]
    fn test_format_duration_hours() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(
            formatter.format_duration(Duration::from_secs(3600)),
            "1 hour ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(7200)),
            "2 hours ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(14400)),
            "4 hours ago"
        );
    }

    #[test]
    fn test_format_duration_days() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(
            formatter.format_duration(Duration::from_secs(86400)),
            "1 day ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(172800)),
            "2 days ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(432000)),
            "5 days ago"
        );
    }

    #[test]
    fn test_format_duration_weeks() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(
            formatter.format_duration(Duration::from_secs(604800)),
            "1 week ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(1209600)),
            "2 weeks ago"
        );
        assert_eq!(
            formatter.format_duration(Duration::from_secs(2419200)),
            "4 weeks ago"
        );
    }

    #[test]
    fn test_future_time() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let future = now + Duration::from_secs(3600);
        // Future time should return "Earlier" (placeholder)
        assert_eq!(formatter.format(future), "Earlier");
    }

    #[test]
    fn test_boundary_59_seconds() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let just_under_minute = now - Duration::from_secs(59);
        assert_eq!(formatter.format(just_under_minute), "Just now");
    }

    #[test]
    fn test_boundary_59_minutes() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let just_under_hour = now - Duration::from_secs(3540); // 59 minutes
        assert_eq!(formatter.format(just_under_hour), "59 minutes ago");
    }

    #[test]
    fn test_boundary_23_hours() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let just_under_day = now - Duration::from_secs(86340); // 23 hours 59 minutes
        assert_eq!(formatter.format(just_under_day), "23 hours ago");
    }

    #[test]
    fn test_boundary_6_days() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        let just_under_week = now - Duration::from_secs(518400); // 6 days
        assert_eq!(formatter.format(just_under_week), "6 days ago");
    }
}
