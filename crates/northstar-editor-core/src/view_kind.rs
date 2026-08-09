use std::fmt;

/// What kind of view this is (e.g. `"map_editor"`, `"asset_browser"`).
///
/// Deliberately an open string, not a closed enum — same reasoning as
/// `northstar_core::AssetCategory`: the set of view kinds grows as editor
/// features are added, and this crate (which doesn't know about any
/// specific editor feature) must not be the place that enumerates them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewKind(String);

impl ViewKind {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ViewKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ViewKind {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}
