use std::fmt;

/// Version and build provenance, captured at compile time. See `build.rs`
/// for how the git fields are populated.
#[derive(Debug, Clone, Copy)]
pub struct BuildInfo {
    /// The `northstar-game` (or whichever binary embeds this) crate
    /// version, from `CARGO_PKG_VERSION`.
    pub version: &'static str,
    /// Short git commit SHA this build was made from, or `"unknown"` if
    /// `git` wasn't available or this isn't a git checkout (e.g. a
    /// packaged source tarball).
    pub git_sha: &'static str,
    /// Whether the working tree had uncommitted changes at build time.
    pub git_dirty: bool,
    /// The Cargo profile this was built with (`"debug"` or `"release"`).
    pub profile: &'static str,
}

impl BuildInfo {
    pub const fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            git_sha: env!("NORTHSTAR_GIT_SHA"),
            git_dirty: matches!(env!("NORTHSTAR_GIT_DIRTY").as_bytes(), b"true"),
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        }
    }
}

impl fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "northstar {} ({}{}, {})",
            self.version,
            self.git_sha,
            if self.git_dirty { "-dirty" } else { "" },
            self.profile
        )
    }
}
