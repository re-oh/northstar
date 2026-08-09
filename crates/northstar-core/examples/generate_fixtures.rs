//! Regenerates `tests/fixtures/`. Fixtures are checked into the repo (not
//! generated at test time) so they're inspectable, stable across runs, and
//! usable as a fuzz-target seed corpus. Run after changing the fixture set:
//!
//! ```sh
//! cargo run --example generate_fixtures -p northstar-core
//! ```

use std::fs;
use std::path::Path;

use northstar_core::{
    AssetCategory, AssetPuid, ChunkDescriptor, ChunkId, ClassifiedFilename, ContainerWriter,
    PackageId,
};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let valid = root.join("valid");
    let corrupted = root.join("corrupted");
    fs::create_dir_all(&valid).unwrap();
    fs::create_dir_all(&corrupted).unwrap();

    write_valid_fixtures(&valid);
    write_corrupted_fixtures(&corrupted);

    println!("wrote fixtures to {}", root.display());
}

fn write_valid_fixtures(dir: &Path) {
    // A complete package with no chunks — the simplest valid container.
    let empty_package = ContainerWriter::new(ClassifiedFilename::Package {
        puid: PackageId::new("sample_basegame").unwrap(),
    })
    .with_metadata(*b"deterministic fixture, no chunks");
    fs::write(dir.join("sample_basegame.nspkg"), empty_package.encode()).unwrap();

    // An asset with two small, deterministic chunks.
    let mut map_asset = ContainerWriter::new(ClassifiedFilename::Asset {
        puid: AssetPuid::new("sample_island").unwrap(),
        category: AssetCategory::new("map").unwrap(),
    })
    .with_metadata(*b"deterministic fixture metadata");
    map_asset
        .push_chunk(
            ChunkDescriptor::new(ChunkId(0), "greeting"),
            b"hello, nspkg fixture!".to_vec(),
        )
        .unwrap();
    map_asset
        .push_chunk(
            ChunkDescriptor::new(ChunkId(1), "sequence"),
            (0u8..64).collect::<Vec<u8>>(),
        )
        .unwrap();
    fs::write(dir.join("sample_island.map.nspkg"), map_asset.encode()).unwrap();
}

fn write_corrupted_fixtures(dir: &Path) {
    // Start from a real, valid encoding, then corrupt it in one specific,
    // documented way per fixture — each one exercises a distinct
    // `ContainerError` variant, not just "any old garbage".
    let mut base = ContainerWriter::new(ClassifiedFilename::Asset {
        puid: AssetPuid::new("sample_island").unwrap(),
        category: AssetCategory::new("map").unwrap(),
    })
    .with_metadata(*b"deterministic fixture metadata");
    base.push_chunk(
        ChunkDescriptor::new(ChunkId(0), "greeting"),
        b"hello, nspkg fixture!".to_vec(),
    )
    .unwrap();
    let valid_bytes = base.encode();

    // Truncated at various points, including mid-header and mid-chunk-data.
    for &len in &[0usize, 4, 8, 10, 40, valid_bytes.len() - 1] {
        fs::write(
            dir.join(format!("truncated_at_{len}.bin")),
            &valid_bytes[..len.min(valid_bytes.len())],
        )
        .unwrap();
    }

    // Bad magic: corrupt the first 4 bytes.
    let mut bad_magic = valid_bytes.clone();
    bad_magic[0..4].copy_from_slice(b"NOPE");
    fs::write(dir.join("bad_magic.bin"), bad_magic).unwrap();

    // Corrupted chunk payload (checksum mismatch): flip the last byte,
    // which — for this fixture — falls inside the chunk data.
    let mut bad_checksum = valid_bytes.clone();
    let last = bad_checksum.len() - 1;
    bad_checksum[last] ^= 0xff;
    fs::write(dir.join("bad_checksum.bin"), bad_checksum).unwrap();

    // Unsupported future format version.
    let mut bad_version = valid_bytes.clone();
    bad_version[8] = 0xff;
    bad_version[9] = 0xff;
    fs::write(dir.join("unsupported_version.bin"), bad_version).unwrap();
}
