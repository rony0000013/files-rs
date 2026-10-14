//! Cancellable directory loading using generation counters.
//!
//! When the user navigates away from a directory mid-load, the previous
//! fetch should be cancelled. Uses a simple generation counter approach:
//! each directory load gets a generation ID. If navigation increments
//! the counter before the load completes, the load aborts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A generation counter for tracking cancellable operations.
/// Each new operation increments the counter. Mid-flight operations
/// check whether their generation is still current.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    current: Arc<AtomicU64>,
    generation: u64,
}

impl CancellationToken {
    /// Create a new token with the given shared counter.
    pub fn new(counter: Arc<AtomicU64>) -> Self {
        let generation = counter.load(Ordering::Relaxed);
        Self {
            current: counter,
            generation,
        }
    }

    /// Check if this operation should cancel (its generation is stale).
    pub fn is_cancelled(&self) -> bool {
        self.current.load(Ordering::Relaxed) != self.generation
    }

    /// Get the generation for this token.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// A shared generation counter that can be incremented to cancel
/// in-flight operations.
#[derive(Debug, Default, Clone)]
pub struct CancelGuard {
    counter: Arc<AtomicU64>,
}

impl CancelGuard {
    /// Create a new cancel guard.
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Issue a new cancellation token for the current generation.
    pub fn token(&self) -> CancellationToken {
        CancellationToken::new(self.counter.clone())
    }

    /// Cancel all in-flight operations by incrementing the generation.
    pub fn cancel(&self) {
        self.counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Cancel all in-flight operations and issue a fresh token for the new generation.
    pub fn reset(&self) -> CancellationToken {
        self.cancel();
        self.token()
    }

    /// Get the current generation number.
    pub fn generation(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }
}

/// A builder for async operations that support cancellation.
pub struct CancellableOp;

impl CancellableOp {
    /// Run a fallible operation, checking cancellation before starting
    /// and after completion.
    pub fn run<T>(
        token: &CancellationToken,
        op: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if token.is_cancelled() {
            return Err("Operation cancelled".to_string());
        }
        let result = op()?;
        if token.is_cancelled() {
            return Err("Operation cancelled (stale)".to_string());
        }
        Ok(result)
    }

    /// Check cancellation and return a sentinel error if cancelled.
    pub fn check(token: &CancellationToken) -> Result<(), String> {
        if token.is_cancelled() {
            Err("Operation cancelled".to_string())
        } else {
            Ok(())
        }
    }
}

/// A generation-tracked async work item for directory loading.
pub struct DirectoryLoadHandle {
    pub path: String,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_guard_initial_generation_zero() {
        let guard = CancelGuard::new();
        assert_eq!(guard.generation(), 0);
    }

    #[test]
    fn test_token_not_cancelled_initially() {
        let guard = CancelGuard::new();
        let token = guard.token();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_token_cancelled_after_cancel() {
        let guard = CancelGuard::new();
        let token = guard.token();
        assert!(!token.is_cancelled());
        guard.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_multiple_tokens_same_generation() {
        let guard = CancelGuard::new();
        let t1 = guard.token();
        let t2 = guard.token();
        assert!(!t1.is_cancelled());
        assert!(!t2.is_cancelled());
        guard.cancel();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }

    #[test]
    fn test_new_token_after_cancel_is_valid() {
        let guard = CancelGuard::new();
        let t1 = guard.token();
        guard.cancel();
        assert!(t1.is_cancelled());

        let t2 = guard.token();
        assert!(!t2.is_cancelled());
    }

    #[test]
    fn test_cancel_guard_send_sync() {
        // CancelGuard is Clone + Send + Sync
        let guard = CancelGuard::new();
        let g2 = guard.clone();
        std::thread::spawn(move || {
            g2.cancel();
        })
        .join()
        .unwrap();
        // The original guard's token should reflect this
    }

    #[test]
    fn test_cancellable_op_runs() {
        let guard = CancelGuard::new();
        let token = guard.token();
        let result = CancellableOp::run(&token, || Ok::<_, String>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_cancellable_op_cancelled_before() {
        let guard = CancelGuard::new();
        let token = guard.token();
        guard.cancel();
        let result = CancellableOp::run(&token, || Ok::<_, String>(42));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cancelled"));
    }

    #[test]
    fn test_cancel_check() {
        let guard = CancelGuard::new();
        let token = guard.token();
        assert!(CancellableOp::check(&token).is_ok());
        guard.cancel();
        assert!(CancellableOp::check(&token).is_err());
    }

    #[test]
    fn test_token_generation_advances() {
        let guard = CancelGuard::new();
        let t1 = guard.token();
        assert_eq!(t1.generation(), 0);
        guard.cancel();
        let t2 = guard.token();
        assert_eq!(t2.generation(), 1);
    }

    #[test]
    fn test_directory_load_handle() {
        let handle = DirectoryLoadHandle {
            path: "/tmp/test".to_string(),
            generation: 42,
        };
        assert_eq!(handle.path, "/tmp/test");
        assert_eq!(handle.generation, 42);
    }

    #[test]
    fn test_rapid_navigation_cancels_all_intermediate_tokens() {
        let guard = CancelGuard::new();
        let tokens: Vec<CancellationToken> = (0..100).map(|_| guard.reset()).collect();
        for token in &tokens[..99] {
            assert!(
                token.is_cancelled(),
                "All superseded navigation tokens must be cancelled"
            );
        }
        assert!(
            !tokens[99].is_cancelled(),
            "Only the latest navigation token must remain active"
        );
    }
}
