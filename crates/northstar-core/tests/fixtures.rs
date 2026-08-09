//! Exercises the checked-in fixtures under `tests/fixtures/` — regenerate
//! them with `cargo run --example generate_fixtures -p northstar-core` if
//! the container format changes.

use std::fs;
use std::path::Path;

use northstar_core::{ClassifiedFilename, ContainerError, ContainerReader};

#[test]
fn valid_fixtures_parse_and_round_trip() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();

        let container = ContainerReader::parse(&bytes)
            .unwrap_or_else(|e| panic!("{}: expected valid, got {e}", path.display()));

        // The fixture's filename and its container-internal self-identity
        // must agree — this is the same check NspkgLoader performs.
        let classified = ClassifiedFilename::classify_path(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            &classified,
            container.self_identity(),
            "{}: filename/container identity mismatch",
            path.display()
        );
    }
}

#[test]
fn corrupted_fixtures_fail_without_panicking() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corrupted");

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();

        let result = ContainerReader::parse(&bytes);
        assert!(
            result.is_err(),
            "{}: expected this corrupted fixture to fail parsing",
            path.display()
        );
    }
}

#[test]
fn specific_corruption_fixtures_map_to_the_expected_error() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corrupted");
    let read = |name: &str| fs::read(dir.join(name)).unwrap();

    assert!(matches!(
        ContainerReader::parse(&read("bad_magic.bin")),
        Err(ContainerError::BadMagic)
    ));
    assert!(matches!(
        ContainerReader::parse(&read("unsupported_version.bin")),
        Err(ContainerError::UnsupportedVersion(_))
    ));
    assert!(matches!(
        ContainerReader::parse(&read("bad_checksum.bin")),
        Err(ContainerError::ChunkIntegrityMismatch(_))
    ));
    for truncated in [
        "truncated_at_0.bin",
        "truncated_at_4.bin",
        "truncated_at_8.bin",
    ] {
        assert!(matches!(
            ContainerReader::parse(&read(truncated)),
            Err(ContainerError::Truncated)
        ));
    }
}
