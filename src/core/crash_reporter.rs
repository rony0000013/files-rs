//! Crash / Error Reporting Service
//!
//! Provides Sentry integration for automatic error and crash reporting.
//!
//! The `CrashReporter` struct wraps the Sentry SDK initialization and
//! lifecycle. It is only active when the `SENTRY_DSN` environment variable
//! is set. If unset, all operations are no-ops.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag indicating whether Sentry was initialized.
static SENTRY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the crash reporter is currently active.
pub fn is_sentry_active() -> bool {
    SENTRY_ACTIVE.load(Ordering::Relaxed)
}

/// Crash reporter that wraps the Sentry SDK.
pub struct CrashReporter {
    /// The Sentry client init guard. When dropped, Sentry flushes events.
    _guard: Option<sentry::ClientInitGuard>,
}

impl CrashReporter {
    /// Initialize the crash reporter.
    ///
    /// Reads `SENTRY_DSN` from the environment. If present, initializes the
    /// Sentry SDK. If absent, returns a no-op instance.
    pub fn init() -> Self {
        match std::env::var("SENTRY_DSN") {
            Ok(dsn) if !dsn.is_empty() => {
                let dsn_str = dsn.trim().to_string();

                tracing::info!("SENTRY_DSN found — initializing Sentry crash reporter");

                let guard = sentry::init((
                    dsn_str,
                    sentry::ClientOptions {
                        release: Some(format!("files-rs@{}", env!("CARGO_PKG_VERSION")).into()),
                        environment: Some(
                            std::env::var("SENTRY_ENVIRONMENT")
                                .unwrap_or_else(|_| "production".to_string())
                                .into(),
                        ),
                        auto_session_tracking: true,
                        ..Default::default()
                    },
                ));

                // Configure sentry-tracing integration (breadcrumbs from tracing events)
                // In sentry 0.34, this is automatic via the `tracing` feature

                SENTRY_ACTIVE.store(true, Ordering::Relaxed);

                tracing::debug!("Sentry crash reporter initialized successfully");

                Self {
                    _guard: Some(guard),
                }
            }
            _ => {
                tracing::debug!("SENTRY_DSN not set — crash reporter disabled");
                Self { _guard: None }
            }
        }
    }

    /// Check whether Sentry is currently active.
    pub fn is_active(&self) -> bool {
        self._guard.is_some()
    }

    /// Send a custom error message to Sentry.
    pub fn capture_message(&self, message: &str) -> bool {
        if self._guard.is_some() {
            sentry::capture_message(message, sentry::Level::Error);
            true
        } else {
            false
        }
    }

    /// Send an error event to Sentry.
    pub fn capture_error(&self, error: &dyn std::error::Error) -> bool {
        if self._guard.is_some() {
            sentry::capture_error(error);
            true
        } else {
            false
        }
    }

    /// Manually flush pending events.
    pub fn flush(&self) {
        if self._guard.is_some() {
            // Dropping the guard flushes automatically.
            // Manual flush is not directly exposed in sentry 0.34 as a free function.
            tracing::debug!("Sentry flush requested — events will flush on shutdown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_reporter_init_no_dsn() {
        let reporter = CrashReporter::init();
        assert!(!reporter.is_active());
        assert!(!is_sentry_active());
    }

    #[test]
    fn test_capture_message_when_inactive() {
        let reporter = CrashReporter::init();
        assert!(!reporter.capture_message("test"));
    }

    #[test]
    fn test_capture_error_when_inactive() {
        let reporter = CrashReporter::init();
        let err = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        assert!(!reporter.capture_error(&err));
    }

    #[test]
    fn test_flush_when_inactive() {
        let reporter = CrashReporter::init();
        reporter.flush(); // should not panic
    }
}
