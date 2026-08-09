//! `northstar-cli` — the minimum useful `.nspkg` inspection workflow:
//! `classify`, `inspect`, `pack-test`, `unpack-test`.
//!
//! This is developer tooling over `northstar-core` only (deliberately no
//! Bevy dependency — see the workspace `AGENTS.md`). The pack/unpack
//! commands only prove lossless opaque-chunk round-tripping; they are not
//! the eventual mod-authoring interface.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use northstar_core::{
    ChunkDescriptor, ChunkId, ClassifiedFilename, ContainerReader, ContainerWriter,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };
    let rest: Vec<String> = args.collect();

    let result = match command.as_str() {
        "classify" => run_classify(rest),
        "inspect" => run_inspect(rest),
        "pack-test" => run_pack_test(rest),
        "unpack-test" => run_unpack_test(rest),
        "help" | "-h" | "--help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "unknown command \"{other}\"; see `northstar-cli help`"
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
        "northstar-cli — minimal .nspkg developer tooling\n\
         \n\
         USAGE:\n\
         \x20   classify <path>                        classify by filename only, no file I/O\n\
         \x20   inspect <path>                          print an .nspkg container's header and chunk index\n\
         \x20   pack-test <output-path>                 write a small deterministic test container\n\
         \x20   unpack-test <input-path> <output-dir>    extract every chunk's raw bytes to files"
    );
}

fn one_arg(args: Vec<String>, usage: &str) -> Result<String, String> {
    <[String; 1]>::try_from(args)
        .map(|[a]| a)
        .map_err(|_| usage.to_string())
}

fn two_args(args: Vec<String>, usage: &str) -> Result<(String, String), String> {
    <[String; 2]>::try_from(args)
        .map(|[a, b]| (a, b))
        .map_err(|_| usage.to_string())
}

/// `classify <path>` — filename-only, performs no file I/O, need not exist.
fn run_classify(args: Vec<String>) -> Result<(), String> {
    let path = one_arg(args, "usage: classify <path>")?;

    match ClassifiedFilename::classify_path(Path::new(&path)) {
        Ok(ClassifiedFilename::Package { puid }) => {
            println!("{path}: complete package (puid = {puid})");
        }
        Ok(ClassifiedFilename::Asset { puid, category }) => {
            println!("{path}: asset (puid = {puid}, category = {category})");
        }
        Err(e) => return Err(format!("{path}: {e}")),
    }
    Ok(())
}

/// `inspect <path>` — reads the file and prints the experimental
/// container's header and chunk index.
fn run_inspect(args: Vec<String>) -> Result<(), String> {
    let path = one_arg(args, "usage: inspect <path>")?;

    let bytes = fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    let container = ContainerReader::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;

    println!("{path}");
    println!("  format version: {}", container.format_version());
    match container.self_identity() {
        ClassifiedFilename::Package { puid } => {
            println!("  self-identity: complete package (puid = {puid})");
        }
        ClassifiedFilename::Asset { puid, category } => {
            println!("  self-identity: asset (puid = {puid}, category = {category})");
        }
    }
    println!("  metadata: {} bytes", container.metadata().len());

    let mut descriptors: Vec<_> = container.chunk_descriptors().collect();
    descriptors.sort_by_key(|d| d.id.0);
    println!("  chunks: {}", descriptors.len());
    for d in descriptors {
        let size = container.chunk_bytes(d.id).map(<[u8]>::len).unwrap_or(0);
        println!(
            "    #{:<4} kind={:<16} compression={:?} size={size}",
            d.id.0, d.kind, d.compression
        );
    }
    Ok(())
}

/// `pack-test <output-path>` — writes a small, deterministic two-chunk
/// container, self-identified by classifying `output-path`'s own filename.
fn run_pack_test(args: Vec<String>) -> Result<(), String> {
    let output_path = one_arg(args, "usage: pack-test <output-path>")?;

    let identity = ClassifiedFilename::classify_path(Path::new(&output_path))
        .map_err(|e| format!("{output_path}: {e}"))?;

    let mut writer =
        ContainerWriter::new(identity).with_metadata(*b"northstar-cli pack-test fixture");
    writer
        .push_chunk(
            ChunkDescriptor::new(ChunkId(0), "greeting"),
            b"hello, nspkg!".to_vec(),
        )
        .map_err(|e| e.to_string())?;
    writer
        .push_chunk(
            ChunkDescriptor::new(ChunkId(1), "numbers"),
            (0u8..32).collect::<Vec<u8>>(),
        )
        .map_err(|e| e.to_string())?;

    let bytes = writer.encode();
    if let Some(parent) = Path::new(&output_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|e| format!("{output_path}: {e}"))?;
    }
    fs::write(&output_path, &bytes).map_err(|e| format!("{output_path}: {e}"))?;

    println!("wrote {output_path} ({} bytes, 2 chunks)", bytes.len());
    Ok(())
}

/// `unpack-test <input-path> <output-dir>` — extracts every chunk's raw
/// bytes (and the metadata payload) to files, proving lossless round-trip.
fn run_unpack_test(args: Vec<String>) -> Result<(), String> {
    let (input_path, output_dir) =
        two_args(args, "usage: unpack-test <input-path> <output-directory>")?;

    let bytes = fs::read(&input_path).map_err(|e| format!("{input_path}: {e}"))?;
    let container = ContainerReader::parse(&bytes).map_err(|e| format!("{input_path}: {e}"))?;

    fs::create_dir_all(&output_dir).map_err(|e| format!("{output_dir}: {e}"))?;
    fs::write(
        PathBuf::from(&output_dir).join("metadata.bin"),
        container.metadata(),
    )
    .map_err(|e| format!("{output_dir}: {e}"))?;

    let mut count = 0usize;
    for descriptor in container.chunk_descriptors() {
        let chunk_bytes = container
            .chunk_bytes(descriptor.id)
            .map_err(|e| format!("{input_path}: chunk #{}: {e}", descriptor.id.0))?;
        let file_name = format!("chunk-{:04}.{}.bin", descriptor.id.0, descriptor.kind);
        fs::write(PathBuf::from(&output_dir).join(file_name), chunk_bytes)
            .map_err(|e| format!("{output_dir}: {e}"))?;
        count += 1;
    }

    println!("unpacked {count} chunk(s) and metadata.bin into {output_dir}");
    Ok(())
}
