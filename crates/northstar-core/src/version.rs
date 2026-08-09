use std::fmt;

/// A container format/schema version.
///
/// Every reader and writer path in this crate is explicit about which
/// version it speaks rather than assuming "the current format" — see
/// `docs/architecture.md` for why the experimental layout must not be
/// treated as a compatibility promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormatVersion(pub u16);

impl FormatVersion {
    /// The experimental version-zero container layout implemented by this
    /// crate. Not a stability promise — see `docs/architecture.md`.
    pub const EXPERIMENTAL_V0: Self = Self(0);
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}
