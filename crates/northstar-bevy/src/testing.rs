//! Two intentionally trivial `.nspkg` asset types, used to prove dispatch
//! and cross-asset dependency loading rather than to model any real
//! Northstar content. See the integration tests in `tests/` for how these
//! are registered and loaded end-to-end.
//!
//! Not gated behind a Cargo feature: it is a handful of tiny types, and
//! keeping it always-available makes it usable from both the crate's own
//! `tests/` binaries and, if useful later, from `northstar-cli` examples.

use bevy::asset::{Asset, Handle};
use bevy::reflect::TypePath;
use thiserror::Error;

use northstar_core::{AssetPuid, ContainerReader};

use crate::load_context::{NorthstarLoadContext, NorthstarReferenceError};
use crate::loader::NspkgDecoder;

/// A leaf asset with no dependencies: its entire payload is one UTF-8
/// string, stored as the container's metadata bytes.
#[derive(Asset, TypePath, Debug, Clone, PartialEq, Eq)]
pub struct TestLeafAsset {
    pub value: String,
}

/// A parent asset that references one [`TestLeafAsset`] in the *same*
/// package, proving `NorthstarLoadContext::load_local` dependency
/// resolution.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct TestParentAsset {
    pub label: String,
    pub leaf: Handle<TestLeafAsset>,
}

#[derive(Debug, Error)]
pub enum TestLeafDecodeError {
    #[error("test_leaf container is missing valid UTF-8 metadata")]
    InvalidMetadata,
}

pub struct TestLeafDecoder;

impl NspkgDecoder for TestLeafDecoder {
    type Asset = TestLeafAsset;
    type Error = TestLeafDecodeError;

    fn decode(
        &self,
        container: &ContainerReader<'_>,
        _puid: &AssetPuid,
        _context: &mut NorthstarLoadContext<'_, '_>,
    ) -> Result<Self::Asset, Self::Error> {
        let value = std::str::from_utf8(container.metadata())
            .map_err(|_| TestLeafDecodeError::InvalidMetadata)?
            .to_owned();
        Ok(TestLeafAsset { value })
    }
}

#[derive(Debug, Error)]
pub enum TestParentDecodeError {
    #[error("test_parent container is missing valid UTF-8 metadata")]
    InvalidMetadata,
    #[error("test_parent could not reference its leaf: {0}")]
    Reference(#[from] NorthstarReferenceError),
}

pub struct TestParentDecoder;

impl NspkgDecoder for TestParentDecoder {
    type Asset = TestParentAsset;
    type Error = TestParentDecodeError;

    fn decode(
        &self,
        container: &ContainerReader<'_>,
        _puid: &AssetPuid,
        context: &mut NorthstarLoadContext<'_, '_>,
    ) -> Result<Self::Asset, Self::Error> {
        // Metadata is "<label>\n<leaf puid>" — trivial on purpose.
        let text = std::str::from_utf8(container.metadata())
            .map_err(|_| TestParentDecodeError::InvalidMetadata)?;
        let (label, leaf_puid) = text
            .split_once('\n')
            .ok_or(TestParentDecodeError::InvalidMetadata)?;

        let leaf = context.load_local::<TestLeafAsset>(leaf_puid)?;

        Ok(TestParentAsset {
            label: label.to_owned(),
            leaf,
        })
    }
}
