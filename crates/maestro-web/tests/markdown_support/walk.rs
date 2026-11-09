//! The recorded Markdown fixture walk, shared by every Markdown owner and run without eval.
use super::BlockRow;
use std::any::Any;

/// A Markdown owner that shows one document until the returned value is dropped.
pub trait Mounter {
    /// Show `content` as the page's only `.maestro-markdown` document, with its escaping and
    /// reasoning options.
    fn mount(&mut self, content: &str, options: (bool, bool)) -> Box<dyn Any>;
}

/// Install the page projections the authored checks read.
pub fn install() {
    super::projection::install();
}
/// The shown document root.
///
/// # Panics
/// Panics if no document is shown.
#[must_use]
pub fn root() -> web_sys::Element {
    super::projection::root().expect("a Markdown document is shown")
}
/// First difference between the shown document and reference markup, or "".
///
/// # Panics
/// Panics if no document is shown.
#[must_use]
pub fn difference(html: &str) -> String {
    super::projection::difference(&root(), html)
}
/// First difference between the shown structure and a recorded tree, or "".
///
/// # Panics
/// Panics if no document is shown.
#[must_use]
pub fn tree_difference(tree: &serde_json::Value) -> String {
    super::projection::tree_difference(tree)
}
/// Whether no document remains shown.
fn cleared() -> bool {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .query_selector(".maestro-markdown")
        .unwrap()
        .is_none()
}

/// The markup a row must produce, given the recorded library observations.
fn target(row: &BlockRow) -> &str {
    match (&row.current_html, &row.original_html) {
        (Some(current), _) => {
            assert_ne!(
                current, &row.html,
                "{}: the tracked gap must still differ",
                row.id
            );
            current
        }
        (None, Some(original)) => {
            assert_ne!(
                original, &row.html,
                "{}: the corrected row must differ",
                row.id
            );
            &row.html
        }
        (None, None) => &row.html,
    }
}
/// Mount every row and require its recorded markup.
///
/// # Panics
/// Panics if a row differs or its document stays shown after release.
pub fn rows_match(mounter: &mut dyn Mounter, rows: Vec<super::BlockRow>) {
    for row in rows {
        let block = mounter.mount(&row.content, (row.escape_html, row.thinking));
        assert_eq!(difference(target(&row)), "", "row {}", row.id);
        drop(block);
        assert!(cleared(), "row {} stayed shown", row.id);
    }
}
/// Mount every contextual row and require its recorded structure.
///
/// # Panics
/// Panics if a row differs.
pub fn contexts_match(mounter: &mut dyn Mounter, rows: Vec<super::ContextRow>) {
    for row in rows {
        let block = mounter.mount(&row.source, (true, false));
        assert_eq!(tree_difference(&row.tree), "", "row {}", row.id);
        drop(block);
    }
}

/// Recorded caller identities and their independent markup or semantic-tree targets.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CallerReplay {
    /// Stable replay identity, including repeated inputs under different operations.
    id: String,
    /// Original authored source.
    content: String,
    /// Per-render authored HTML policy.
    escape_html: bool,
    /// Per-render text presentation.
    thinking: bool,
    /// Unmodified caller observation, retained independently of corrected targets.
    original_reference: String,
    /// Authority for a corrected observable result.
    ruling: Option<String>,
    /// Independent source of the selected target.
    target_source: String,
    /// Recorded document markup when a complete tree is not needed.
    html: Option<String>,
    /// Recorded semantic math/container tree.
    tree: Option<serde_json::Value>,
}

/// Replay every recorded caller identity without stopping at its first mismatch.
///
/// # Panics
/// Panics with every mismatch once all identities ran.
pub fn caller_replay_matches(mounter: &mut dyn Mounter) {
    let rows: Vec<CallerReplay> =
        serde_json::from_str(include_str!("../fixtures/markdown/caller_replay.json")).unwrap();
    assert_eq!(rows.len(), 447);
    let mut failures = Vec::new();
    let (mut exact, mut corrected, mut unresolved) = (0, 0, 0);
    let mut dom_bytes_exact = 0;
    for row in rows {
        let block = mounter.mount(&row.content, (row.escape_html, row.thinking));
        let found = match row.tree {
            Some(tree) => tree_difference(&tree),
            None => difference(row.html.as_deref().unwrap()),
        };
        if !found.is_empty() {
            failures.push(format!("{} ({}): {found}", row.id, row.target_source));
        }
        dom_bytes_exact += usize::from(root().inner_html() == row.original_reference);
        let raw_difference = difference(&row.original_reference);
        if raw_difference.is_empty() {
            exact += 1;
        } else if row.ruling.is_some() {
            corrected += 1;
        } else {
            unresolved += 1;
            failures.push(format!(
                "{}: unexplained raw observation: {raw_difference}",
                row.id
            ));
        }
        drop(block);
    }
    web_sys::console::log_1(
        &format!(
            "caller replay: exact={exact}, corrected={corrected}, unresolved={unresolved}, dom_bytes_exact={dom_bytes_exact}"
        )
        .into(),
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
