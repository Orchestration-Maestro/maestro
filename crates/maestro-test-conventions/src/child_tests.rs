//! Exact test selection belongs to the checked test-process owner.

use ra_ap_rustc_lexer::{LiteralKind, TokenKind};

use crate::source::{Member, Source, line};
use syn::{spanned::Spanned, visit::Visit};

/// Reject an exact-selector string token outside its owning package.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            ignore_ownership(member, source)?;
            if member.name != "maestro-test-process"
                && let Some(offset) = exact_offset(source)
            {
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

/// The sole execution-owner reason for browser-only assertions.
const CHROMIUM: &str = "runs in the pinned Chromium job";

/// Check parsed attributes through the existing workspace source model.
fn ignore_ownership(member: &Member, source: &Source) -> Result<(), String> {
    let Some(syntax) = &source.syntax else {
        return Ok(());
    };
    let browser = member.name == "maestro-test-browser"
        && source.path == member.directory.join("tests/browser.rs");
    let mut visitor = Ignored {
        browser,
        error: None,
    };
    visitor.visit_file(syntax);
    if let Some((line, message)) = visitor.error {
        return Err(format!("{}:{line}: {message}", source.path.display()));
    }
    if browser && !member.sources.iter().any(|source| {
        source.path == member.directory.join("tests/integration.rs")
            && source.syntax.as_ref().is_some_and(|syntax| syntax.items.iter().any(|item| {
                matches!(item, syn::Item::Mod(module) if module.ident == "browser" && module.content.is_none())
            }))
    }) {
        return Err(format!("{}: browser tests require registration in tests/integration.rs", source.path.display()));
    }
    Ok(())
}

/// Ignore-attribute ownership during a parsed syntax walk.
struct Ignored {
    /// Whether the source is the dedicated pinned-browser module.
    browser: bool,
    /// First offending line and diagnostic.
    error: Option<(usize, &'static str)>,
}

impl<'ast> Visit<'ast> for Ignored {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if attribute.path().is_ident("ignore") {
            let reason = ignore_reason(attribute);
            if reason.as_deref() != Some("runs only inside a controlled child")
                && !(self.browser && reason.as_deref() == Some(CHROMIUM))
            {
                self.error.get_or_insert((attribute.span().start().line, "ignored tests must name their controlled-child or pinned-browser execution owner"));
            }
        }
        syn::visit::visit_attribute(self, attribute);
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if self.browser
            && function
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("test"))
            && !function
                .attrs
                .iter()
                .any(|attr| ignore_reason(attr).as_deref() == Some(CHROMIUM))
        {
            self.error.get_or_insert((
                function.span().start().line,
                "browser tests must run in the pinned Chromium job",
            ));
        }
        syn::visit::visit_item_fn(self, function);
    }
}

/// Decode a direct ignore attribute's literal reason.
fn ignore_reason(attribute: &syn::Attribute) -> Option<String> {
    if attribute.path().is_ident("ignore")
        && let syn::Meta::NameValue(value) = &attribute.meta
        && let syn::Expr::Lit(value) = &value.value
        && let syn::Lit::Str(value) = &value.lit
    {
        Some(value.value())
    } else {
        None
    }
}

#[cfg(test)]
thread_local! {
    /// Literal tokens decoded by the exact-selector search on the current test thread.
    static DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Decode one string literal token, or fail for any other token.
fn decode(token: &str) -> syn::Result<syn::LitStr> {
    #[cfg(test)]
    DECODES.set(DECODES.get() + 1);
    syn::parse_str(token)
}

/// Find the first string token whose decoded value is the exact-selector flag.
fn exact_offset(source: &Source) -> Option<usize> {
    for (kind, range) in &source.tokens {
        let offset = range.start;
        let end = range.end;
        // Only string kinds can decode as a string literal.
        if matches!(
            kind,
            TokenKind::Literal {
                kind: LiteralKind::Str { .. } | LiteralKind::RawStr { .. },
                ..
            }
        ) && decode(&source.contents[offset..end])
            .is_ok_and(|literal| literal.value().as_bytes() == b"--exact")
        {
            return Some(offset);
        }
    }
    None
}

#[cfg(test)]
mod tests;
