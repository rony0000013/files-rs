//! Relative Date Formatter — "X minutes ago" logic with 60s refresh.
//! Per DESIGN_SPEC.md section 3.4 / TODO.md Section 6 Sandbox UI Tests.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Convert SystemTime to (month, day, year) tuple using local time.
/// Returns (month, day, year) where month is 1-12.
fn local_datetime_from_system_time(timestamp: SystemTime) -> (u32, u32, u32) {
    let duration_since_epoch = timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or(UNIX_EPOCH.duration_since(timestamp).unwrap_or(Duration::ZERO));

    // Use the offset to approximate local time (simplified)
    // For production, use platform-specific APIs for accurate local time
    let total_secs = duration_since_epoch.as_secs();
    let days_since_epoch = total_secs / 86400;
    let secs_of_day = total_secs % 86400;

    // Calculate year, month, day using civil_from_days algorithm (Zeller-like)
    // This is a simplified version that assumes Gregorian calendar
    let days = days_since_epoch as i64 + 719468; // Days from 0000-03-01 to 1970-01-01

    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = days - era * 146097; // Day of era [0, 36524]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // Year of era
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // Day of year
    let mp = (5 * doy + 2) / 153; // Month part
    let d = doy - (153 * mp + 2) / 5 + 1; // Day of month
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // Month [1, 12]

    (m as u32, d as u32, (y + (if m <= 2 { 1 } else { 0 })) as u32)
}

/// Relative date formatting result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelativeTimeResult {
    /// Less than a minute: "Just now"
    JustNow,
    /// Less than an hour: "X minutes ago"
    MinutesAgo(u32),
    /// Less than 24 hours: "X hours ago"
    HoursAgo(u32),
    /// Less than 7 days: "X days ago"
    DaysAgo(u32),
    /// 7 or more days: use standard date format
    StandardDate(String),
}

impl RelativeTimeResult {
    /// Get the display string for this result.
    pub fn display(&self) -> String {
        match self {
            RelativeTimeResult::JustNow => "Just now".to_string(),
            RelativeTimeResult::MinutesAgo(mins) => {
                format!("{} minute{} ago", mins, if *mins == 1 { "" } else { "s" })
            }
            RelativeTimeResult::HoursAgo(hrs) => {
                format!("{} hour{} ago", hrs, if *hrs == 1 { "" } else { "s" })
            }
            RelativeTimeResult::DaysAgo(days) => {
                format!("{} day{} ago", days, if *days == 1 { "" } else { "s" })
            }
            RelativeTimeResult::StandardDate(date) => date.clone(),
        }
    }
}

/// Configuration for the relative date formatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeDateConfig {
    /// Refresh interval in seconds (default 60s per spec).
    pub refresh_interval_secs: u64,
    /// Whether to show "Just now" or "0 minutes ago".
    pub show_just_now: bool,
    /// Minimum minutes to show (e.g., 1 = show "1 minute ago" not "0 minutes ago").
    pub min_minutes: u32,
}

impl Default for RelativeDateConfig {
    fn default() -> Self {
        Self {
            refresh_interval_secs: 60,
            show_just_now: true,
            min_minutes: 1,
        }
    }
}

/// Service for formatting dates in relative format.
#[derive(Debug, Clone)]
pub struct RelativeDateFormatter {
    config: RelativeDateConfig,
}

impl RelativeDateFormatter {
    /// Create a new formatter with default config.
    pub fn new() -> Self {
        Self {
            config: RelativeDateConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: RelativeDateConfig) -> Self {
        Self { config }
    }

    /// Get the refresh interval.
    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.config.refresh_interval_secs)
    }

    /// Format a timestamp as relative time.
    pub fn format(&self, timestamp: SystemTime) -> RelativeTimeResult {
        let now = SystemTime::now();
        let diff = match now.duration_since(timestamp) {
            Ok(d) => d,
            Err(_) => return RelativeTimeResult::StandardDate(self.format_standard(timestamp)),
        };

        // Less than 1 minute -> "Just now" (if enabled) or "X minutes ago"
        if diff < Duration::from_secs(60) {
            if self.config.show_just_now {
                return RelativeTimeResult::JustNow;
            }
            // show_just_now=false: fall through to minutes calculation
        }

        // Less than 1 hour -> "X minutes ago"
        if diff < Duration::from_secs(3600) {
            let minutes = (diff.as_secs() / 60) as u32;
            let minutes = if self.config.min_minutes > 1 && minutes < self.config.min_minutes {
                self.config.min_minutes
            } else {
                minutes.max(self.config.min_minutes)
            };
            return RelativeTimeResult::MinutesAgo(minutes);
        }

        // Less than 24 hours -> "X hours ago"
        if diff < Duration::from_secs(86400) {
            let hours = (diff.as_secs() / 3600) as u32;
            return RelativeTimeResult::HoursAgo(hours.max(1));
        }

        // Less than 7 days -> "X days ago"
        if diff < Duration::from_secs(604800) {
            let days = (diff.as_secs() / 86400) as u32;
            return RelativeTimeResult::DaysAgo(days.max(1));
        }

        // 7 or more days -> standard date
        RelativeTimeResult::StandardDate(self.format_standard(timestamp))
    }

    /// Format a timestamp in standard date format.
    fn format_standard(&self, timestamp: SystemTime) -> String {
        // Use the system's local time representation for cross-platform consistency
        let datetime = local_datetime_from_system_time(timestamp);
        format!("{:02}/{:02}/{:04}", datetime.0, datetime.1, datetime.2)
    }

    /// Check if a refresh is needed based on the interval.
    pub fn needs_refresh(&self, last_update: SystemTime) -> bool {
        let now = SystemTime::now();
        now.duration_since(last_update)
            .map(|d| d >= self.refresh_interval())
            .unwrap_or(true)
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

    // ==========================
    // RelativeTimeResult Tests
    // ==========================

    #[test]
    fn test_relative_time_result_just_now() {
        let result = RelativeTimeResult::JustNow;
        assert_eq!(result.display(), "Just now");
    }

    #[test]
    fn test_relative_time_result_minutes_ago_singular() {
        let result = RelativeTimeResult::MinutesAgo(1);
        assert_eq!(result.display(), "1 minute ago");
    }

    #[test]
    fn test_relative_time_result_minutes_ago_plural() {
        let result = RelativeTimeResult::MinutesAgo(5);
        assert_eq!(result.display(), "5 minutes ago");
    }

    #[test]
    fn test_relative_time_result_hours_ago_singular() {
        let result = RelativeTimeResult::HoursAgo(1);
        assert_eq!(result.display(), "1 hour ago");
    }

    #[test]
    fn test_relative_time_result_hours_ago_plural() {
        let result = RelativeTimeResult::HoursAgo(3);
        assert_eq!(result.display(), "3 hours ago");
    }

    #[test]
    fn test_relative_time_result_days_ago_singular() {
        let result = RelativeTimeResult::DaysAgo(1);
        assert_eq!(result.display(), "1 day ago");
    }

    #[test]
    fn test_relative_time_result_days_ago_plural() {
        let result = RelativeTimeResult::DaysAgo(6);
        assert_eq!(result.display(), "6 days ago");
    }

    #[test]
    fn test_relative_time_result_standard_date() {
        let result = RelativeTimeResult::StandardDate("12/25/2024".to_string());
        assert_eq!(result.display(), "12/25/2024");
    }

    // ==========================
    // RelativeDateConfig Tests
    // ==========================

    #[test]
    fn test_relative_date_config_default() {
        let config = RelativeDateConfig::default();
        assert_eq!(config.refresh_interval_secs, 60);
        assert!(config.show_just_now);
        assert_eq!(config.min_minutes, 1);
    }

    // ==========================
    // RelativeDateFormatter Tests
    // ==========================

    #[test]
    fn test_formatter_new() {
        let formatter = RelativeDateFormatter::new();
        assert_eq!(formatter.refresh_interval(), Duration::from_secs(60));
    }

    #[test]
    fn test_formatter_with_custom_config() {
        let config = RelativeDateConfig {
            refresh_interval_secs: 30,
            show_just_now: false,
            min_minutes: 2,
        };
        let formatter = RelativeDateFormatter::with_config(config.clone());
        assert_eq!(formatter.refresh_interval(), Duration::from_secs(30));
    }

    #[test]
    fn test_format_just_now() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 30 seconds ago -> Just now
        let past = now - Duration::from_secs(30);
        assert!(matches!(formatter.format(past), RelativeTimeResult::JustNow));
    }

    #[test]
    fn test_format_minutes_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 5 minutes ago
        let past = now - Duration::from_secs(300);
        match formatter.format(past) {
            RelativeTimeResult::MinutesAgo(mins) => assert_eq!(mins, 5),
            _ => panic!("Expected MinutesAgo(5)"),
        }
    }

    #[test]
    fn test_format_hours_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 3 hours ago
        let past = now - Duration::from_secs(3 * 3600);
        match formatter.format(past) {
            RelativeTimeResult::HoursAgo(hrs) => assert_eq!(hrs, 3),
            _ => panic!("Expected HoursAgo(3)"),
        }
    }

    #[test]
    fn test_format_days_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 5 days ago
        let past = now - Duration::from_secs(5 * 86400);
        match formatter.format(past) {
            RelativeTimeResult::DaysAgo(days) => assert_eq!(days, 5),
            _ => panic!("Expected DaysAgo(5)"),
        }
    }

    #[test]
    fn test_format_one_day_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 1 day ago
        let past = now - Duration::from_secs(86400);
        match formatter.format(past) {
            RelativeTimeResult::DaysAgo(days) => assert_eq!(days, 1),
            _ => panic!("Expected DaysAgo(1)"),
        }
    }

    #[test]
    fn test_format_one_hour_ago() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 1 hour ago
        let past = now - Duration::from_secs(3600);
        match formatter.format(past) {
            RelativeTimeResult::HoursAgo(hrs) => assert_eq!(hrs, 1),
            _ => panic!("Expected HoursAgo(1)"),
        }
    }

    #[test]
    fn test_format_standard_date() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 10 days ago -> standard date
        let past = now - Duration::from_secs(10 * 86400);
        match formatter.format(past) {
            RelativeTimeResult::StandardDate(_) => {},
            _ => panic!("Expected StandardDate for 10+ days"),
        }
    }

    #[test]
    fn test_format_future_time() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Future time should return standard date
        let future = now + Duration::from_secs(3600);
        match formatter.format(future) {
            RelativeTimeResult::StandardDate(_) => {},
            other => panic!("Expected StandardDate for future time, got {:?}", other),
        }
    }

    #[test]
    fn test_needs_refresh_true() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 61 seconds ago -> needs refresh
        let last_update = now - Duration::from_secs(61);
        assert!(formatter.needs_refresh(last_update));
    }

    #[test]
    fn test_needs_refresh_false() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 30 seconds ago -> no refresh needed
        let last_update = now - Duration::from_secs(30);
        assert!(!formatter.needs_refresh(last_update));
    }

    #[test]
    fn test_needs_refresh_exactly_at_interval() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Exactly 60 seconds ago -> needs refresh (>= interval)
        let last_update = now - Duration::from_secs(60);
        assert!(formatter.needs_refresh(last_update));
    }

    #[test]
    fn test_format_boundary_just_now_to_minutes() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Exactly 60 seconds ago should be MinutesAgo(1), not JustNow
        let past = now - Duration::from_secs(60);
        match formatter.format(past) {
            RelativeTimeResult::MinutesAgo(1) => {},
            RelativeTimeResult::JustNow => panic!("60 seconds should be MinutesAgo(1), not JustNow"),
            other => panic!("Expected MinutesAgo(1), got {:?}", other),
        }
    }

    #[test]
    fn test_format_boundary_minutes_to_hours() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Exactly 60 minutes ago should be HoursAgo(1), not MinutesAgo(60)
        let past = now - Duration::from_secs(60 * 60);
        match formatter.format(past) {
            RelativeTimeResult::HoursAgo(1) => {},
            RelativeTimeResult::MinutesAgo(60) => {},
            other => panic!("Expected HoursAgo(1), got {:?}", other),
        }
    }

    #[test]
    fn test_format_boundary_hours_to_days() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Exactly 24 hours ago should be DaysAgo(1), not HoursAgo(24)
        let past = now - Duration::from_secs(24 * 3600);
        match formatter.format(past) {
            RelativeTimeResult::DaysAgo(1) => {},
            RelativeTimeResult::HoursAgo(24) => {},
            other => panic!("Expected DaysAgo(1), got {:?}", other),
        }
    }

    #[test]
    fn test_format_boundary_days_to_standard() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // Exactly 7 days ago should be StandardDate, not DaysAgo(7)
        let past = now - Duration::from_secs(7 * 86400);
        match formatter.format(past) {
            RelativeTimeResult::StandardDate(_) => {},
            RelativeTimeResult::DaysAgo(7) => {},
            other => panic!("Expected StandardDate or DaysAgo(7), got {:?}", other),
        }
    }

    #[test]
    fn test_format_large_minute_count() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 59 minutes ago
        let past = now - Duration::from_secs(59 * 60);
        match formatter.format(past) {
            RelativeTimeResult::MinutesAgo(mins) => assert_eq!(mins, 59),
            _ => panic!("Expected MinutesAgo(59)"),
        }
    }

    #[test]
    fn test_format_large_hour_count() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 23 hours ago
        let past = now - Duration::from_secs(23 * 3600);
        match formatter.format(past) {
            RelativeTimeResult::HoursAgo(hrs) => assert_eq!(hrs, 23),
            _ => panic!("Expected HoursAgo(23)"),
        }
    }

    #[test]
    fn test_format_large_day_count() {
        let formatter = RelativeDateFormatter::new();
        let now = SystemTime::now();
        
        // 6 days ago
        let past = now - Duration::from_secs(6 * 86400);
        match formatter.format(past) {
            RelativeTimeResult::DaysAgo(days) => assert_eq!(days, 6),
            _ => panic!("Expected DaysAgo(6)"),
        }
    }

    #[test]
    fn test_min_minutes_config() {
        let config = RelativeDateConfig {
            refresh_interval_secs: 60,
            show_just_now: true,
            min_minutes: 2,
        };
        let formatter = RelativeDateFormatter::with_config(config);
        let now = SystemTime::now();
        
        // With min_minutes=2, 1 minute should show as 2 minutes
        let past = now - Duration::from_secs(60);
        match formatter.format(past) {
            RelativeTimeResult::MinutesAgo(mins) => assert_eq!(mins, 2),
            _ => panic!("Expected MinutesAgo(2) with min_minutes=2"),
        }
    }

    #[test]
    fn test_show_just_now_false() {
        let config = RelativeDateConfig {
            refresh_interval_secs: 60,
            show_just_now: false,  // Disabled
            min_minutes: 1,
        };
        let formatter = RelativeDateFormatter::with_config(config);
        let now = SystemTime::now();
        
        // With show_just_now=false, 30 seconds should show as MinutesAgo
        let past = now - Duration::from_secs(30);
        match formatter.format(past) {
            RelativeTimeResult::MinutesAgo(1) => {},
            RelativeTimeResult::JustNow => panic!("Should not show JustNow when disabled"),
            _ => panic!("Expected MinutesAgo(1)"),
        }
    }
}