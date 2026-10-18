//! Independent refresh demands supplied to the effect owner.
/// Which repository observations a refresh needs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitStatusRefreshDemand {
    /// Refresh the branch name.
    pub branch: bool,
    /// Refresh upstream ahead/behind counts.
    pub ahead_behind: bool,
}
impl GitStatusRefreshDemand {
    /// Whether neither observation is requested.
    #[must_use]
    pub fn is_empty(self) -> bool {
        !self.branch && !self.ahead_behind
    }
}
