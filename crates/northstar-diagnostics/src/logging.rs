use tracing_subscriber::EnvFilter;

/// Install the global `tracing` subscriber: `RUST_LOG`-driven filtering
/// (default `info` if unset), targets shown so [`crate::targets`]
/// categories are visible in output.
///
/// Safe to call more than once (e.g. once from a binary's `main` and again
/// from a test that also happens to construct one) — a subscriber can only
/// be installed globally once; later calls are silently ignored rather than
/// panicking.
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}
