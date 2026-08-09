use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::ecs::error::BevyError;
use bevy::reflect::TypePath;
use thiserror::Error;

use northstar_core::{
    AssetCategory, AssetPuid, ClassifiedFilename, ClassifyError, ContainerError, ContainerReader,
};

use crate::load_context::NorthstarLoadContext;
use crate::path::package_id_from_load_context;

/// Decodes one `.nspkg` asset category's container bytes into a typed Bevy
/// asset.
///
/// Implementors are engine code, registered once per category via
/// [`crate::NspkgAssetApp::register_nspkg_asset`]. A decoder never sees
/// unclassified bytes: by the time [`NspkgDecoder::decode`] runs, the
/// container has already round-tripped through [`ContainerReader::parse`]
/// and its self-reported identity has already been checked against the
/// path it was loaded from.
pub trait NspkgDecoder: Send + Sync + 'static {
    /// The runtime asset type this decoder produces.
    type Asset: Asset;
    /// The decode error type.
    type Error: Into<BevyError>;

    /// Decode `container` (the asset's own package-local PUID is `puid`)
    /// into `Self::Asset`, using `context` to resolve any same-package or
    /// cross-package dependencies into typed Bevy handles.
    fn decode(
        &self,
        container: &ContainerReader<'_>,
        puid: &AssetPuid,
        context: &mut NorthstarLoadContext<'_, '_>,
    ) -> Result<Self::Asset, Self::Error>;
}

/// Everything that can go wrong loading one `.nspkg` asset before a
/// [`NspkgDecoder`] ever sees it.
#[derive(Debug, Error)]
pub enum NspkgLoadError {
    #[error("failed to read asset bytes: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not classify \"{path}\" as an .nspkg asset: {source}")]
    Classify {
        path: String,
        #[source]
        source: ClassifyError,
    },
    #[error("\"{path}\" classifies as a complete package, not an individually loadable asset")]
    NotAnAsset { path: String },
    #[error(
        "\"{path}\" has category \"{found}\" but this loader is registered for category \"{expected}\""
    )]
    CategoryMismatch {
        path: String,
        expected: AssetCategory,
        found: AssetCategory,
    },
    #[error("malformed .nspkg container at \"{path}\": {source}")]
    Container {
        path: String,
        #[source]
        source: ContainerError,
    },
    #[error(
        "\"{path}\" classifies as category \"{from_filename}\" but its container header \
         self-identifies as category \"{from_container}\" — refusing to load a mismatched container"
    )]
    IdentityMismatch {
        path: String,
        from_filename: AssetCategory,
        from_container: AssetCategory,
    },
    #[error("decoding \"{path}\" failed: {cause}")]
    Decode { path: String, cause: BevyError },
}

/// Routes one `.nspkg` category to a Bevy [`Asset`] type via a
/// [`NspkgDecoder`].
///
/// One `NspkgLoader<D>` instance is registered per category by
/// [`crate::NspkgAssetApp::register_nspkg_asset`]; several such loaders may
/// all declare the `nspkg` extension because Bevy disambiguates them by the
/// requested `Handle<A>` type, not by extension alone — see
/// `docs/architecture.md`.
pub struct NspkgLoader<D: NspkgDecoder> {
    category: AssetCategory,
    decoder: D,
}

impl<D: NspkgDecoder> NspkgLoader<D> {
    pub(crate) fn new(category: AssetCategory, decoder: D) -> Self {
        Self { category, decoder }
    }
}

// `AssetLoader` requires `TypePath`, which normally comes from
// `#[derive(TypePath)]`. That derive doesn't support our generic `D`
// parameter, and this internal type is never exposed to reflection UI, so a
// direct implementation over `core::any::type_name` (itself always
// `&'static`) is sufficient here.
impl<D: NspkgDecoder> TypePath for NspkgLoader<D> {
    fn type_path() -> &'static str {
        core::any::type_name::<Self>()
    }

    fn short_type_path() -> &'static str {
        core::any::type_name::<Self>()
    }
}

impl<D: NspkgDecoder> AssetLoader for NspkgLoader<D> {
    type Asset = D::Asset;
    type Settings = ();
    type Error = NspkgLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let display_path = load_context.path().to_string();

        // Classify by filename alone, before touching the container bytes.
        let classified =
            ClassifiedFilename::classify_path(load_context.path().path()).map_err(|source| {
                NspkgLoadError::Classify {
                    path: display_path.clone(),
                    source,
                }
            })?;
        let (puid, filename_category) = match classified {
            ClassifiedFilename::Package { .. } => {
                return Err(NspkgLoadError::NotAnAsset { path: display_path });
            }
            ClassifiedFilename::Asset { puid, category } => (puid, category),
        };
        if filename_category != self.category {
            return Err(NspkgLoadError::CategoryMismatch {
                path: display_path,
                expected: self.category.clone(),
                found: filename_category,
            });
        }

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let container =
            ContainerReader::parse(&bytes).map_err(|source| NspkgLoadError::Container {
                path: display_path.clone(),
                source,
            })?;

        // The container's self-reported identity must agree with what the
        // filename claims — this is the "detect obvious mismatches when a
        // file is actually loaded" check from the container format design.
        if let ClassifiedFilename::Asset {
            category: container_category,
            ..
        } = container.self_identity()
            && *container_category != self.category
        {
            return Err(NspkgLoadError::IdentityMismatch {
                path: display_path,
                from_filename: self.category.clone(),
                from_container: container_category.clone(),
            });
        }

        let owning_package = package_id_from_load_context(load_context);
        let mut ctx = NorthstarLoadContext::new(load_context, owning_package);

        self.decoder
            .decode(&container, &puid, &mut ctx)
            .map_err(|error| NspkgLoadError::Decode {
                path: display_path,
                cause: error.into(),
            })
    }

    fn extensions(&self) -> &[&str] {
        &["nspkg"]
    }
}
