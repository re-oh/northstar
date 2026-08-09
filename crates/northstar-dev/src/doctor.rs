//! `northstar-dev doctor` — reports toolchain, project configuration, and
//! asset-directory health. Informational only: it never fails the process,
//! since "something's wrong" is exactly the case it exists to describe
//! clearly rather than short-circuit on.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(_args: Vec<String>) -> Result<(), String> {
    println!("northstar-dev doctor\n");

    check_toolchain();
    println!();
    let workspace_root = check_project_configuration();
    println!();
    check_asset_directory(workspace_root.as_deref());

    Ok(())
}

fn status(ok: bool, label: &str, detail: impl AsRef<str>) {
    let marker = if ok { "[ok]  " } else { "[warn]" };
    println!("{marker} {label}: {}", detail.as_ref());
}

fn check_toolchain() {
    println!("Toolchain:");
    match command_output("rustc", &["--version"]) {
        Some(v) => status(true, "rustc", v),
        None => status(false, "rustc", "not found on PATH"),
    }
    match command_output("cargo", &["--version"]) {
        Some(v) => status(true, "cargo", v),
        None => status(false, "cargo", "not found on PATH"),
    }
    match command_output("cargo", &["clippy", "--version"]) {
        Some(v) => status(true, "clippy", v),
        None => status(
            false,
            "clippy",
            "not installed (rustup component add clippy)",
        ),
    }
    match command_output("cargo", &["fmt", "--version"]) {
        Some(v) => status(true, "rustfmt", v),
        None => status(
            false,
            "rustfmt",
            "not installed (rustup component add rustfmt)",
        ),
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Walk up from the current directory looking for the workspace root
/// (identified by `rust-toolchain.toml` sitting next to a workspace
/// `Cargo.toml`). Returns it if found.
fn find_workspace_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        if dir.join("rust-toolchain.toml").is_file() && dir.join("Cargo.toml").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn check_project_configuration() -> Option<PathBuf> {
    println!("Project configuration:");
    let root = find_workspace_root();
    match &root {
        Some(root) => status(true, "workspace root", root.display().to_string()),
        None => status(
            false,
            "workspace root",
            "not found (no rust-toolchain.toml + Cargo.toml above the current directory)",
        ),
    }

    if let Some(root) = &root {
        status(
            root.join("AGENTS.md").is_file(),
            "AGENTS.md",
            "repository conventions doc",
        );
        status(
            root.join(".github/workflows/ci.yml").is_file(),
            "CI workflow",
            ".github/workflows/ci.yml",
        );
    }

    root
}

fn check_asset_directory(workspace_root: Option<&Path>) {
    println!("Asset directory:");
    let Some(root) = workspace_root else {
        status(
            false,
            "assets/packages",
            "skipped (no workspace root found)",
        );
        return;
    };

    let packages_dir = root.join("crates/northstar-game/assets/packages");
    if !packages_dir.is_dir() {
        status(
            false,
            "assets/packages",
            format!("missing: {}", packages_dir.display()),
        );
        return;
    }

    let entries = std::fs::read_dir(&packages_dir)
        .map(|rd| rd.filter_map(Result::ok).count())
        .unwrap_or(0);
    status(
        true,
        "assets/packages",
        format!(
            "{} (mounted package dir{})",
            packages_dir.display(),
            if entries == 1 { "" } else { "s" }
        ),
    );
    status(entries > 0, "mounted packages", entries.to_string());
}
