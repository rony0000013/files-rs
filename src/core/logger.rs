use std::path::PathBuf;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, Registry};

pub struct Logger {
    _guard: WorkerGuard,
}

impl Logger {
    /// Initializes the global tracing subscriber with both console and file output.
    /// The file output uses daily rotation.
    pub fn init() -> Self {
        // 1. Define the directory for logs
        // In a real app, this would be in the AppData folder via the `directories` crate
        let log_dir = PathBuf::from("logs");

        // 2. Setup rolling file appender (daily rotation)
        let file_appender = tracing_appender::rolling::daily(log_dir, "files-rs.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // 3. Create the registry (the heart of tracing-subscriber)
        let registry = Registry::default();

        // 4. Console layer (stdout)
        let console_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_target(true)
            .with_thread_ids(true);

        // 5. File layer
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false) // Disable ANSI colors in files
            .with_target(true)
            .with_thread_ids(true);

        // 6. Combine layers and set as global default
        registry.with(console_layer).with(file_layer).init();

        info!("Logging initialized. Logs are being written to the 'logs' directory.");

        Self { _guard: guard }
    }
}
