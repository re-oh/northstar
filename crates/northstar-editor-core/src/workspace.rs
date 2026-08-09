use crate::view_id::ViewId;
use crate::view_kind::ViewKind;

/// What a [`crate::View`] is allowed to ask the editor workspace for during
/// a lifecycle callback.
///
/// Kept as a small, object-safe trait rather than a concrete struct so the
/// eventual workspace implementation (whatever UI framework it's built on)
/// can implement it however it needs to, without this crate depending on
/// that framework. No method here is implemented by anything yet — this is
/// the shape of the interaction, not a working workspace.
pub trait WorkspaceHandle {
    /// Ask the workspace to open a new view of `kind`. Whether this opens
    /// immediately, queues, or is denied (e.g. a singleton view already
    /// open) is a workspace policy decision, not this trait's concern.
    fn request_open(&mut self, kind: &ViewKind);

    /// Ask the workspace to close the view identified by `id` — typically
    /// a view asking to close *itself*, but the trait doesn't prevent
    /// asking to close another view.
    fn request_close(&mut self, id: ViewId);

    /// Tell the workspace this view's persisted state has changed since it
    /// was last saved (e.g. to drive an "unsaved changes" indicator).
    fn mark_dirty(&mut self, id: ViewId);

    /// Ask the workspace to bring the view identified by `id` to the
    /// foreground/focus.
    fn request_focus(&mut self, id: ViewId);
}

/// Passed to [`crate::View`] lifecycle methods so they can interact with
/// the workspace without holding a direct reference to it.
pub struct ViewContext<'a> {
    workspace: &'a mut dyn WorkspaceHandle,
}

impl<'a> ViewContext<'a> {
    pub fn new(workspace: &'a mut dyn WorkspaceHandle) -> Self {
        Self { workspace }
    }

    pub fn workspace(&mut self) -> &mut dyn WorkspaceHandle {
        self.workspace
    }
}
