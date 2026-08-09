use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// The identity of one open [`crate::View`] instance.
///
/// Distinct from [`ViewKind`](crate::ViewKind): a `ViewId` identifies *this
/// particular open view* (so the workspace can e.g. close exactly one of
/// three open map-editor tabs); `ViewKind` identifies *what kind of view it
/// is*. Two views of the same kind never share a `ViewId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(u64);

impl ViewId {
    /// Allocates a new, process-unique `ViewId`. There is no meaning to
    /// the underlying value beyond uniqueness — do not persist it across
    /// process runs (that's what serialized view *state*, not `ViewId`, is
    /// for; see `docs/editor-views.md`).
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ViewId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "view#{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = ViewId::new();
        let b = ViewId::new();
        assert_ne!(a, b);
    }
}
