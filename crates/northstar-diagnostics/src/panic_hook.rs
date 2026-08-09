use std::panic;

use crate::build_info::BuildInfo;

/// Install a panic hook that reports panics as a single structured
/// `tracing::error!` line (build info + location + message) instead of the
/// default raw multi-line dump, then falls through to the default hook so
/// `RUST_BACKTRACE=1` still prints a backtrace beneath it.
///
/// Call this once, as early as possible — before [`crate::init_logging`] if
/// you want pre-logging-setup panics covered too (the fallback default hook
/// still handles those; this hook itself only formats nicely once tracing
/// is live).
pub fn install() {
    let default_hook = panic::take_hook();
    let build_info = BuildInfo::current();

    panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_owned());

        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_owned());

        tracing::error!(
            target: "northstar::panic",
            %location,
            %message,
            %build_info,
            "northstar panicked — set RUST_BACKTRACE=1 for a backtrace",
        );

        default_hook(info);
    }));
}
