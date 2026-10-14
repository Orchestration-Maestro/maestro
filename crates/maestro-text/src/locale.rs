//! Shared first-present environment locale selection.

/// Selects the locale without retaining the caller's reader.
pub(crate) fn select(environment: &dyn Fn(&str) -> Option<String>) -> icu_locale_core::Locale {
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
    locale.parse().unwrap_or(icu_locale_core::Locale::UNKNOWN)
}
