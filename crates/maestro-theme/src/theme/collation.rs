//! Locale collation shared by native inventory sorting and tool listings.
use icu_collator::{CollatorBorrowed, options::CollatorOptions};
use std::{cmp::Ordering, io};

/// A locale-aware string comparison.
pub struct Collator(CollatorBorrowed<'static>);

impl Collator {
    /// Prepare a collator from the first present of `LC_ALL`, `LC_MESSAGES` and `LANG`.
    ///
    /// Encoding and modifier suffixes are dropped, `C` and `POSIX` mean `en-US`, and
    /// text that is not a locale selects the library default.
    ///
    /// # Errors
    /// Returns the library's failure to load collation data.
    pub fn new(environment: &dyn Fn(&str) -> Option<String>) -> io::Result<Self> {
        let raw = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(environment)
            .unwrap_or_else(|| "en_US".into());
        let base = raw.split(['.', '@']).next().unwrap_or_default();
        let locale = match base {
            "C" | "POSIX" => "en-US".into(),
            "" => "und".into(),
            value => value.replace('_', "-"),
        };
        let preferences = locale
            .parse::<icu_locale_core::Locale>()
            .map(Into::into)
            .unwrap_or_default();
        icu_collator::Collator::try_new(preferences, CollatorOptions::default())
            .map(Self)
            .map_err(io::Error::other)
    }

    /// Order `left` before, equal to or after `right`.
    #[must_use]
    pub fn compare(&self, left: &str, right: &str) -> Ordering {
        self.0.compare(left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::Collator;
    use std::cmp::Ordering;

    /// Compare `left` with `right` under the locale operands `env`.
    fn compare(env: &[(&str, &str)], left: &str, right: &str) -> Ordering {
        let lookup = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        };
        Collator::new(&lookup).unwrap().compare(left, right)
    }

    #[test]
    fn theme_collation_follows_the_first_locale_operand() {
        assert_eq!(
            compare(&[("LANG", "sv_SE.UTF-8")], "z", "ä"),
            Ordering::Less
        );
        assert_eq!(
            compare(&[("LANG", "de_DE@euro")], "z", "ä"),
            Ordering::Greater
        );
        let all = [
            ("LC_ALL", "de_DE"),
            ("LC_MESSAGES", "sv_SE"),
            ("LANG", "sv_SE"),
        ];
        assert_eq!(compare(&all, "z", "ä"), Ordering::Greater);
        let messages = [("LC_MESSAGES", "sv_SE"), ("LANG", "de_DE")];
        assert_eq!(compare(&messages, "z", "ä"), Ordering::Less);
    }

    #[test]
    fn theme_collation_defaults_and_tolerates_unusable_locales() {
        for env in [
            &[][..],
            &[("LANG", "C")],
            &[("LANG", "POSIX")],
            &[("LANG", "")],
            &[("LANG", "not a locale!")],
        ] {
            assert_eq!(compare(env, "a", "A"), Ordering::Less, "{env:?}");
            assert_eq!(compare(env, "é", "e\u{301}"), Ordering::Equal, "{env:?}");
        }
    }
}
