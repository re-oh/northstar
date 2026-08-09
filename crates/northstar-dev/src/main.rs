//! `northstar-dev` — the Northstar developer CLI.
//!
//! ```text
//! northstar-dev doctor
//! northstar-dev packages ...
//! northstar-dev assets ...
//! northstar-dev validate ...
//! ```
//!
//! `assets` absorbs the `.nspkg` tooling (`classify`/`inspect`/`pack-test`/
//! `unpack-test`) that used to be this crate's entire surface, back when it
//! was `northstar-cli`.

mod assets;
mod doctor;
mod packages;
mod validate;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };
    let rest: Vec<String> = args.collect();

    let result = match command.as_str() {
        "doctor" => doctor::run(rest),
        "packages" => packages::run(rest),
        "assets" => assets::run(rest),
        "validate" => validate::run(rest),
        "help" | "-h" | "--help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "unknown command \"{other}\"; see `northstar-dev help`"
        )),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "northstar-dev — the Northstar developer CLI\n\
         \n\
         USAGE:\n\
         \x20   doctor                  report toolchain, project config, and asset-dir health\n\
         \x20   packages ...            package catalog inspection (not yet implemented)\n\
         \x20   assets ...              .nspkg tooling: classify, inspect, pack-test, unpack-test\n\
         \x20   validate ...            validate .nspkg content (not yet implemented)\n\
         \n\
         Run `northstar-dev assets help` for the assets subcommands."
    );
}

/// Shared by subcommand modules that take exactly one positional argument.
pub(crate) fn one_arg(args: Vec<String>, usage: &str) -> Result<String, String> {
    <[String; 1]>::try_from(args)
        .map(|[a]| a)
        .map_err(|_| usage.to_string())
}

/// Shared by subcommand modules that take exactly two positional arguments.
pub(crate) fn two_args(args: Vec<String>, usage: &str) -> Result<(String, String), String> {
    <[String; 2]>::try_from(args)
        .map(|[a, b]| (a, b))
        .map_err(|_| usage.to_string())
}
