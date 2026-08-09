//! Captures the git commit this build was made from, if any, so
//! [`crate::build_info`] can report it without a runtime git dependency.

use std::process::Command;

fn main() {
    let git_sha = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(false);

    println!("cargo:rustc-env=NORTHSTAR_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=NORTHSTAR_GIT_DIRTY={dirty}");
    // Re-run if HEAD moves, but don't otherwise force a rebuild every time.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
