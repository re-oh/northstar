//! The editor `View` lifecycle contract.
//!
//! This crate defines the shape of an editor panel/tab — identity, kind,
//! title, activation/deactivation, closing (with the option to block on
//! unsaved changes), and state serialization — plus a minimal
//! [`WorkspaceHandle`] a view can use to talk back to whatever holds it.
//!
//! **No UI library is chosen or referenced here.** That's deliberate: see
//! `docs/editor-views.md` for why, and what picking one will need to add on
//! top of (not instead of) this trait.

mod view;
mod view_id;
mod view_kind;
mod workspace;

pub use view::{CloseResponse, View, ViewDescriptor, ViewStateError};
pub use view_id::ViewId;
pub use view_kind::ViewKind;
pub use workspace::{ViewContext, WorkspaceHandle};
