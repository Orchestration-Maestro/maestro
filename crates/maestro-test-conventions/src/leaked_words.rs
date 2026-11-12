//! Library and product words of other projects in public text.
//!
//! Every Maestro name and word says what the code does. The check reads each Rust
//! identifier, string literal, doc comment and other comment in the members' sources, each
//! Markdown page that version control tracks under `docs/`, and each tracked file and
//! directory name under `crates/` and `docs/`. A text splits into words at
//! non-alphanumeric characters and case changes. `DENIED` holds the words that are
//! generic library and product words, readable; a denied word matches when its segments
//! appear next to each other, ignoring case. `HASHED` holds the 64-bit FNV-1a hash of
//! the lowercase form of words that are another project's own name, so the name never
//! appears in this repository; a word matches when its hash does. `EXEMPT` holds the
//! places a user, a provider, a stored file or a detected program carries the word.

use crate::source::{self, Member, Source};
use crate::tracked_files;
use ra_ap_rustc_lexer::{LiteralKind, TokenKind};
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::Path;
use words::{Found, find};

/// Hashing, splitting and matching of words.
mod words;

/// Words that belong to a library or product Maestro was written from.
///
/// A word is one or more segments joined by `_`. Edit this list to deny another generic
/// library or product word; a word that is another project's own name goes in `HASHED`.
const DENIED: &[&str] = &["ssf", "docx_preview", "minimatch", "plugin", "libghostty"];

/// Hashes of words that are another project's own name.
///
/// Each entry is the 64-bit FNV-1a hash of one lowercase word, so the name itself never
/// appears in the repository. Edit this list to deny another name.
const HASHED: &[u64] = &[
    0x907e_d0a4_e178_860b,
    0x1d45_8ada_694e_23ce,
    0xd5b8_e962_9a22_6645,
];

/// A place that may keep a denied word because a user, a provider, a stored file or a detected program carries it.
#[derive(Clone, Copy)]
struct Exemption {
    /// Workspace-relative path of the file or directory, or of the source file or page holding the word.
    path: &'static str,
    /// The `DENIED` word the place may keep, or the hash of a hashed word as `0x` and sixteen hex digits.
    word: &'static str,
    /// Who carries the word and where it is stored.
    reason: &'static str,
}

/// Exempt places, each with its reason. A row that matches nothing fails the check.
///
/// A hashed word is named by its hash as `0x` and sixteen hex digits.
const EXEMPT: &[Exemption] = &[
    Exemption {
        path: "crates/maestro-models-catalog/src/provider_github-copilot.rs",
        word: "plugin",
        reason: "GitHub Copilot's request header `Editor-Plugin-Version`, emitted by the catalog generator",
    },
    Exemption {
        path: "crates/maestro-models/src/catalog_generation/generate_models/routes.rs",
        word: "plugin",
        reason: "GitHub Copilot's request header `Editor-Plugin-Version`",
    },
    Exemption {
        path: "crates/maestro-models/src/oauth/device/browser/token.rs",
        word: "plugin",
        reason: "GitHub Copilot's request header `Editor-Plugin-Version`",
    },
    Exemption {
        path: "crates/maestro-models/tests/device_account_authorization.rs",
        word: "plugin",
        reason: "GitHub Copilot's request header `Editor-Plugin-Version`, asserted on the wire",
    },
    Exemption {
        path: "crates/maestro-models/tests/support/response_qualification/invocation.rs",
        word: "plugin",
        reason: "GitHub Copilot's request header `Editor-Plugin-Version`, recorded in qualification",
    },
    Exemption {
        path: "crates/maestro-detection/src/agents/process.rs",
        word: "0xd5b8e9629a226645",
        reason: "the package scope that a detected program's command line carries",
    },
    Exemption {
        path: "crates/maestro-detection/src/agents/process/tests.rs",
        word: "0xd5b8e9629a226645",
        reason: "the package scope in the command lines of the detection fixtures",
    },
    Exemption {
        path: "crates/maestro-daemon/src/settings/tests/io.rs",
        word: "plugin",
        reason: "temporary: a settings key in a daemon test that the daemon lane rewrites",
    },
];

/// The file that holds the deny list, so it holds each denied word readable.
const LIST_FILE: &str = "crates/maestro-test-conventions/src/leaked_words.rs";

/// Every exemption whose path exists under the root: the table, and one row per readable denied
/// word in the list file.
///
/// A tree without a row's path is not the tree the row describes, so the row stays out of its report.
fn exemptions(root: &Path) -> Vec<Exemption> {
    let list = DENIED.iter().map(|word| Exemption {
        path: LIST_FILE,
        word,
        reason: "the deny list holds its words readable",
    });
    list.chain(EXEMPT.iter().copied())
        .filter(|row| root.join(row.path).exists())
        .collect()
}

/// The word lists and exemptions a run reads.
struct Lists<'a> {
    /// Readable words, as in `DENIED`.
    denied: &'a [&'a str],
    /// Word hashes, as in `HASHED`.
    hashed: &'a [u64],
    /// Exempt places, as in `EXEMPT`.
    exempt: &'a [Exemption],
}

/// Directories whose tracked file and directory names are checked.
const NAME_ROOTS: &[&str] = &["crates/", "docs/"];

/// Where a hit was found.
struct Where {
    /// An absolute source path, or a workspace-relative tracked path.
    location: String,
    /// Workspace-relative path used to match an exemption.
    relative: String,
    /// What holds the word: an identifier, a string literal, a comment, a page or a name.
    kind: &'static str,
}

/// One denied word in public text.
struct Hit {
    /// Where the word is.
    at: Where,
    /// The line of the word; absent for a file or directory name.
    line: Option<usize>,
    /// The identifier or the file or directory name holding the word; absent for text.
    name: Option<String>,
    /// The word, as the text spells it for a hashed word.
    word: String,
    /// What an exemption row names for the word.
    key: String,
}

/// Reject another project's names and words in identifiers, string literals, comments, pages and tracked names.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    let exempt = exemptions(root);
    let lists = Lists {
        denied: DENIED,
        hashed: HASHED,
        exempt: &exempt,
    };
    check_with(root, members, &lists)
}

/// Run the check against given lists.
fn check_with(root: &Path, members: &[Member], lists: &Lists<'_>) -> Result<(), String> {
    report(&hits(root, members, lists)?, lists.exempt)
}

/// Every denied word in the members' sources, the tracked pages and the tracked names.
fn hits(root: &Path, members: &[Member], lists: &Lists<'_>) -> Result<Vec<Hit>, String> {
    let tracked = tracked_files::paths(root)?;
    let mut hits = source_hits(root, members, lists);
    hits.extend(page_hits(root, &tracked, lists)?);
    hits.extend(name_hits(root, &tracked, lists));
    Ok(hits)
}

/// The diagnostics for the hits no exemption covers, and for each exemption that covers none.
fn report(hits: &[Hit], exempt: &[Exemption]) -> Result<(), String> {
    let mut used = vec![false; exempt.len()];
    let mut diagnostics: Vec<String> = hits
        .iter()
        .filter(|hit| {
            let row = exempt
                .iter()
                .position(|row| row.path == hit.at.relative && row.word == hit.key);
            if let Some(index) = row {
                used[index] = true;
            }
            row.is_none()
        })
        .map(describe)
        .collect();
    diagnostics.sort();
    diagnostics.extend(
        exempt
            .iter()
            .zip(&used)
            .filter(|(_, used)| !**used)
            .map(|(row, _)| {
                format!(
                    "exemption for `{}` and `{}` ({}) matches no word; remove the row from EXEMPT",
                    row.path, row.word, row.reason
                )
            }),
    );
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// The diagnostic for one hit.
fn describe(hit: &Hit) -> String {
    let place = hit.line.map_or_else(
        || hit.at.location.clone(),
        |line| format!("{}:{line}", hit.at.location),
    );
    let subject = hit.name.as_ref().map_or_else(
        || hit.at.kind.to_owned(),
        |name| format!("{} `{name}`", hit.at.kind),
    );
    format!(
        "{place}: {subject} holds the word `{}`; say it in Maestro's own words (words: DENIED and HASHED in leaked_words.rs)",
        hit.word
    )
}

/// A hit for a found word.
fn hit(at: &Where, line: Option<usize>, name: Option<&str>, found: Found) -> Hit {
    Hit {
        at: Where {
            location: at.location.clone(),
            relative: at.relative.clone(),
            kind: at.kind,
        },
        line,
        name: name.map(str::to_owned),
        word: found.word,
        key: found.key,
    }
}

/// How a token reads as public text: what it is called, or none for a token that holds no text.
fn text_kind(kind: TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::LineComment { doc_style } | TokenKind::BlockComment { doc_style, .. } => {
            Some(if doc_style.is_some() {
                "doc comment"
            } else {
                "comment"
            })
        }
        TokenKind::Literal {
            kind:
                LiteralKind::Str { .. }
                | LiteralKind::ByteStr { .. }
                | LiteralKind::CStr { .. }
                | LiteralKind::RawStr { .. }
                | LiteralKind::RawByteStr { .. }
                | LiteralKind::RawCStr { .. },
            ..
        } => Some("string literal"),
        _ => None,
    }
}

/// A string literal's text with each escaped line break, tab, quote and backslash blanked.
///
/// The text keeps its length, so offsets in it are offsets in the literal.
fn blank_escapes(text: &str) -> String {
    let mut blanked = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|next| "nrt0\\'\"".contains(*next)) {
            chars.next();
            blanked.push_str("  ");
        } else {
            blanked.push(c);
        }
    }
    blanked
}

/// Every denied word in the members' identifiers, string literals and comments.
fn source_hits(root: &Path, members: &[Member], lists: &Lists<'_>) -> Vec<Hit> {
    let mut hits = Vec::new();
    for source in members.iter().flat_map(|member| &member.sources) {
        let relative = relative_path(root, &source.path);
        let location = source.path.display().to_string();
        let place = |kind| Where {
            location: location.clone(),
            relative: relative.clone(),
            kind,
        };
        let mut known: HashMap<&str, Vec<Found>> = HashMap::new();
        for (kind, range) in &source.tokens {
            let text = &source.contents[range.clone()];
            if matches!(kind, TokenKind::Ident | TokenKind::RawIdent) {
                let name = text.strip_prefix("r#").unwrap_or(text);
                let found = known
                    .entry(name)
                    .or_insert_with(|| find(name, lists.denied, lists.hashed));
                hits.extend(identifier_hits(
                    &place("identifier"),
                    source,
                    range.start,
                    name,
                    found,
                ));
            } else if let Some(label) = text_kind(*kind) {
                hits.extend(text_hits(&place(label), source, range, lists));
            }
        }
    }
    hits
}

/// The hits for the denied words found in one identifier.
fn identifier_hits(
    at: &Where,
    source: &Source,
    start: usize,
    name: &str,
    found: &[Found],
) -> Vec<Hit> {
    if found.is_empty() {
        return Vec::new();
    }
    let line = source::line(&source.contents, start);
    found
        .iter()
        .map(|found| hit(at, Some(line), Some(name), found.clone()))
        .collect()
}

/// The hits for the denied words in the string literal or comment at a token range.
fn text_hits(at: &Where, source: &Source, range: &Range<usize>, lists: &Lists<'_>) -> Vec<Hit> {
    let text = blank_escapes(&source.contents[range.clone()]);
    find(&text, lists.denied, lists.hashed)
        .into_iter()
        .map(|found| {
            let line = source::line(&source.contents, range.start + found.offset);
            hit(at, Some(line), None, found)
        })
        .collect()
}

/// A path relative to the workspace root, with forward slashes.
fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every denied word in the Markdown pages that version control tracks under `docs/`.
fn page_hits(root: &Path, tracked: &[String], lists: &Lists<'_>) -> Result<Vec<Hit>, String> {
    let mut hits = Vec::new();
    for path in tracked {
        if !path.starts_with("docs/")
            || !Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            || std::fs::symlink_metadata(root.join(path)).is_err()
        {
            continue;
        }
        let page =
            std::fs::read_to_string(root.join(path)).map_err(|error| format!("{path}: {error}"))?;
        let at = Where {
            location: path.clone(),
            relative: path.clone(),
            kind: "documentation page",
        };
        for found in find(&page, lists.denied, lists.hashed) {
            let line = source::line(&page, found.offset);
            hits.push(hit(&at, Some(line), None, found));
        }
    }
    Ok(hits)
}

/// Every tracked file and directory name under the name roots that holds a denied word.
fn name_hits(root: &Path, tracked: &[String], lists: &Lists<'_>) -> Vec<Hit> {
    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    for path in tracked {
        if !NAME_ROOTS.iter().any(|prefix| path.starts_with(prefix))
            || std::fs::symlink_metadata(root.join(path)).is_err()
        {
            continue;
        }
        let mut prefix = String::new();
        for component in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if !seen.insert(prefix.clone()) {
                continue;
            }
            let at = Where {
                location: prefix.clone(),
                relative: prefix.clone(),
                kind: "file or directory name",
            };
            for found in find(component, lists.denied, lists.hashed) {
                hits.push(hit(&at, None, Some(component), found));
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests;
