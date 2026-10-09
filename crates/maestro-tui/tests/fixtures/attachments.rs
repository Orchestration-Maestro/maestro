//! Typed attachment inputs and complete public observations.
#![cfg(test)]
use maestro_tui::autocomplete::{
    AutocompleteOperations, CompletionOptions, CursorPosition, DirectoryEntry,
};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, CombinedAutocompleteProvider,
};
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::future::Future;
use std::io;
use std::pin::Pin;

/// A single controlled provider request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Assertion identity.
    pub id: String,
    /// Owning behavior test.
    pub group: String,
    /// Input text.
    pub text: String,
    /// Selected UTF-8 byte offset.
    pub col: usize,
    /// Authored base.
    pub base: String,
    /// Optional executable.
    pub executable: Option<String>,
    /// Forced request.
    #[serde(default)]
    pub force: Option<bool>,
    /// Cancellation boundary.
    #[serde(default)]
    pub abort: String,
    /// Home value.
    #[serde(default)]
    pub home: String,
    /// Home failure.
    #[serde(default)]
    pub home_error: bool,
    /// Directory metadata.
    #[serde(default)]
    pub stat: BTreeMap<String, serde_json::Value>,
    /// Collected process chunks.
    #[serde(default)]
    pub chunks: Vec<Chunk>,
    /// Process failure.
    #[serde(default)]
    pub error: bool,
    /// Expected complete suggestion record.
    pub result: Option<Suggestions>,
    /// Expected effect operands.
    pub calls: Vec<Vec<String>>,
    /// Expected composed applications.
    #[serde(default)]
    pub applications: Vec<Application>,
    /// Target-specific path choices.
    #[serde(default)]
    pub windows: bool,
}
/// Raw process chunks, preserving malformed byte sequences where needed.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Chunk {
    /// A complete UTF-8 chunk.
    Text(String),
    /// Bytes which may split or fail UTF-8 decoding.
    Bytes(Vec<u8>),
}
impl Chunk {
    /// Borrow bytes without decoding before collection.
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }
}
/// Public suggestion fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestions {
    /// Replacement prefix.
    prefix: String,
    /// Complete candidate records.
    items: Vec<Item>,
}
/// Candidate fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    /// Inserted value.
    value: String,
    /// Display label.
    label: String,
    /// Path description.
    description: Option<String>,
}
impl Suggestions {
    /// Project fixture fields into public records.
    #[must_use]
    pub fn public(&self) -> AutocompleteSuggestions {
        AutocompleteSuggestions {
            prefix: self.prefix.clone(),
            items: self
                .items
                .iter()
                .map(|i| AutocompleteItem {
                    value: i.value.clone(),
                    label: i.label.clone(),
                    description: i.description.clone(),
                })
                .collect(),
        }
    }
}
/// Expected insertion output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Application {
    /// Updated lines.
    pub lines: Vec<String>,
    /// Updated cursor line.
    pub cursor_line: usize,
    /// Updated cursor byte offset.
    pub cursor_col: usize,
}
/// Controlled effects with per-request observations.
pub struct Effects<'a> {
    /// Request input.
    pub case: &'a Case,
    /// Request cancellation shared with the provider.
    pub signal: &'a Cell<bool>,
    /// Effect operands.
    pub calls: std::rc::Rc<RefCell<Vec<Vec<String>>>>,
}
impl AutocompleteOperations for Effects<'_> {
    type Signal = Cell<bool>;
    fn is_aborted(&self, signal: &Self::Signal) -> bool {
        signal.get()
    }
    fn run_fd<'a>(
        &'a self,
        executable: &'a str,
        args: &'a [String],
        signal: &'a Self::Signal,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'a>> {
        if signal.get() {
            return Box::pin(async { Err(io::Error::other("cancelled")) });
        }
        let mut call = vec!["spawn".into(), executable.into()];
        call.extend_from_slice(args);
        self.calls.borrow_mut().push(call);
        Box::pin(async move {
            if matches!(self.case.abort.as_str(), "during" | "after_exit" | "home") {
                signal.set(true);
            }
            if self.case.error {
                Err(io::Error::other("process failure"))
            } else {
                Ok(self
                    .case
                    .chunks
                    .iter()
                    .flat_map(Chunk::bytes)
                    .copied()
                    .collect())
            }
        })
    }
    fn home_dir(&self) -> io::Result<String> {
        self.calls.borrow_mut().push(vec!["home".into()]);
        if self.case.abort == "home" {
            self.signal.set(true);
        }
        if self.case.home_error {
            Err(io::Error::other("home failure"))
        } else {
            Ok(self.case.home.clone())
        }
    }
    fn read_dir(&self, path: &str) -> io::Result<Vec<DirectoryEntry>> {
        self.calls
            .borrow_mut()
            .push(vec!["read_dir".into(), path.into()]);
        Ok(vec![])
    }
    fn is_directory(&self, path: &str) -> io::Result<bool> {
        self.calls
            .borrow_mut()
            .push(vec!["stat".into(), path.into()]);
        match self
            .case
            .stat
            .get(path)
            .and_then(serde_json::Value::as_bool)
        {
            Some(v) => Ok(v),
            None => Err(io::Error::other("stat failure")),
        }
    }
    fn compare(&self, left: &str, right: &str) -> io::Result<Ordering> {
        Ok(left.cmp(right))
    }
}
/// Run the corpus subset owned by one test.
///
/// # Panics
/// Panics on invalid fixture data or an observation differing from its expected result.
pub fn run_cases(group: &str) {
    let cases: Vec<Case> = serde_json::from_str(include_str!("attachments.json")).unwrap();
    for case in cases
        .iter()
        .filter(|c| c.group == group && c.windows == cfg!(windows))
    {
        run_case(case);
    }
}
/// Observe one request and all its selected insertion outputs.
fn run_case(case: &Case) {
    let signal = Cell::new(case.abort == "before");
    let effects = Effects {
        case,
        signal: &signal,
        calls: std::rc::Rc::new(RefCell::new(vec![])),
    };
    let calls = std::rc::Rc::clone(&effects.calls);
    let provider = CombinedAutocompleteProvider::new(
        vec![],
        case.base.clone(),
        case.executable.clone(),
        effects,
    );
    let lines = [case.text.clone()];
    let cursor = CursorPosition {
        line: 0,
        col: case.col,
    };
    let result = super::futures::block_on(provider.get_suggestions(
        &lines,
        cursor,
        CompletionOptions {
            signal: &signal,
            force: case.force,
        },
    ))
    .unwrap();
    assert_eq!(*calls.borrow(), case.calls, "{} operands", case.id);
    assert_eq!(
        result,
        case.result.as_ref().map(Suggestions::public),
        "{}",
        case.id
    );
    if let Some(result) = result {
        if !case.applications.is_empty() {
            assert_eq!(result.items.len(), case.applications.len());
        }
        for (item, expected) in result.items.iter().zip(&case.applications) {
            let applied = provider.apply_completion(&lines, cursor, item, &result.prefix);
            assert_eq!(applied.lines, expected.lines, "{}", case.id);
            assert_eq!(
                (applied.cursor_line, applied.cursor_col),
                (expected.cursor_line, expected.cursor_col),
                "{}",
                case.id
            );
        }
    }
}
