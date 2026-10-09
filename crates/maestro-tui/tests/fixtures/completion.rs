//! Controlled operations and command callbacks for completion observations.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::io;
use std::rc::Rc;

use maestro_tui::autocomplete::CursorPosition;
use maestro_tui::autocomplete::NativeAutocompleteOperations;
use maestro_tui::autocomplete::{
    ArgumentCompletions, AutocompleteOperations, Command, DirectoryEntry, DirectoryEntryKind,
};
use maestro_tui::{AutocompleteItem, SlashCommand};

/// Call trace shared by the operations and callbacks.
pub type Trace = Rc<RefCell<Vec<Vec<String>>>>;

/// Controlled filesystem inputs for one request.
#[derive(Default)]
pub struct Files {
    /// Directory entries in supplied order.
    pub entries: Vec<(&'static str, DirectoryEntryKind)>,
    /// Resolved symbolic-link metadata.
    pub stat: Vec<(&'static str, &'static str)>,
    /// Whether directory enumeration fails.
    pub read_error: bool,
    /// Whether home lookup fails.
    pub home_error: bool,
    /// Whether label comparison fails.
    pub compare_error: bool,
    /// Calls observed at the effect boundary.
    pub trace: Trace,
}

impl AutocompleteOperations for Files {
    type Signal = Cell<bool>;
    fn home_dir(&self) -> io::Result<String> {
        self.trace.borrow_mut().push(vec!["home".into()]);
        if self.home_error {
            Err(io::Error::other("home failure"))
        } else {
            Ok("/home/test".into())
        }
    }
    fn read_dir(&self, path: &str) -> io::Result<Vec<DirectoryEntry>> {
        self.trace
            .borrow_mut()
            .push(vec!["read_dir".into(), path.into()]);
        if self.read_error {
            return Err(io::Error::other("read failure"));
        }
        Ok(self
            .entries
            .iter()
            .map(|(name, kind)| DirectoryEntry {
                name: (*name).into(),
                kind: *kind,
            })
            .collect())
    }
    fn is_directory(&self, path: &str) -> io::Result<bool> {
        self.trace
            .borrow_mut()
            .push(vec!["is_directory".into(), path.into()]);
        match self
            .stat
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, kind)| *kind)
        {
            Some("directory") => Ok(true),
            Some("error") => Err(io::Error::other("stat failure")),
            _ => Ok(false),
        }
    }
    fn compare(&self, left: &str, right: &str) -> io::Result<Ordering> {
        if self.compare_error {
            return Err(io::Error::other("compare failure"));
        }
        NativeAutocompleteOperations::with_environment(|_| Some("en_US".into()))
            .compare(left, right)
    }
}

/// Candidate constructor with distinct display and insertion fields.
pub fn item(value: &str, label: &str, description: Option<&str>) -> AutocompleteItem {
    AutocompleteItem {
        value: value.into(),
        label: label.into(),
        description: description.map(str::to_owned),
    }
}

/// Callback forms recorded at the public argument boundary.
pub fn callback(name: &str, kind: &str, trace: &Trace) -> ArgumentCompletions {
    let name = name.to_owned();
    let kind = kind.to_owned();
    let trace = Rc::clone(trace);
    Rc::new(move |prefix| {
        trace
            .borrow_mut()
            .push(vec!["arguments".into(), name.clone(), prefix.into()]);
        let kind = kind.clone();
        Box::pin(async move {
            match kind.as_str() {
                "echo" => Ok(Some(vec![item(
                    &format!("{prefix}!"),
                    "argument",
                    Some("returned unchanged"),
                )])),
                "empty" => Ok(Some(vec![])),
                "reject" => Err(io::Error::other("argument failure").into()),
                _ => Ok(None),
            }
        })
    })
}

/// Owned command record assembled from the supplied metadata.
pub fn slash(
    name: &str,
    description: Option<&str>,
    hint: Option<&str>,
    complete: Option<ArgumentCompletions>,
) -> Command {
    Command::SlashCommand(SlashCommand {
        name: name.into(),
        description: description.map(str::to_owned),
        argument_hint: hint.map(str::to_owned),
        get_argument_completions: complete,
    })
}

/// An expected provider result, including its callback error boundary.
pub enum Expected {
    /// No candidates.
    None,
    /// The exact prefix and all public candidate fields.
    Items(
        &'static str,
        &'static [(&'static str, &'static str, Option<&'static str>)],
    ),
    /// An unchanged callback failure.
    Error(&'static str),
}

/// A query and the controlled effects it reaches.
pub struct SuggestionCase {
    /// Observation name for assertion failures.
    pub id: &'static str,
    /// Current lines and selected byte cursor.
    pub lines: &'static [&'static str],
    /// Selected cursor line.
    pub line: usize,
    /// Selected byte offset.
    pub col: usize,
    /// Forced completion flag.
    pub force: Option<bool>,
    /// Initial caller signal state.
    pub aborted: bool,
    /// Whether to supply command metadata.
    pub commands: CommandInput,
    /// Controlled host inputs.
    pub files: FileInput,
    /// Entire expected public result.
    pub expected: Expected,
    /// Exact reached effects; comparison callback counts are not observations.
    pub calls: &'static [&'static [&'static str]],
}

/// Borrowed host input literals for a query.
#[derive(Default)]
pub struct FileInput {
    /// Directory entry input.
    pub entries: &'static [(&'static str, DirectoryEntryKind)],
    /// Symbolic-link metadata input.
    pub stat: &'static [(&'static str, &'static str)],
    /// Enumeration error.
    pub read_error: bool,
    /// Home error.
    pub home_error: bool,
    /// Comparison error.
    pub compare_error: bool,
}

/// Command metadata shared by the recorded queries.
pub fn commands(trace: &Trace) -> Vec<Command> {
    vec![
        slash("help", Some("Help"), Some("<topic>"), None),
        slash("hello", Some(""), None, None),
        Command::AutocompleteItem(item("clear", "Not the label", Some("Clear"))),
        slash("hint", None, Some("<arg>"), None),
        slash("blank", Some(""), Some(""), None),
        slash("cl", None, None, None),
        slash("load", None, None, Some(callback("load", "echo", trace))),
        slash("none", None, None, Some(callback("none", "none", trace))),
        slash("empty", None, None, Some(callback("empty", "empty", trace))),
        slash("invalid", None, None, None),
        slash("fail", None, None, Some(callback("fail", "reject", trace))),
        slash("dup", None, None, Some(callback("dup", "echo", trace))),
        slash("dup", None, None, Some(callback("dup", "none", trace))),
    ]
}

/// Observe a request's complete result and reached effects.
///
/// # Panics
/// Panics when the observed result or effects differ.
pub fn run_suggestions(cases: &[SuggestionCase]) {
    use maestro_tui::autocomplete::{CompletionOptions, CursorPosition};
    use maestro_tui::{
        AutocompleteProvider, AutocompleteSuggestions, CombinedAutocompleteProvider,
    };
    for case in cases {
        let files = Files {
            entries: case.files.entries.to_vec(),
            stat: case.files.stat.to_vec(),
            read_error: case.files.read_error,
            home_error: case.files.home_error,
            compare_error: case.files.compare_error,
            ..Files::default()
        };
        let trace = Rc::clone(&files.trace);
        let commands = match case.commands {
            CommandInput::Standard => commands(&trace),
            CommandInput::Empty => vec![],
            CommandInput::EmptyName => vec![slash("", None, None, None)],
        };
        let provider = CombinedAutocompleteProvider::new(commands, "/work".into(), files);
        let lines: Vec<_> = case.lines.iter().map(|s| (*s).to_owned()).collect();
        let signal = Cell::new(case.aborted);
        let result = super::futures::block_on(provider.get_suggestions(
            &lines,
            CursorPosition {
                line: case.line,
                col: case.col,
            },
            CompletionOptions {
                signal: &signal,
                force: case.force,
            },
        ));
        let expected = match &case.expected {
            Expected::Error(error) => Err((*error).to_owned()),
            Expected::None => Ok(None),
            Expected::Items(prefix, items) => Ok(Some(AutocompleteSuggestions {
                prefix: (*prefix).into(),
                items: items
                    .iter()
                    .map(|(value, label, description)| item(value, label, *description))
                    .collect(),
            })),
        };
        assert_eq!(
            result.map_err(|error| error.to_string()),
            expected,
            "{}",
            case.id
        );
        let expected: Vec<Vec<String>> = case
            .calls
            .iter()
            .map(|call| call.iter().map(|s| (*s).into()).collect())
            .collect();
        assert_eq!(*trace.borrow(), expected, "{}", case.id);
    }
}

/// Command collections used by the source queries.
pub enum CommandInput {
    /// Full supplied catalog.
    Standard,
    /// No commands.
    Empty,
    /// A command with an empty name.
    EmptyName,
}

/// An insertion query with all of its observed output fields.
pub type ApplicationCase = (
    &'static str,
    &'static [&'static str],
    CursorPosition,
    &'static str,
    (&'static str, &'static str, Option<&'static str>),
    &'static [&'static str],
    usize,
    usize,
);

/// Observe insertion text and byte cursor through the public provider.
///
/// # Panics
/// Panics when the inserted text or cursor differs.
pub fn run_applications(cases: &[ApplicationCase]) {
    use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider};
    for (id, input, cursor, prefix, chosen, expected, line, col) in cases.iter().copied() {
        let provider = CombinedAutocompleteProvider::new(vec![], "/work".into(), Files::default());
        let input: Vec<_> = input.iter().map(|s| (*s).to_owned()).collect();
        let applied =
            provider.apply_completion(&input, cursor, &item(chosen.0, chosen.1, chosen.2), prefix);
        assert_eq!(applied.lines, expected, "{id}");
        assert_eq!(
            (applied.cursor_line, applied.cursor_col),
            (line, col),
            "{id}"
        );
    }
}
