//! Northstar core asset model.
//!
//! This crate is intentionally dependency-light and has no knowledge of
//! Bevy. It defines:
//!
//! - durable package identity ([`PackageId`]) and package-local asset
//!   identity ([`AssetPuid`], [`AssetKey`]);
//! - open-ended asset classification ([`AssetCategory`]);
//! - the `.nspkg` filename grammar and its classifier ([`filename`]);
//! - the experimental chunked container codec ([`container`]).
//!
//! See `docs/architecture.md` in the repository root for the design
//! rationale and the decisions this crate deliberately defers.

mod ident;

pub mod category;
pub mod chunk;
pub mod container;
pub mod filename;
pub mod key;
pub mod package_id;
pub mod puid;
pub mod version;

pub use category::{AssetCategory, AssetCategoryError};
pub use chunk::{ChunkCompression, ChunkDescriptor, ChunkId};
pub use container::{ContainerError, ContainerReader, ContainerWriter};
pub use filename::{ClassifiedFilename, ClassifyError};
pub use key::AssetKey;
pub use package_id::{PackageId, PackageIdError};
pub use puid::{AssetPuid, AssetPuidError};
pub use version::FormatVersion;
