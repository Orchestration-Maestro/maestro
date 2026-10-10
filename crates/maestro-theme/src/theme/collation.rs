//! Locale collation for native inventory sorting.
use icu_collator::{Collator, CollatorBorrowed, options::CollatorOptions};
use std::io;

/// Prepare a collator from the first present of `LC_ALL`, `LC_MESSAGES` and `LANG`.
///
/// Encoding and modifier suffixes are dropped, `C` and `POSIX` mean `en-US`, and
/// text that is not a locale selects the library default.
pub(super) fn collator(
    environment: &dyn Fn(&str) -> Option<String>,
) -> io::Result<CollatorBorrowed<'static>> {
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
    Collator::try_new(preferences, CollatorOptions::default()).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::collator;
    use std::cmp::Ordering;

    /// Compare `left` with `right` under the locale operands `env`.
    fn compare(env: &[(&str, &str)], left: &str, right: &str) -> Ordering {
        let lookup = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        };
        collator(&lookup).unwrap().compare(left, right)
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
