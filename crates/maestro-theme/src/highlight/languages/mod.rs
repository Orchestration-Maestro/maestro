//! Ordered surface registries over shared immutable grammars.
use super::HighlightProfile;
use super::core::types::{Language, ModeId};
mod bash;
mod c;
mod cpp;
mod csharp;
mod css;
mod diff;
mod go;
mod graphql;
mod ini;
mod java;
mod javascript;
mod json;
mod kotlin;
mod less;
mod lua;
mod makefile;
mod markdown;
mod objectivec;
mod perl;
mod php;
mod php_template;
mod plaintext;
mod python;
mod python_repl;
mod r;
mod ruby;
mod rust;
mod scss;
mod shell;
mod sql;
mod swift;
mod typescript;
mod vbnet;
mod wasm;
mod xml;
mod yaml;

/// Ordered viewer registrations.
const VIEWER: &[Language] = &[
    bash::VIEWER,
    c::VIEWER,
    cpp::VIEWER,
    csharp::VIEWER,
    css::VIEWER,
    diff::VIEWER,
    go::VIEWER,
    graphql::VIEWER,
    ini::VIEWER,
    java::VIEWER,
    javascript::VIEWER,
    json::VIEWER,
    kotlin::VIEWER,
    less::VIEWER,
    lua::VIEWER,
    makefile::VIEWER,
    markdown::VIEWER,
    objectivec::VIEWER,
    perl::VIEWER,
    php::VIEWER,
    php_template::VIEWER,
    plaintext::VIEWER,
    python::VIEWER,
    python_repl::VIEWER,
    r::VIEWER,
    ruby::VIEWER,
    rust::VIEWER,
    scss::VIEWER,
    shell::VIEWER,
    sql::VIEWER,
    swift::VIEWER,
    typescript::VIEWER,
    vbnet::VIEWER,
    wasm::VIEWER,
    xml::VIEWER,
    yaml::VIEWER,
];

/// Ordered browser registrations.
const BROWSER: &[Language] = &[
    javascript::BROWSER,
    typescript::BROWSER,
    python::BROWSER,
    xml::BROWSER_HTML,
    xml::BROWSER,
    css::BROWSER,
    json::BROWSER,
    bash::BROWSER,
    sql::BROWSER,
];

/// The registrations admitted by one surface.
pub(super) fn registered(profile: HighlightProfile) -> &'static [Language] {
    match profile {
        HighlightProfile::ExportViewer => VIEWER,
        HighlightProfile::CodeBlock => BROWSER,
    }
}

/// Resolve a raw label without trimming or splitting it.
pub(super) fn lookup<'a>(languages: &'a [Language], label: &str) -> Option<&'a Language> {
    let label = label.to_lowercase();
    languages.iter().find(|l| l.name == label).or_else(|| {
        languages
            .iter()
            .rev()
            .find(|l| l.aliases.contains(&label.as_str()))
    })
}
