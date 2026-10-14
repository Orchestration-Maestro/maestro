//! Locale comparison prepared from caller-supplied environment observations.
use icu_collator::{CollatorBorrowed, options::CollatorOptions};
use std::{cmp::Ordering, io};

/// A prepared comparator; it keeps the chosen locale, never the reader that chose it.
pub struct Collator(CollatorBorrowed<'static>);

impl Collator {
    /// Prepare from the first present of `LC_ALL`, `LC_MESSAGES` and `LANG`, empty
    /// values included; none present means `en_US`.
    ///
    /// Encoding and modifier suffixes are dropped, `C` and `POSIX` mean `en-US`, an empty
    /// name means `und`, and text that is not a locale selects the library default.
    ///
    /// # Errors
    /// Returns a native collator construction failure.
    pub fn new(environment: &dyn Fn(&str) -> Option<String>) -> io::Result<Self> {
        CollatorBorrowed::try_new(
            crate::locale::select(environment).into(),
            CollatorOptions::default(),
        )
        .map(Self)
        .map_err(io::Error::other)
    }

    /// Order `left` against `right` under the prepared locale.
    #[must_use]
    pub fn compare(&self, left: &str, right: &str) -> Ordering {
        self.0.compare(left, right)
    }
}
