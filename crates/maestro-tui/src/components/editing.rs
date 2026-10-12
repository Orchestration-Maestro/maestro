//! The editing action vocabulary shared by [`Editor`](super::Editor) and [`Input`](super::Input).

/// Previous editing action used for coalescing and replacement eligibility.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum LastAction {
    /// No coalescing action.
    #[default]
    None,
    /// Consecutive typed word.
    TypeWord,
    /// Accumulating deleted text.
    Kill,
    /// Eligible inserted ring text.
    Yank,
}
