//! Tracing span helpers for async I/O hot paths.
//! Provides a `traced_task` wrapper that instruments async operations
//! with duration, path, and result.

use std::time::Instant;
use tracing::info_span;

/// Wraps an operation with a tracing span that records duration.
/// Returns a guard that must be held for the duration.
pub fn trace_io_span(operation: &str, path: &str) -> tracing::span::EnteredSpan {
    info_span!("io", op = operation, path = %path).entered()
}

/// Tracing wrapper for directory listing — logs start, duration, and file count.
pub fn trace_directory_list(path: &str) -> tracing::span::EnteredSpan {
    info_span!(
        "list_directory",
        path = %path,
        files = tracing::field::Empty,
        duration_ms = tracing::field::Empty
    )
    .entered()
}

/// Record the result into an active span.
pub fn record_span_result(files_count: usize, duration_ms: u64) {
    tracing::Span::current().record("files", &files_count);
    tracing::Span::current().record("duration_ms", &duration_ms);
}

/// Timer for measuring operation duration.
pub struct OperationTimer {
    start: Instant,
    operation: &'static str,
}

impl OperationTimer {
    pub fn start(operation: &'static str) -> Self {
        Self {
            start: Instant::now(),
            operation,
        }
    }
    pub fn finish(&self, path: &str) {
        let elapsed = self.start.elapsed();
        tracing::debug!(
            op = self.operation,
            path = %path,
            duration_ms = elapsed.as_secs_f64() * 1000.0,
            "Operation completed"
        );
    }
    pub fn finish_with_count(&self, path: &str, count: usize) {
        let elapsed = self.start.elapsed();
        tracing::info!(
            op = self.operation,
            path = %path,
            duration_ms = elapsed.as_secs_f64() * 1000.0,
            count = count,
            "Operation completed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_io_span_creates_span() {
        let _span = trace_io_span("test_op", "/tmp/test");
        // Span should be entered — verify by checking it doesn't panic
    }

    #[test]
    fn test_trace_directory_list_creates_span() {
        let _span = trace_directory_list("/tmp");
    }

    #[test]
    fn test_timer_basic() {
        let timer = OperationTimer::start("test");
        std::thread::sleep(std::time::Duration::from_millis(1));
        timer.finish("/tmp");
    }

    #[test]
    fn test_timer_with_count() {
        let timer = OperationTimer::start("test");
        timer.finish_with_count("/tmp", 42);
    }

    #[test]
    fn test_record_span_result_no_panic() {
        let _span = trace_directory_list("/tmp");
        record_span_result(100, 50);
    }
}
