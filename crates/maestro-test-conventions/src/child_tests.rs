//! Exact test selection belongs to the checked test-process owner.

use ra_ap_rustc_lexer::TokenKind;

use crate::source::{Member, line};

/// Reject an exact-selector string token outside its owning package.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members
        .iter()
        .filter(|member| member.name != "maestro-test-process")
    {
        for source in &member.sources {
            if let Some(offset) = exact_offset(&source.contents) {
                return Err(format!(
                    "{}:{}: exact test selection belongs to maestro-test-process",
                    source.path.display(),
                    line(&source.contents, offset)
                ));
            }
        }
    }
    Ok(())
}

/// Find the first string token whose decoded value is the exact-selector flag.
fn exact_offset(source: &str) -> Option<usize> {
    for (kind, range) in crate::source::tokens(source) {
        let offset = range.start;
        let end = range.end;
        if matches!(kind, TokenKind::Literal { .. })
            && syn::parse_str::<syn::LitStr>(&source[offset..end])
                .is_ok_and(|literal| literal.value().as_bytes() == b"--exact")
        {
            return Some(offset);
        }
    }
    None
}

#[cfg(test)]
mod tests;
