//! Terminal dimensions: the size of standard output, then the environment, then a default.

use std::cell::Cell;

use rustix::stdio::stdout;
use rustix::termios::tcgetwinsize;

/// The width used when neither the terminal nor `COLUMNS` supplies a positive one.
const DEFAULT_COLUMNS: usize = 80;

/// The height used when neither the terminal nor `LINES` supplies a positive one.
const DEFAULT_ROWS: usize = 24;

/// A window size, where zero means the terminal reported no extent on that axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Size {
    /// Width in columns.
    columns: usize,
    /// Height in rows.
    rows: usize,
}

impl Size {
    /// The size of standard output, or `None` when standard output reports none.
    fn query() -> Option<Self> {
        let size = tcgetwinsize(stdout()).ok()?;
        Some(Self {
            columns: usize::from(size.ws_col),
            rows: usize::from(size.ws_row),
        })
    }
}

/// The size of standard output as last observed.
#[derive(Debug, Default)]
pub(super) struct Dimensions(Cell<Size>);

impl Dimensions {
    /// Observes the size of standard output now.
    pub(super) fn observe() -> Self {
        let dimensions = Self::default();
        dimensions.refresh();
        dimensions
    }

    /// Observes the size again and reports whether it differs from the last observation.
    pub(super) fn refresh(&self) -> bool {
        self.apply(Size::query())
    }

    /// Records `observed` and reports whether it differs. An absent observation changes nothing.
    fn apply(&self, observed: Option<Size>) -> bool {
        observed.is_some_and(|size| self.0.replace(size) != size)
    }

    /// The observed width when positive, else a valid `COLUMNS`, else 80.
    pub(super) fn columns(&self) -> usize {
        extent(self.0.get().columns, "COLUMNS", DEFAULT_COLUMNS)
    }

    /// The observed height when positive, else a valid `LINES`, else 24.
    pub(super) fn rows(&self) -> usize {
        extent(self.0.get().rows, "LINES", DEFAULT_ROWS)
    }
}

/// `observed` when positive, else the positive decimal in `variable`, else `default`.
fn extent(observed: usize, variable: &str, default: usize) -> usize {
    if observed > 0 {
        return observed;
    }
    std::env::var(variable)
        .ok()
        .and_then(|text| positive_decimal(&text))
        .unwrap_or(default)
}

/// The value of `text` when, after ASCII whitespace is trimmed, it is nonempty ASCII digits
/// naming a nonzero number that fits `usize`.
fn positive_decimal(text: &str) -> Option<usize> {
    let digits = text.trim_ascii();
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|&number| number > 0)
}

#[cfg(test)]
mod tests {
    use super::{Dimensions, Size, positive_decimal};

    /// The size observed last.
    fn observed(dimensions: &Dimensions) -> Size {
        dimensions.0.get()
    }

    #[test]
    fn a_changed_size_is_reported_once_and_an_unchanged_one_is_not() {
        let dimensions = Dimensions::default();
        let (small, large) = (
            Size {
                columns: 80,
                rows: 24,
            },
            Size {
                columns: 100,
                rows: 24,
            },
        );
        assert!(dimensions.apply(Some(small)));
        assert!(!dimensions.apply(Some(small)));
        assert!(dimensions.apply(Some(large)));
        assert_eq!(observed(&dimensions), large);
    }

    #[test]
    fn a_failed_query_keeps_the_last_observed_size_and_reports_no_change() {
        let dimensions = Dimensions::default();
        let size = Size {
            columns: 80,
            rows: 24,
        };
        dimensions.apply(Some(size));
        assert!(!dimensions.apply(None));
        assert_eq!(observed(&dimensions), size);
    }

    #[test]
    fn a_size_of_zero_replaces_the_observation_and_is_a_change() {
        let dimensions = Dimensions::default();
        dimensions.apply(Some(Size {
            columns: 80,
            rows: 24,
        }));
        assert!(dimensions.apply(Some(Size::default())));
        assert_eq!(observed(&dimensions), Size::default());
    }

    #[test]
    fn an_embedded_nul_is_not_a_decimal() {
        assert_eq!(positive_decimal("123\0"), None);
    }
}
