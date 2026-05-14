//! Performance & Observability module.
//!
//! Provides tracing span helpers, parallel metadata loading,
//! memory-mapped file previews, cancellable operations, and
//! directory enumeration benchmarks.

pub mod benchmarks;
pub mod cancellable_load;
pub mod mmap_preview;
pub mod parallel_batch;
pub mod tracing_spans;

pub use benchmarks::*;
pub use cancellable_load::*;
pub use mmap_preview::*;
pub use parallel_batch::*;
pub use tracing_spans::*;
