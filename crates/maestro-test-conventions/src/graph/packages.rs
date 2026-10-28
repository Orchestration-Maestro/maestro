//! Guest-only package edges and single-export component layout.

use std::collections::BTreeSet;

use cargo_metadata::{CrateType, Metadata, TargetKind};
use proc_macro2::{TokenStream, TokenTree};

use super::{Edge, metadata::members, policy::UNIFIER};
use crate::source::{Member, test_scope};

/// Internal guest libraries available to a package on every target.
const DEPENDENCIES: &[&str] = &[
    "maestro-extensions-wasm",
    "maestro-request",
    "maestro-entry",
    "maestro-tool",
    "maestro-json",
];

/// Handles every edge involving a package before native-owner exceptions apply.
pub(super) fn dependency(edge: &Edge, packages: &BTreeSet<String>) -> Option<Result<(), String>> {
    let from = packages.contains(&edge.from);
    let to = packages.contains(&edge.to);
    if to && !from {
        return Some(Err(format!(
            "non-package crate {} must not depend on package crate {}",
            edge.from, edge.to
        )));
    }
    from.then(|| {
        if to || DEPENDENCIES.contains(&edge.to.as_str()) || edge.to == UNIFIER {
            Ok(())
        } else {
            Err(format!(
                "forbidden package dependency: {} -> {}",
                edge.from, edge.to
            ))
        }
    })
}

/// Checks reusable libraries and their one separately linked component export.
pub(crate) fn check(
    metadata: &Metadata,
    sources: &[Member],
    packages: &BTreeSet<String>,
) -> Result<(), String> {
    for package in members(metadata).filter(|package| packages.contains(package.name.as_str())) {
        let library = package.targets.iter().find(|target| target.is_rlib());
        let component = package.targets.iter().find(|target| {
            target.name == "component" && target.kind.contains(&TargetKind::Example)
        });
        let directory = package
            .manifest_path
            .parent()
            .ok_or("manifest has no parent")?;
        if library.is_none_or(|target| target.crate_types != [CrateType::RLib])
            || component.is_none_or(|target| {
                target.crate_types != [CrateType::CDyLib]
                    || target.src_path != directory.join("src/component.rs")
            })
        {
            return Err(format!(
                "{}: package needs an rlib and a cdylib example component at src/component.rs",
                package.name
            ));
        }
        let member = sources
            .iter()
            .find(|member| member.name == package.name.as_str())
            .ok_or("missing package sources")?;
        check_exports(member)?;
    }
    Ok(())
}

/// Rejects exports outside the component entry and requires the reusable factory.
fn check_exports(member: &Member) -> Result<(), String> {
    let mut count = 0;
    let mut factory = false;
    for source in &member.sources {
        if test_scope(member, source) {
            continue;
        }
        let Some(syntax) = &source.syntax else {
            continue;
        };
        if source.path == member.directory.join("src/lib.rs") {
            factory = syntax.items.iter().any(|item| {
                matches!(item,
                syn::Item::Fn(item) if item.sig.ident == "factory"
                    && matches!(item.vis, syn::Visibility::Public(_)))
            });
        }
        let exports = exports(
            source
                .contents
                .parse()
                .map_err(|error| format!("{error}"))?,
        );
        if exports > 0 && source.path != member.directory.join("src/component.rs") {
            return Err(format!(
                "{}: export_extension belongs only in src/component.rs",
                member.name
            ));
        }
        count += exports;
    }
    if !factory || count != 1 {
        return Err(format!(
            "{}: package needs a public factory and exactly one export_extension",
            member.name
        ));
    }
    Ok(())
}

/// Counts literal macro invocations without interpreting comments or strings.
fn exports(tokens: TokenStream) -> usize {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut count = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Ident(name)
                if name == "export_extension"
                    && matches!(tokens.get(index + 1), Some(TokenTree::Punct(mark)) if mark.as_char() == '!') =>
            {
                count += 1;
            }
            TokenTree::Group(group) => count += exports(group.stream()),
            _ => {}
        }
    }
    count
}
