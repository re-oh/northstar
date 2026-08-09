//! `northstar-dev validate` — repository/content validation. Stub. Intended
//! eventual scope: recursively classify+inspect every `.nspkg` under a
//! directory and report malformed files (see
//! `crates/northstar-core/tests/fixtures` for the corrupted/truncated
//! fixtures this would be tested against), plus whatever repo-level checks
//! (doc cross-links, `AGENTS.md` freshness, ...) turn out to be worth
//! automating once there's more than one contributor relying on them.

pub fn run(_args: Vec<String>) -> Result<(), String> {
    println!(
        "northstar-dev validate: not yet implemented.\n\
         \n\
         Planned: recursively validate every .nspkg under a directory\n\
         (classify + container parse, reporting malformed files) and\n\
         eventually repo-level checks. For now, `northstar-dev assets\n\
         inspect <path>` validates one file."
    );
    Ok(())
}
