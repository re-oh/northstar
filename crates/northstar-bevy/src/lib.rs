//! Bevy integration for Northstar's asset layer.
//!
//! Northstar is an addressing, package-mounting, and container-decoding
//! layer beneath Bevy's normal typed asset system — it does not replace
//! Bevy's runtime asset storage with a generic blob store. This crate:
//!
//! - registers a custom `northstar://` [`bevy::asset::io::AssetSource`] via
//!   [`NorthstarAssetPlugin`], backed by a [`PackageCatalog`];
//! - classifies `.nspkg` paths by filename before decoding, via
//!   `northstar-core`;
//! - routes each registered category through an engine-owned
//!   [`NspkgDecoder`], never through content-provided code — see
//!   [`NspkgAssetApp::register_nspkg_asset`];
//! - lets gameplay code load a package-qualified, typed [`AssetRef<T>`]
//!   through [`NorthstarAssets`] and get back an ordinary `Handle<T>`.
//!
//! See `docs/architecture.md` for the design rationale, including why the
//! category registry ([`registry`]) is a process-global table rather than
//! an ECS resource.

mod asset_ref;
mod catalog;
mod load_context;
mod loader;
mod path;
mod plugin;
mod registry;

pub mod testing;

pub use asset_ref::{AssetRef, NorthstarAssets};
pub use catalog::PackageCatalog;
pub use load_context::{NorthstarLoadContext, NorthstarReferenceError};
pub use loader::{NspkgDecoder, NspkgLoadError, NspkgLoader};
pub use plugin::{NorthstarAssetPlugin, SOURCE_NAME};
pub use registry::NspkgAssetApp;
