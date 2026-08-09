//! `northstar-dev packages` — package catalog inspection. Stub: there is no
//! package catalog tooling yet beyond what `assets inspect` already does
//! for one file at a time. Intended eventual scope: listing mounted
//! packages, showing a catalog's resolved backing storage (loose dir /
//! Steam / archive — see `docs/assets.md`), and diffing two catalogs.

pub fn run(_args: Vec<String>) -> Result<(), String> {
    println!(
        "northstar-dev packages: not yet implemented.\n\
         \n\
         Planned: list mounted packages in a catalog, show each package's\n\
         resolved backing storage, and diff two catalogs. See docs/assets.md.\n\
         \n\
         For now, `northstar-dev assets inspect <path>` works on one .nspkg\n\
         file at a time."
    );
    Ok(())
}
