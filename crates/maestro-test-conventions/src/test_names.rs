//! Behaviour names for attributed tests, without redundant prefixes or numbered labels.

use syn::ext::IdentExt;

use crate::source::Member;

/// Report every attributed test whose name carries a redundant prefix or numbered label.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let diagnostics: Vec<_> = members
        .iter()
        .flat_map(|member| &member.sources)
        .flat_map(|source| {
            source.walk.iter().flat_map(|walk| &walk.test_names).map(|(line, name)| {
                format!(
                    "{}:{line}: test `{name}` must describe behaviour without a maestro_, test_ or numbered issue_ label",
                    source.path.display()
                )
            })
        })
        .collect();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// Record a forbidden name only when the function carries a test attribute.
pub(crate) fn record(function: &syn::ItemFn, names: &mut Vec<(usize, String)>) {
    if function.attrs.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident.unraw() == "test")
    }) {
        let name = function.sig.ident.unraw().to_string();
        if name.starts_with("maestro_")
            || name.starts_with("test_")
            || name.match_indices("issue_").any(|(offset, label)| {
                name.as_bytes()
                    .get(offset + label.len())
                    .is_some_and(u8::is_ascii_digit)
            })
        {
            names.push((function.sig.ident.span().start().line, name));
        }
    }
}
