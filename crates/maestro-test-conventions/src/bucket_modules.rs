//! Module names that say where code was put.
//!
//! A module named `utils`, `util`, `helpers`, `common`, `misc`, `shared` or `types` names a
//! place, not what the module holds. The check reads the sources below each member's `src/`:
//! every module declaration, inline or not, and every module file and directory. A module is
//! keyed by its member and its path below `src/`, so a declaration, its file and its
//! directory are one module with one diagnostic. `EXEMPT` holds the modules that keep such a
//! name, each with its reason, and the workspace reports a row that matches no module.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::source::Member;

/// Module names that say where code was put.
const BUCKETS: &[&str] = &[
    "common", "helpers", "misc", "shared", "types", "util", "utils",
];

/// Why a module keeps a bucket name.
#[derive(Clone, Copy)]
enum Reason {
    /// The module mirrors a file or directory of the ported interface, whose layout stays.
    Mirrored,
    /// The module is Maestro's own and has yet to take a name for what it holds.
    Unnamed,
}

impl Reason {
    /// The reason as a diagnostic spells it.
    const fn text(self) -> &'static str {
        match self {
            Self::Mirrored => "mirrors the ported file layout",
            Self::Unnamed => "Maestro's own module, still to be named for what it holds",
        }
    }
}

/// A module that keeps a bucket name: its member, its path below `src/` and the reason.
type Row = (&'static str, &'static str, Reason);

/// Modules that keep a bucket name, sorted by member and then by module path, each once.
///
/// A row that matches no module fails the check, so a row leaves with its module.
const EXEMPT: &[Row] = &[
    ("maestro-agent", "types", Reason::Mirrored),
    ("maestro-app", "agent_session::types", Reason::Unnamed),
    ("maestro-app", "utils", Reason::Mirrored),
    ("maestro-extensions", "helpers", Reason::Unnamed),
    ("maestro-extensions", "types", Reason::Mirrored),
    ("maestro-extensions-wasm", "agent::types", Reason::Mirrored),
    (
        "maestro-extensions-wasm",
        "summarization::utils",
        Reason::Mirrored,
    ),
    ("maestro-extensions-wasm", "types", Reason::Mirrored),
    ("maestro-extensions-wasm", "utils", Reason::Mirrored),
    ("maestro-http", "pac_script::util", Reason::Unnamed),
    (
        "maestro-interchange",
        "entry::compactions::utils",
        Reason::Mirrored,
    ),
    ("maestro-interchange", "tool::types", Reason::Mirrored),
    ("maestro-interchange", "types", Reason::Mirrored),
    ("maestro-models", "oauth::types", Reason::Mirrored),
    ("maestro-models", "records::types", Reason::Mirrored),
    (
        "maestro-office",
        "spreadsheet::read::xls::formulas::types",
        Reason::Unnamed,
    ),
    ("maestro-protocol", "schema::common", Reason::Mirrored),
    ("maestro-protocol", "shared", Reason::Unnamed),
    ("maestro-rpc", "rpc_client::types", Reason::Unnamed),
    (
        "maestro-schema-validator",
        "authored::types",
        Reason::Unnamed,
    ),
    ("maestro-session", "summarization::utils", Reason::Mirrored),
    ("maestro-tools", "types", Reason::Mirrored),
    ("maestro-tools", "utils", Reason::Mirrored),
    ("maestro-tui", "text::utils", Reason::Mirrored),
    ("maestro-web", "persistence::types", Reason::Mirrored),
    ("maestro-web", "tools::types", Reason::Mirrored),
    ("maestro-web", "utils", Reason::Mirrored),
];

/// The file that holds the table, which marks the workspace that holds every row's module.
const LIST_FILE: &str = "crates/maestro-test-conventions/src/bucket_modules.rs";

/// Reject bucket-named modules outside the exemption table.
///
/// The rows apply in every tree, so a tree that holds part of the workspace keeps its names.
/// Only the workspace that holds the table reports a row that matches no module.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    check_with(members, EXEMPT, root.join(LIST_FILE).exists())
}

/// Run the check against a given table; a `complete` tree holds every row's module.
fn check_with(members: &[Member], exempt: &[Row], complete: bool) -> Result<(), String> {
    if let Some(pair) = exempt
        .windows(2)
        .find(|pair| (pair[0].0, pair[0].1) >= (pair[1].0, pair[1].1))
    {
        return Err(format!(
            "EXEMPT in bucket_modules.rs is sorted by member and module path, each row once: move the row for module `{}` of {}",
            pair[1].1, pair[1].0
        ));
    }
    let mut found = modules(members);
    let unused: Vec<String> = exempt
        .iter()
        .filter(|(member, module, _)| {
            found
                .remove(&((*member).to_owned(), (*module).to_owned()))
                .is_none()
                && complete
        })
        .map(|(member, module, reason)| {
            format!(
                "exemption for module `{module}` of {member} ({}) matches no module; remove the row from EXEMPT in bucket_modules.rs",
                reason.text()
            )
        })
        .collect();
    let diagnostics: Vec<String> = found
        .into_iter()
        .map(|((member, module), place)| {
            format!(
                "{place}: module `{module}` of {member} is named for where code was put; name it for what it holds (modules that keep such a name: EXEMPT in bucket_modules.rs)"
            )
        })
        .chain(unused)
        .collect();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// Every bucket-named module below the members' `src/` directories, by member and module path.
///
/// Each module carries its place: the file and line of its declaration, or the path of its
/// file or directory when no parsed source declares it.
fn modules(members: &[Member]) -> BTreeMap<(String, String), String> {
    let mut found = BTreeMap::new();
    for member in members {
        let root = member.directory.join("src");
        for source in &member.sources {
            let Ok(relative) = source.path.strip_prefix(&root) else {
                continue;
            };
            let path = module_path(relative);
            record_path_modules(&mut found, &member.name, &path, relative, &root);
            for (line, inline) in source.walk.iter().flat_map(|walk| &walk.bucket_modules) {
                let module = path.iter().map(String::as_str).chain([inline.as_str()]);
                found.insert(
                    (member.name.clone(), module.collect::<Vec<_>>().join("::")),
                    format!("{}:{line}", source.path.display()),
                );
            }
        }
    }
    found
}

/// Record each bucket-named module along one source's path, keeping the place found first.
fn record_path_modules(
    found: &mut BTreeMap<(String, String), String>,
    member: &str,
    path: &[String],
    relative: &Path,
    root: &Path,
) {
    for (depth, name) in path.iter().enumerate() {
        if BUCKETS.contains(&name.as_str()) {
            let place: PathBuf = relative.iter().take(depth + 1).collect();
            found
                .entry((member.to_owned(), path[..=depth].join("::")))
                .or_insert_with(|| root.join(place).display().to_string());
        }
    }
}

/// The module path a source's place below `src/` gives it: its directories, then its own name
/// unless it is a directory's `mod.rs` or a crate root.
fn module_path(relative: &Path) -> Vec<String> {
    let mut path: Vec<String> = relative
        .with_extension("")
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let root = matches!(path.as_slice(), [name] if name == "lib" || name == "main");
    if root || path.last().is_some_and(|name| name == "mod") {
        path.pop();
    }
    path
}

/// Bucket-named module declarations of one file, gathered by the shared syntax walk.
#[derive(Default)]
pub(crate) struct Declarations {
    /// Names of the modules around the current syntax, outermost first.
    around: Vec<String>,
    /// Each bucket-named declaration's line and its module path below the file's own module.
    pub(crate) found: Vec<(usize, String)>,
}

impl Declarations {
    /// Enter a module declaration, recording it when it carries a bucket name.
    pub(crate) fn enter(&mut self, name: &proc_macro2::Ident) {
        self.around.push(name.to_string());
        if BUCKETS.iter().any(|bucket| name == bucket) {
            self.found
                .push((name.span().start().line, self.around.join("::")));
        }
    }

    /// Leave the module declaration entered last.
    pub(crate) fn leave(&mut self) {
        self.around.pop();
    }
}

#[cfg(test)]
mod tests;
