use thiserror::Error;

use crate::view_id::ViewId;
use crate::view_kind::ViewKind;
use crate::workspace::ViewContext;

/// A view's answer to "can you close right now?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResponse {
    /// Nothing unsaved; the workspace may close this view immediately.
    Allow,
    /// This view has unsaved changes or an in-progress operation; the
    /// workspace should prompt the user (save/discard/cancel) rather than
    /// close silently. What that prompt looks like is a workspace/UI
    /// concern, not this crate's.
    Block,
}

/// Failure reporting for view state (de)serialization. See `docs/errors.md`
/// — this is deliberately its own small error type, not folded into a
/// larger one, and callers that need more context should wrap this rather
/// than the other way around.
#[derive(Debug, Error)]
pub enum ViewStateError {
    #[error("failed to serialize view state: {0}")]
    Serialize(String),
    #[error("failed to restore view state: {0}")]
    Restore(String),
}

/// Registration-time facts about a view kind, independent of any one open
/// instance. A future view registry (analogous to
/// `northstar_bevy`'s category registry) would key on [`ViewKind`] and
/// store one of these per registered kind.
#[derive(Debug, Clone)]
pub struct ViewDescriptor {
    pub kind: ViewKind,
    /// Human-readable name shown wherever the workspace lists available
    /// view kinds (e.g. an "Add View" menu).
    pub display_name: &'static str,
    /// If true, the workspace should never have more than one open
    /// instance of this kind at a time (e.g. a single asset browser rather
    /// than one per package). If false, multiple instances may be open
    /// as separate tabs/panels simultaneously.
    pub singleton: bool,
}

/// The lifecycle contract every editor panel/tab implements.
///
/// A `View` owns **persistent internal state** across being hidden, shown,
/// docked, or undocked — the workspace is not responsible for remembering
/// anything about a view's content, only for *when* to call these methods
/// and where to place the view visually. Whether a given `View` instance
/// is presented as a standalone panel or one of several tabs within a
/// container is a workspace layout decision; this trait does not
/// distinguish the two.
///
/// Object-safe by design (`&dyn View` / `Box<dyn View>`), since the
/// workspace holds a heterogeneous collection of open views. No UI
/// framework is referenced here — see `docs/editor-views.md` for why that
/// choice is deferred, and what a real implementation will need to add on
/// top of this trait once it's made.
pub trait View {
    /// This open instance's identity. Stable for the lifetime of the
    /// instance.
    fn id(&self) -> ViewId;

    /// What kind of view this is. Should match the `ViewKind` of whatever
    /// `ViewDescriptor` this instance was created from.
    fn kind(&self) -> ViewKind;

    /// The current display title (e.g. tab label). May change over time
    /// (a common case: appending `"*"` while there are unsaved changes) —
    /// callers should not cache this.
    fn title(&self) -> String;

    /// Called when this view becomes the active/focused view. Default:
    /// no-op.
    fn on_activate(&mut self, _ctx: &mut ViewContext<'_>) {}

    /// Called when this view stops being the active/focused view (another
    /// view was activated, or the workspace lost focus entirely). Default:
    /// no-op.
    fn on_deactivate(&mut self, _ctx: &mut ViewContext<'_>) {}

    /// Called when the workspace is about to close this view. Returning
    /// [`CloseResponse::Block`] asks the workspace to prompt the user
    /// instead of closing immediately. Default: always allow.
    fn on_close(&mut self, _ctx: &mut ViewContext<'_>) -> CloseResponse {
        CloseResponse::Allow
    }

    /// Serialize this view's persistent state (e.g. for workspace-layout
    /// save). The encoding is entirely up to the implementation — this
    /// crate does not impose one (RON, bincode, JSON, whatever fits) — see
    /// `docs/editor-views.md`.
    fn serialize_state(&self) -> Result<Vec<u8>, ViewStateError>;

    /// Restore this view's persistent state from bytes produced by a
    /// previous [`View::serialize_state`] call. Implementations should
    /// treat `bytes` as untrusted (a saved-layout file may be hand-edited
    /// or stale) and return [`ViewStateError::Restore`] rather than
    /// panicking on malformed input — same rule as everywhere else
    /// untrusted bytes are parsed, see `docs/errors.md`.
    fn deserialize_state(&mut self, bytes: &[u8]) -> Result<(), ViewStateError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_id::ViewId;

    /// A trivial `View` impl, only to prove the trait is actually
    /// object-safe and usable through `Box<dyn View>` the way a real
    /// workspace would hold it.
    struct NoteView {
        id: ViewId,
        text: String,
    }

    impl View for NoteView {
        fn id(&self) -> ViewId {
            self.id
        }

        fn kind(&self) -> ViewKind {
            ViewKind::new("note")
        }

        fn title(&self) -> String {
            "Note".to_owned()
        }

        fn serialize_state(&self) -> Result<Vec<u8>, ViewStateError> {
            Ok(self.text.clone().into_bytes())
        }

        fn deserialize_state(&mut self, bytes: &[u8]) -> Result<(), ViewStateError> {
            self.text = String::from_utf8(bytes.to_vec())
                .map_err(|e| ViewStateError::Restore(e.to_string()))?;
            Ok(())
        }
    }

    #[test]
    fn view_is_object_safe_and_round_trips_state() {
        let mut view: Box<dyn View> = Box::new(NoteView {
            id: ViewId::new(),
            text: "hello".to_owned(),
        });

        let bytes = view.serialize_state().unwrap();
        view.deserialize_state(&bytes).unwrap();
        assert_eq!(view.serialize_state().unwrap(), b"hello");
        assert_eq!(view.title(), "Note");
        assert_eq!(view.kind(), ViewKind::new("note"));
    }

    #[test]
    fn default_close_response_allows_closing() {
        struct Empty(ViewId);
        impl View for Empty {
            fn id(&self) -> ViewId {
                self.0
            }
            fn kind(&self) -> ViewKind {
                ViewKind::new("empty")
            }
            fn title(&self) -> String {
                String::new()
            }
            fn serialize_state(&self) -> Result<Vec<u8>, ViewStateError> {
                Ok(Vec::new())
            }
            fn deserialize_state(&mut self, _bytes: &[u8]) -> Result<(), ViewStateError> {
                Ok(())
            }
        }

        struct NullWorkspace;
        impl crate::workspace::WorkspaceHandle for NullWorkspace {
            fn request_open(&mut self, _kind: &ViewKind) {}
            fn request_close(&mut self, _id: ViewId) {}
            fn mark_dirty(&mut self, _id: ViewId) {}
            fn request_focus(&mut self, _id: ViewId) {}
        }

        let mut view = Empty(ViewId::new());
        let mut workspace = NullWorkspace;
        let mut ctx = ViewContext::new(&mut workspace);
        assert_eq!(view.on_close(&mut ctx), CloseResponse::Allow);
    }
}
