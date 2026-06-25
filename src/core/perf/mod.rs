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

// Explicit re-exports
pub use benchmarks::{bench_jwalk, bench_std_fs, compare_methods, print_comparison, BenchResult};
pub use cancellable_load::{CancelGuard, CancellableOp, CancellationToken, DirectoryLoadHandle};
pub use mmap_preview::MmapPreviewReader;
pub use parallel_batch::{batch_load_items, batch_load_metadata};
pub use tracing_spans::{trace_directory_list, trace_io_span, OperationTimer};
