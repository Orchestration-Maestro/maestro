//! Native ID comparison using caller-supplied locale operands.
use icu_collator::{Collator, options::CollatorOptions};
use std::{cmp::Ordering, io};
/// Prepare ID comparison from the first present locale environment operand.
///
/// Reads `LC_ALL`, `LC_MESSAGES`, then `LANG`; unusable text selects ICU defaults.
/// The returned closure retains the prepared collator, not the reader.
///
/// # Errors
/// Returns a native collator construction failure.
pub fn model_id_comparator(
    environment: &dyn Fn(&str) -> Option<String>,
) -> io::Result<impl Fn(&str, &str) -> Ordering + use<>> {
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
    let collator =
        Collator::try_new(preferences, CollatorOptions::default()).map_err(io::Error::other)?;
    Ok(move |left: &str, right: &str| collator.compare(left, right))
}
