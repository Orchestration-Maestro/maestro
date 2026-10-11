//! Shared prepared suffix positions with globstar link eligibility.
use maestro_pattern::{Minimatch, ParseReturnFiltered};
/// Identity of one prepared suffix, retained across directory transitions.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Pattern {
    /// Prepared alternative index.
    pub(super) alternative: usize,
    /// First pending component.
    pub(super) index: usize,
}
impl Pattern {
    /// Borrows the pending component.
    pub(super) fn part(self, matcher: &Minimatch) -> &ParseReturnFiltered {
        &matcher.set()[self.alternative][self.index]
    }
    /// Consumes one component without copying the suffix's state.
    pub(super) fn rest(self, matcher: &Minimatch) -> Option<Self> {
        (self.index + 1 < matcher.set()[self.alternative].len()).then_some(Self {
            index: self.index + 1,
            ..self
        })
    }
    /// Selects a native root and the first component after it.
    pub(super) fn root(self, matcher: &Minimatch) -> Option<(String, usize)> {
        if self.index != 0 {
            return None;
        }
        let parts = &matcher.glob_parts()[self.alternative];
        if cfg!(windows) && parts.starts_with(&[String::new(), String::new()]) && parts.len() >= 4 {
            return Some((format!("{}/", parts[..4].join("/")), 4));
        }
        if cfg!(windows) && parts[0].len() == 2 && parts[0].ends_with(':') {
            return Some((format!("{}/", parts[0]), 1));
        }
        (parts[0].is_empty()).then(|| ("/".into(), 1))
    }
}
