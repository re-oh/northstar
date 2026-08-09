//! Property tests for filename parsing and identity round-tripping.
//!
//! These complement (don't replace) the example-based tests in
//! `src/filename.rs` and `src/container.rs` — the examples pin down
//! specific documented behaviors, these sweep the input space around them.

use std::collections::HashSet;

use proptest::prelude::*;

use northstar_core::{
    AssetCategory, AssetKey, AssetPuid, ChunkDescriptor, ChunkId, ClassifiedFilename,
    ContainerReader, ContainerWriter, PackageId,
};

/// Any non-empty, filename-safe segment — matches the charset
/// `PackageId`/`AssetPuid`/`AssetCategory` all accept.
fn segment() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,24}"
}

proptest! {
    #[test]
    fn any_valid_asset_filename_round_trips(puid in segment(), category in segment()) {
        let filename = format!("{puid}.{category}.nspkg");
        let classified = ClassifiedFilename::classify(&filename).unwrap();
        prop_assert_eq!(
            classified,
            ClassifiedFilename::Asset {
                puid: AssetPuid::new(puid).unwrap(),
                category: AssetCategory::new(category).unwrap(),
            }
        );
    }

    #[test]
    fn any_valid_package_filename_round_trips(puid in segment()) {
        let filename = format!("{puid}.nspkg");
        let classified = ClassifiedFilename::classify(&filename).unwrap();
        prop_assert_eq!(
            classified,
            ClassifiedFilename::Package { puid: PackageId::new(puid).unwrap() }
        );
    }

    /// Mirrors required test 7 from the asset foundation brief, swept over
    /// arbitrary inputs instead of one fixed example.
    #[test]
    fn identical_puid_in_different_packages_is_never_the_same_key(
        puid in segment(),
        package_a in segment(),
        package_b in segment(),
    ) {
        prop_assume!(package_a != package_b);
        let key_a = AssetKey::new(
            PackageId::new(package_a).unwrap(),
            AssetPuid::new(puid.clone()).unwrap(),
        );
        let key_b = AssetKey::new(
            PackageId::new(package_b).unwrap(),
            AssetPuid::new(puid).unwrap(),
        );
        prop_assert_ne!(key_a, key_b);
    }

    /// Mirrors required test 9 (round-trip multiple arbitrary chunks
    /// exactly), swept over arbitrary metadata/chunk contents instead of
    /// one fixed example.
    #[test]
    fn container_round_trips_arbitrary_metadata_and_chunks(
        metadata in proptest::collection::vec(any::<u8>(), 0..256),
        chunks in proptest::collection::vec(
            (any::<u32>(), proptest::collection::vec(any::<u8>(), 0..256)),
            0..8,
        ),
    ) {
        // The writer rejects duplicate chunk ids by design (see
        // `container::tests::writer_rejects_duplicate_chunk_ids`), so dedup
        // here rather than asserting on writer behavior this test isn't
        // about.
        let mut seen = HashSet::new();
        let chunks: Vec<_> = chunks.into_iter().filter(|(id, _)| seen.insert(*id)).collect();

        let identity = ClassifiedFilename::Package { puid: PackageId::new("proptest_pkg").unwrap() };
        let mut writer = ContainerWriter::new(identity.clone()).with_metadata(metadata.clone());
        for (id, bytes) in &chunks {
            writer
                .push_chunk(ChunkDescriptor::new(ChunkId(*id), "blob"), bytes.clone())
                .unwrap();
        }

        let encoded = writer.encode();
        let reader = ContainerReader::parse(&encoded).unwrap();

        prop_assert_eq!(reader.self_identity(), &identity);
        prop_assert_eq!(reader.metadata(), metadata.as_slice());
        for (id, bytes) in &chunks {
            prop_assert_eq!(reader.chunk_bytes(ChunkId(*id)).unwrap(), bytes.as_slice());
        }
    }
}
