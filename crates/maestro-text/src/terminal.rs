//! Terminal escape stripping and binary-output control filtering.
use regex::Regex;
use std::sync::LazyLock;

/// Terminal escape sequences: OSC strings up to their first terminator, and CSI and related
/// introducers with their parameters and a final byte from the listed set.
const ANSI_PATTERN: &str = r"(?:\x{001B}\][\s\S]*?(?:\x{0007}|\x{001B}\\|\x{009C}))|[\x{001B}\x{009B}][\[\]()#;?]*(?:[0-9]{1,4}(?:[;:][0-9]{0,4})*)?[0-9A-PR-TZcf-nq-uy=><~]";

/// The compiled escape expression, built once.
static ANSI: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| Regex::new(ANSI_PATTERN));

/// Borrows the single compiled terminal escape expression.
///
/// # Errors
/// Returns the failure to compile the escape expression.
pub fn ansi_regex() -> Result<&'static Regex, &'static regex::Error> {
    ANSI.as_ref()
}

/// Removes every match of the terminal escape expression.
///
/// # Errors
/// Returns the failure to compile the escape expression.
pub fn strip_ansi(text: &str) -> Result<String, &'static regex::Error> {
    Ok(ansi_regex()?.replace_all(text, "").into_owned())
}

/// Drop C0 controls other than TAB, LF and CR, and the interlinear annotation marks
/// U+FFF9 to U+FFFB; DEL, C1 controls, ZWJ and every other scalar stay.
#[must_use]
pub fn sanitize_binary_output(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r')
                || !(c <= '\u{1f}' || ('\u{fff9}'..='\u{fffb}').contains(&c))
        })
        .collect()
}
