//! Prompt parsing, ordered loading and expansion through public interfaces.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_resources::{
    LoadPromptTemplatesOptions, PromptTemplate, ResourceEntry, ResourceFileType,
    ResourceOperations, SourceOrigin, SourceScope, SyntheticSourceOptions,
    create_synthetic_source_info, expand_prompt_template, load_prompt_templates,
    parse_command_args, substitute_args,
};
#[cfg(test)]
pub mod support;
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
};

/// Controlled entry observations; text exists only for files read by the test.
enum Node<'a> {
    /// Regular file with optional readable text.
    File(Option<&'a str>),
    /// Directory with its returned entry names.
    Dir(&'a [&'a str]),
    /// Authored symlink target.
    Link(&'a str),
    /// Another filesystem kind.
    Other,
}
impl Node<'_> {
    /// Directory-entry kind before following a link.
    fn kind(&self) -> ResourceFileType {
        match self {
            Self::File(_) => ResourceFileType::File,
            Self::Dir(_) => ResourceFileType::Directory,
            Self::Link(_) => ResourceFileType::Symlink,
            Self::Other => ResourceFileType::Other,
        }
    }
}

/// Synchronous ordered observations with operation-specific failures.
struct Observations<'a> {
    /// Named entries supplied by the test.
    entries: &'a [(&'a str, Node<'a>)],
    /// Failures, optionally at a specific invocation number.
    failures: &'a [(&'a str, &'a str, usize)],
    /// Replacement metadata observations in invocation order.
    kinds: &'a [(&'a str, &'a [ResourceFileType])],
    /// Process directory observation.
    cwd: Option<&'a str>,
    /// Completed synchronous calls.
    calls: RefCell<Vec<(String, String)>>,
}
impl<'a> Observations<'a> {
    /// Construct an adapter with no failures and an anchored process directory.
    fn new(entries: &'a [(&'a str, Node<'a>)]) -> Self {
        Self {
            entries,
            failures: &[],
            kinds: &[],
            cwd: Some("/ambient"),
            calls: RefCell::new(Vec::new()),
        }
    }
    /// Find the authored node, without following links.
    fn node(&self, path: &str) -> io::Result<&Node<'a>> {
        let resolved = maestro_path::resolve(
            &[path],
            &maestro_path::Cwd {
                current: self.cwd.unwrap_or("/ambient"),
                drive_directories: &[],
            },
        );
        self.entries
            .iter()
            .find(|(p, _)| *p == resolved)
            .map(|(_, n)| n)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
    /// Record a call before deciding its injected failure.
    fn call(&self, op: &str, path: &str) -> io::Result<usize> {
        let mut calls = self.calls.borrow_mut();
        calls.push((op.into(), path.into()));
        let count = calls.iter().filter(|(o, p)| o == op && p == path).count();
        if self
            .failures
            .iter()
            .any(|(o, p, n)| *o == op && *p == path && (*n == 0 || *n == count))
        {
            Err(io::ErrorKind::PermissionDenied.into())
        } else {
            Ok(count)
        }
    }
    /// Follow a controlled link to its target.
    fn target(&self, path: &str) -> io::Result<&Node<'a>> {
        match self.node(path)? {
            Node::Link(target) => self.node(target),
            node => Ok(node),
        }
    }
}
impl ResourceOperations for Observations<'_> {
    fn exists(&self, path: &Path) -> bool {
        let path = support::text(path);
        self.call("exists", path).unwrap();
        self.target(path).is_ok()
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        let path = support::text(path);
        self.call("dir", path)?;
        let Node::Dir(names) = self.target(path)? else {
            return Err(io::ErrorKind::NotADirectory.into());
        };
        names
            .iter()
            .map(|name| {
                Ok(ResourceEntry {
                    name: (*name).into(),
                    file_type: self.node(&maestro_path::join(&[path, name]))?.kind(),
                })
            })
            .collect()
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let path = support::text(path);
        self.call("read", path)?;
        let Node::File(Some(text)) = self.target(path)? else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        Ok((*text).into())
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        let path = support::text(path);
        let count = self.call("stat", path)?;
        if let Some((_, kinds)) = self.kinds.iter().find(|(p, _)| *p == path)
            && let Some(kind) = kinds.get(count - 1)
        {
            return Ok(*kind);
        }
        Ok(self.target(path)?.kind())
    }
    fn canonicalize(&self, _: &Path) -> io::Result<PathBuf> {
        panic!("prompt loading must not canonicalize")
    }
    fn current_directory(&self) -> io::Result<String> {
        self.call("cwd", "")?;
        self.cwd
            .map(str::to_owned)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        Vec::new()
    }
}

/// Borrowed expected fields, all consumed by record assertions.
struct ExpectedTemplate<'a> {
    /// Template name.
    name: &'a str,
    /// Description text.
    description: &'a str,
    /// Optional nonempty hint.
    hint: Option<&'a str>,
    /// Retained body.
    content: &'a str,
    /// Authored file/source path.
    path: &'a str,
    /// Source scope.
    scope: SourceScope,
    /// Source base directory.
    base: &'a str,
}

/// Compare all output fields with borrowed expected records.
fn assert_records(actual: &[PromptTemplate], expected: &[ExpectedTemplate<'_>]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_record(actual, expected);
    }
}

/// Check one complete record without assuming a directory enumeration order.
fn assert_record(actual: &PromptTemplate, expected: &ExpectedTemplate<'_>) {
    assert_eq!(actual.name, expected.name);
    assert_eq!(actual.description, expected.description);
    assert_eq!(actual.argument_hint.as_deref(), expected.hint);
    assert_eq!(actual.content, expected.content);
    assert_eq!(actual.file_path, expected.path);
    assert_eq!(actual.source_info.path, expected.path);
    assert_eq!(actual.source_info.source, "local");
    assert_eq!(actual.source_info.scope, expected.scope);
    assert_eq!(actual.source_info.origin, SourceOrigin::TopLevel);
    assert_eq!(actual.source_info.base_dir.as_deref(), Some(expected.base));
}

/// One ordered controlled filesystem input and its expected loaded records.
struct LoadCase<'a> {
    /// Test observations.
    entries: &'a [(&'a str, Node<'a>)],
    /// Selected native-operation failures.
    failures: &'a [(&'a str, &'a str, usize)],
    /// Replacement metadata kinds.
    kinds: &'a [(&'a str, &'a [ResourceFileType])],
    /// Adapter process directory.
    process: &'a str,
    /// Caller-selected roots/paths.
    options: LoadPromptTemplatesOptions<'a>,
    /// Borrowed authored explicit paths converted at the public seam.
    paths: &'a [&'a str],
    /// Complete retained records.
    expected: &'a [ExpectedTemplate<'a>],
}

/// Run one controlled input and return the synchronous completed observations.
fn run_case(case: &LoadCase<'_>) -> Vec<(String, String)> {
    let mut operations = Observations::new(case.entries);
    operations.failures = case.failures;
    operations.kinds = case.kinds;
    operations.cwd = Some(case.process);
    let paths: Vec<String> = case.paths.iter().map(|path| (*path).into()).collect();
    let options = LoadPromptTemplatesOptions {
        prompt_paths: &paths,
        ..case.options
    };
    let actual = load_prompt_templates(options, &operations).unwrap();
    assert_records(&actual, case.expected);
    operations.calls.into_inner()
}

/// Create a template input with inert provenance.
fn template(name: &str, content: &str) -> PromptTemplate {
    PromptTemplate {
        name: name.into(),
        description: String::new(),
        argument_hint: None,
        content: content.into(),
        file_path: "/x".into(),
        source_info: create_synthetic_source_info(
            "/x".into(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
    }
}

/// Check every field of owned expected records in the ambient-directory test.
fn assert_templates(actual: &[PromptTemplate], expected: &[PromptTemplate]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.description, expected.description);
        assert_eq!(actual.argument_hint, expected.argument_hint);
        assert_eq!(actual.content, expected.content);
        assert_eq!(actual.file_path, expected.file_path);
        assert_eq!(actual.source_info.path, expected.source_info.path);
        assert_eq!(actual.source_info.source, expected.source_info.source);
        assert_eq!(actual.source_info.scope, expected.source_info.scope);
        assert_eq!(actual.source_info.origin, expected.source_info.origin);
        assert_eq!(actual.source_info.base_dir, expected.source_info.base_dir);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const WILDCARD_PLACEHOLDERS_JOIN_EVERY_ARGUMENT_CASES: &[(&str, &[&str], &str)] = &[
    ("Test: $ARGUMENTS", &["a", "b", "c"], "Test: a b c"),
    ("Test: $@", &["a", "b", "c"], "Test: a b c"),
    ("Test: $@", &["foo", "bar", "baz"], "Test: foo bar baz"),
    (
        "Test: $ARGUMENTS",
        &["foo", "bar", "baz"],
        "Test: foo bar baz",
    ),
    ("Test: $ARGUMENTS", &[], "Test: "),
    ("Test: $@", &[], "Test: "),
    ("Test: $1", &[], "Test: "),
    ("$ARGUMENTS and $ARGUMENTS", &["a", "b"], "a b and a b"),
    ("$@ and $@", &["a", "b"], "a b and a b"),
    ("$@ and $ARGUMENTS", &["a", "b"], "a b and a b"),
    ("$ARGUMENTS", &["日本語", "🎉", "café"], "日本語 🎉 café"),
    ("Test: $ARGUMENTS", &["only"], "Test: only"),
    ("Test: $@", &["only"], "Test: only"),
    ("pre$ARGUMENTS", &["a", "b"], "prea b"),
    ("pre$@", &["a", "b"], "prea b"),
    (
        "Prefix $ARGUMENTS suffix",
        &["ARGUMENTS"],
        "Prefix ARGUMENTS suffix",
    ),
    ("$@ and $ARGUMENTS", &["x", "y", "z"], "x y z and x y z"),
    ("$ARGUMENTS and $@", &["x", "y", "z"], "x y z and x y z"),
    (
        "$ARGUMENTS",
        &[
            "arg0", "arg1", "arg2", "arg3", "arg4", "arg5", "arg6", "arg7", "arg8", "arg9",
            "arg10", "arg11", "arg12", "arg13", "arg14", "arg15", "arg16", "arg17", "arg18",
            "arg19", "arg20", "arg21", "arg22", "arg23", "arg24", "arg25", "arg26", "arg27",
            "arg28", "arg29", "arg30", "arg31", "arg32", "arg33", "arg34", "arg35", "arg36",
            "arg37", "arg38", "arg39", "arg40", "arg41", "arg42", "arg43", "arg44", "arg45",
            "arg46", "arg47", "arg48", "arg49", "arg50", "arg51", "arg52", "arg53", "arg54",
            "arg55", "arg56", "arg57", "arg58", "arg59", "arg60", "arg61", "arg62", "arg63",
            "arg64", "arg65", "arg66", "arg67", "arg68", "arg69", "arg70", "arg71", "arg72",
            "arg73", "arg74", "arg75", "arg76", "arg77", "arg78", "arg79", "arg80", "arg81",
            "arg82", "arg83", "arg84", "arg85", "arg86", "arg87", "arg88", "arg89", "arg90",
            "arg91", "arg92", "arg93", "arg94", "arg95", "arg96", "arg97", "arg98", "arg99",
        ],
        "arg0 arg1 arg2 arg3 arg4 arg5 arg6 arg7 arg8 arg9 arg10 arg11 arg12 arg13 arg14 arg15 arg16 arg17 arg18 arg19 arg20 arg21 arg22 arg23 arg24 arg25 arg26 arg27 arg28 arg29 arg30 arg31 arg32 arg33 arg34 arg35 arg36 arg37 arg38 arg39 arg40 arg41 arg42 arg43 arg44 arg45 arg46 arg47 arg48 arg49 arg50 arg51 arg52 arg53 arg54 arg55 arg56 arg57 arg58 arg59 arg60 arg61 arg62 arg63 arg64 arg65 arg66 arg67 arg68 arg69 arg70 arg71 arg72 arg73 arg74 arg75 arg76 arg77 arg78 arg79 arg80 arg81 arg82 arg83 arg84 arg85 arg86 arg87 arg88 arg89 arg90 arg91 arg92 arg93 arg94 arg95 arg96 arg97 arg98 arg99",
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn wildcard_placeholders_join_every_argument() {
    for (text, arguments, expected) in WILDCARD_PLACEHOLDERS_JOIN_EVERY_ARGUMENT_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const MAESTRO_PROMPT_ARGUMENTS_ARE_INSERTED_LITERALLY_CASES: &[(&str, &[&str], &str)] = &[
    ("$ARGUMENTS", &["$1", "$ARGUMENTS"], "$1 $ARGUMENTS"),
    ("$@", &["$100", "$1"], "$100 $1"),
    ("$ARGUMENTS", &["$100", "$1"], "$100 $1"),
    ("$1", &["$&", "tail"], "$&"),
    ("${@:1:1}", &["$&", "tail"], "$&"),
    ("$@", &["$&", "tail"], "$& tail"),
    ("$ARGUMENTS", &["$&", "tail"], "$& tail"),
    ("$1", &["$$", "tail"], "$$"),
    ("${@:1:1}", &["$$", "tail"], "$$"),
    ("$@", &["$$", "tail"], "$$ tail"),
    ("$ARGUMENTS", &["$$", "tail"], "$$ tail"),
    ("$1", &["$`", "tail"], "$`"),
    ("${@:1:1}", &["$`", "tail"], "$`"),
    ("$@", &["$`", "tail"], "$` tail"),
    ("$ARGUMENTS", &["$`", "tail"], "$` tail"),
    ("$1", &["$'", "tail"], "$'"),
    ("${@:1:1}", &["$'", "tail"], "$'"),
    ("$@", &["$'", "tail"], "$' tail"),
    ("$ARGUMENTS", &["$'", "tail"], "$' tail"),
    ("$1", &["$1", "tail"], "$1"),
    ("${@:1:1}", &["$1", "tail"], "$1"),
    ("$@", &["$1", "tail"], "$1 tail"),
    ("$ARGUMENTS", &["$1", "tail"], "$1 tail"),
    ("$1", &["$100", "tail"], "$100"),
    ("${@:1:1}", &["$100", "tail"], "$100"),
    ("$@", &["$100", "tail"], "$100 tail"),
    ("$ARGUMENTS", &["$100", "tail"], "$100 tail"),
    ("$1", &["$@", "tail"], "$@"),
    ("${@:1:1}", &["$@", "tail"], "$@"),
    ("$@", &["$@", "tail"], "$@ tail"),
    ("$ARGUMENTS", &["$@", "tail"], "$@ tail"),
    ("$1", &["$ARGUMENTS", "tail"], "$ARGUMENTS"),
    ("${@:1:1}", &["$ARGUMENTS", "tail"], "$ARGUMENTS"),
    ("$@", &["$ARGUMENTS", "tail"], "$ARGUMENTS tail"),
    ("$ARGUMENTS", &["$ARGUMENTS", "tail"], "$ARGUMENTS tail"),
    ("$1", &["${@:2}", "tail"], "${@:2}"),
    ("${@:1:1}", &["${@:2}", "tail"], "${@:2}"),
    ("$@", &["${@:2}", "tail"], "${@:2} tail"),
    ("$ARGUMENTS", &["${@:2}", "tail"], "${@:2} tail"),
    ("$1", &["\\$2", "tail"], "\\$2"),
    ("${@:1:1}", &["\\$2", "tail"], "\\$2"),
    ("$@", &["\\$2", "tail"], "\\$2 tail"),
    ("$ARGUMENTS", &["\\$2", "tail"], "\\$2 tail"),
    ("$1", &["🎉\n\t", "tail"], "🎉\n\t"),
    ("${@:1:1}", &["🎉\n\t", "tail"], "🎉\n\t"),
    ("$@", &["🎉\n\t", "tail"], "🎉\n\t tail"),
    ("$ARGUMENTS", &["🎉\n\t", "tail"], "🎉\n\t tail"),
    (
        "$1|${@:2}|$ARGUMENTS|$@",
        &["${@:2}", "$@", "$ARGUMENTS"],
        "${@:2}|$@ $ARGUMENTS|${@:2} $@ $ARGUMENTS|${@:2} $@ $ARGUMENTS",
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn maestro_prompt_arguments_are_inserted_literally() {
    for (text, arguments, expected) in MAESTRO_PROMPT_ARGUMENTS_ARE_INSERTED_LITERALLY_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const ORIGINAL_PLACEHOLDERS_COMPOSE_WITHOUT_EXTRA_SPACING_CASES: &[(&str, &[&str], &str)] = &[
    (
        "$1: $ARGUMENTS",
        &["prefix", "a", "b"],
        "prefix: prefix a b",
    ),
    ("$1: $@", &["prefix", "a", "b"], "prefix: prefix a b"),
    (
        "$1 $2: $ARGUMENTS",
        &["arg100", "@user"],
        "arg100 @user: arg100 @user",
    ),
    ("$1$2", &["a", "b"], "ab"),
    (
        "$1: $@ ($ARGUMENTS)",
        &["first", "second", "third"],
        "first: first second third (first second third)",
    ),
    ("$1 $2 $@", &["a", "b", "c"], "a b a b c"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn original_placeholders_compose_without_extra_spacing() {
    for (text, arguments, expected) in ORIGINAL_PLACEHOLDERS_COMPOSE_WITHOUT_EXTRA_SPACING_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const POSITIONAL_PLACEHOLDERS_USE_FULL_DECIMAL_INDEXES_CASES: &[(&str, &[&str], &str)] = &[
    ("$1 $2 $3 $4 $5", &["a", "b"], "a b   "),
    ("$0", &["a", "b"], ""),
    ("$1.5", &["a"], "a.5"),
    ("$1 $2 $3", &["a", "b", "c"], "a b c"),
    (
        "$10 $12 $15",
        &[
            "val0", "val1", "val2", "val3", "val4", "val5", "val6", "val7", "val8", "val9",
            "val10", "val11", "val12", "val13", "val14",
        ],
        "val9 val11 val14",
    ),
    ("Price: \\$100", &[], "Price: \\"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn positional_placeholders_use_full_decimal_indexes() {
    for (text, arguments, expected) in POSITIONAL_PLACEHOLDERS_USE_FULL_DECIMAL_INDEXES_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const SUBSTITUTION_PRESERVES_ARGUMENT_WHITESPACE_CASES: &[(&str, &[&str], &str)] = &[
    (
        "$1 $2",
        &["line1\nline2", "tab\tthere"],
        "line1\nline2 tab\tthere",
    ),
    (
        "$ARGUMENTS",
        &["first arg", "second arg"],
        "first arg second arg",
    ),
    ("$ARGUMENTS", &["a", "", "c"], "a  c"),
    (
        "$ARGUMENTS",
        &["  leading  ", "trailing  "],
        "  leading   trailing  ",
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn substitution_preserves_argument_whitespace() {
    for (text, arguments, expected) in SUBSTITUTION_PRESERVES_ARGUMENT_WHITESPACE_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const UNMATCHED_TEMPLATE_TEXT_STAYS_LITERAL_CASES: &[(&str, &[&str], &str)] = &[
    ("$A $$ $ $ARGS", &["a"], "$A $$ $ $ARGS"),
    (
        "$arguments $Arguments $ARGUMENTS",
        &["a", "b"],
        "$arguments $Arguments a b",
    ),
    ("Just plain text", &["a", "b"], "Just plain text"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn unmatched_template_text_stays_literal() {
    for (text, arguments, expected) in UNMATCHED_TEMPLATE_TEXT_STAYS_LITERAL_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const SLICE_PLACEHOLDERS_OBEY_START_AND_LENGTH_CASES: &[(&str, &[&str], &str)] = &[
    ("${@:2}", &["a", "b", "c", "d"], "b c d"),
    ("${@:1}", &["a", "b", "c"], "a b c"),
    ("${@:3}", &["a", "b", "c", "d"], "c d"),
    ("${@:2:2}", &["a", "b", "c", "d"], "b c"),
    ("${@:1:1}", &["a", "b", "c"], "a"),
    ("${@:3:1}", &["a", "b", "c", "d"], "c"),
    ("${@:2:3}", &["a", "b", "c", "d", "e"], "b c d"),
    ("${@:99}", &["a", "b"], ""),
    ("${@:5}", &["a", "b"], ""),
    ("${@:10:5}", &["a", "b"], ""),
    ("${@:2:0}", &["a", "b", "c"], ""),
    ("${@:1:0}", &["a", "b"], ""),
    ("${@:2:99}", &["a", "b", "c"], "b c"),
    ("${@:1:10}", &["a", "b"], "a b"),
    ("${@:1}", &["${@:2}", "test"], "${@:2} test"),
    ("${@:2}", &["a", "${@:3}", "c"], "${@:3} c"),
    ("${@:0}", &["a", "b", "c"], "a b c"),
    ("${@:2}", &[], ""),
    ("${@:1}", &[], ""),
    ("${@:1}", &["only"], "only"),
    ("${@:2}", &["only"], ""),
    (
        "${@:2}",
        &["cmd", "first arg", "second arg"],
        "first arg second arg",
    ),
    (
        "${@:2}",
        &["cmd", "$100", "@user", "#tag"],
        "$100 @user #tag",
    ),
    ("${@:1}", &["日本語", "🎉", "café"], "日本語 🎉 café"),
    (
        "${@:5:100}",
        &[
            "arg1", "arg2", "arg3", "arg4", "arg5", "arg6", "arg7", "arg8", "arg9", "arg10",
        ],
        "arg5 arg6 arg7 arg8 arg9 arg10",
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn slice_placeholders_obey_start_and_length() {
    for (text, arguments, expected) in SLICE_PLACEHOLDERS_OBEY_START_AND_LENGTH_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const SLICE_PLACEHOLDERS_COMPOSE_WITH_ORIGINAL_TOKENS_CASES: &[(&str, &[&str], &str)] = &[
    ("${@:2} vs $@", &["a", "b", "c"], "b c vs a b c"),
    (
        "First: ${@:1:1}, All: $@",
        &["x", "y", "z"],
        "First: x, All: x y z",
    ),
    ("$1: ${@:2}", &["cmd", "arg1", "arg2"], "cmd: arg1 arg2"),
    ("$1 $2 ${@:3}", &["a", "b", "c", "d"], "a b c d"),
    (
        "Process ${@:2} with $1",
        &["tool", "file1", "file2"],
        "Process file1 file2 with tool",
    ),
    ("${@:1:1} and ${@:2}", &["a", "b", "c"], "a and b c"),
    (
        "${@:1:2} vs ${@:3:2}",
        &["a", "b", "c", "d", "e"],
        "a b vs c d",
    ),
    (
        "Run $1 on ${@:2:2}, then process $@",
        &["eslint", "file1.ts", "file2.ts", "file3.ts"],
        "Run eslint on file1.ts file2.ts, then process eslint file1.ts file2.ts file3.ts",
    ),
    ("prefix${@:2}suffix", &["a", "b", "c"], "prefixb csuffix"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn slice_placeholders_compose_with_original_tokens() {
    for (text, arguments, expected) in SLICE_PLACEHOLDERS_COMPOSE_WITH_ORIGINAL_TOKENS_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const COMMAND_ARGS_SPLIT_ONLY_SPACES_AND_TABS_CASES: &[(&str, &[&str])] = &[
    ("a b c", &["a", "b", "c"]),
    ("", &[]),
    ("a  b   c", &["a", "b", "c"]),
    ("a\tb\tc", &["a", "b", "c"]),
    ("$100 @user #tag", &["$100", "@user", "#tag"]),
    ("日本語 🎉 café", &["日本語", "🎉", "café"]),
    ("\"line1\nline2\" second", &["line1\nline2", "second"]),
    ("a b c   ", &["a", "b", "c"]),
    ("   a b c", &["a", "b", "c"]),
    ("a\nb", &["a\nb"]),
    ("a\rb", &["a\rb"]),
    ("a\u{b} b", &["a\u{b}", "b"]),
    ("a\u{c} b", &["a\u{c}", "b"]),
    ("a b", &["a b"]),
    ("a﻿b", &["a﻿b"]),
    ("ab", &["ab"]),
    ("a b", &["a b"]),
    ("a\\ b", &["a\\", "b"]),
    ("  \t  ", &[]),
    ("a\0b", &["a\0b"]),
];

/// Exercise original inputs through the public template seam.
#[test]
fn command_args_split_only_spaces_and_tabs() {
    for (text, expected) in COMMAND_ARGS_SPLIT_ONLY_SPACES_AND_TABS_CASES {
        assert_eq!(parse_command_args(text), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const COMMAND_ARGS_PRESERVE_QUOTES_AND_LITERAL_BACKSLASHES_CASES: &[(&str, &[&str])] = &[
    ("\"first arg\" second", &["first arg", "second"]),
    ("'first arg' second", &["first arg", "second"]),
    (
        "\"double\" 'single' \"double again\"",
        &["double", "single", "double again"],
    ),
    ("\"\" \" \"", &[" "]),
    ("\"quoted \\\"text\\\"\"", &["quoted \\text\\"]),
    ("ab\"cd\"'ef'", &["abcdef"]),
    ("\"a'b\" 'c\"d'", &["a'b", "c\"d"]),
    ("a \"unclosed b", &["a", "unclosed b"]),
    ("'unclosed\tend", &["unclosed\tend"]),
    ("\"\"''", &[]),
    ("\" \"", &[" "]),
    ("\\\"a b", &["\\a b"]),
    ("\"a\tb\" c", &["a\tb", "c"]),
];

/// Exercise original inputs through the public template seam.
#[test]
fn command_args_preserve_quotes_and_literal_backslashes() {
    for (text, expected) in COMMAND_ARGS_PRESERVE_QUOTES_AND_LITERAL_BACKSLASHES_CASES {
        assert_eq!(parse_command_args(text), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const PARSED_ARGUMENTS_EXPAND_ORIGINAL_MARKER_FORMS_CASES: &[(&str, &[&str])] = &[
    (
        "Button \"onClick handler\" \"disabled support\"",
        &["Button", "onClick handler", "disabled support"],
    ),
    (
        "feature1 feature2 feature3",
        &["feature1", "feature2", "feature3"],
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn parsed_arguments_expand_original_marker_forms() {
    let parsed: Vec<_> = PARSED_ARGUMENTS_EXPAND_ORIGINAL_MARKER_FORMS_CASES
        .iter()
        .map(|(text, expected)| {
            let args = parse_command_args(text);
            assert_eq!(args, *expected);
            args
        })
        .collect();
    for (text, arguments, expected) in
        PARSED_ARGUMENTS_EXPAND_ORIGINAL_MARKER_FORMS_CASES_SUBSTITUTION
    {
        let args = parsed
            .iter()
            .find(|args| args.as_slice() == *arguments)
            .unwrap();
        assert_eq!(substitute_args(text, args), *expected);
    }
}

/// Substitution inputs paired with parsed argument witnesses.
const PARSED_ARGUMENTS_EXPAND_ORIGINAL_MARKER_FORMS_CASES_SUBSTITUTION: &[(&str, &[&str], &str)] =
    &[
        (
            "Create component $1 with features: $ARGUMENTS",
            &["Button", "onClick handler", "disabled support"],
            "Create component Button with features: Button onClick handler disabled support",
        ),
        (
            "Create a React component named $1 with features: $ARGUMENTS",
            &["Button", "onClick handler", "disabled support"],
            "Create a React component named Button with features: Button onClick handler disabled support",
        ),
        (
            "Implement: $@",
            &["feature1", "feature2", "feature3"],
            "Implement: feature1 feature2 feature3",
        ),
        (
            "Implement: $ARGUMENTS",
            &["feature1", "feature2", "feature3"],
            "Implement: feature1 feature2 feature3",
        ),
    ];

/// Native authored files and their complete expected records.
const PROMPT_HINTS_RETAIN_OPTIONAL_STRING_VALUES_CASES: &[NativeCase<'_>] = &[
    (
        &[(
            "pr.md",
            "---\ndescription: Review PRs from URLs with structured issue and code analysis\nargument-hint: \"<PR-URL>\"\n---\nYou are given one or more GitHub PR URLs: $@",
        )],
        &[ExpectedTemplate {
            name: "pr",
            description: "Review PRs from URLs with structured issue and code analysis",
            hint: Some("<PR-URL>"),
            content: "You are given one or more GitHub PR URLs: $@",
            path: "native/pr.md",
            scope: SourceScope::Temporary,
            base: "native",
        }],
    ),
    (
        &[
            (
                "pr.md",
                "---\ndescription: Review PRs from URLs with structured issue and code analysis\nargument-hint: \"<PR-URL>\"\n---\nYou are given one or more GitHub PR URLs: $@",
            ),
            (
                "wr.md",
                "---\ndescription: Finish the current task end-to-end with changelog, commit, and push\nargument-hint: \"[instructions]\"\n---\nWrap it. Additional instructions: $ARGUMENTS",
            ),
        ],
        &[
            ExpectedTemplate {
                name: "pr",
                description: "Review PRs from URLs with structured issue and code analysis",
                hint: Some("<PR-URL>"),
                content: "You are given one or more GitHub PR URLs: $@",
                path: "native/pr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "wr",
                description: "Finish the current task end-to-end with changelog, commit, and push",
                hint: Some("[instructions]"),
                content: "Wrap it. Additional instructions: $ARGUMENTS",
                path: "native/wr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
        ],
    ),
    (
        &[
            (
                "cl.md",
                "---\ndescription: Audit changelog entries before release\n---\nAudit changelog entries for all commits since the last release.",
            ),
            (
                "pr.md",
                "---\ndescription: Review PRs from URLs with structured issue and code analysis\nargument-hint: \"<PR-URL>\"\n---\nYou are given one or more GitHub PR URLs: $@",
            ),
            (
                "wr.md",
                "---\ndescription: Finish the current task end-to-end with changelog, commit, and push\nargument-hint: \"[instructions]\"\n---\nWrap it. Additional instructions: $ARGUMENTS",
            ),
        ],
        &[
            ExpectedTemplate {
                name: "cl",
                description: "Audit changelog entries before release",
                hint: None,
                content: "Audit changelog entries for all commits since the last release.",
                path: "native/cl.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "pr",
                description: "Review PRs from URLs with structured issue and code analysis",
                hint: Some("<PR-URL>"),
                content: "You are given one or more GitHub PR URLs: $@",
                path: "native/pr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "wr",
                description: "Finish the current task end-to-end with changelog, commit, and push",
                hint: Some("[instructions]"),
                content: "Wrap it. Additional instructions: $ARGUMENTS",
                path: "native/wr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
        ],
    ),
    (
        &[
            (
                "cl.md",
                "---\ndescription: Audit changelog entries before release\n---\nAudit changelog entries for all commits since the last release.",
            ),
            (
                "empty-hint.md",
                "---\ndescription: A command with empty hint\nargument-hint: \"\"\n---\nDo something",
            ),
            (
                "pr.md",
                "---\ndescription: Review PRs from URLs with structured issue and code analysis\nargument-hint: \"<PR-URL>\"\n---\nYou are given one or more GitHub PR URLs: $@",
            ),
            (
                "wr.md",
                "---\ndescription: Finish the current task end-to-end with changelog, commit, and push\nargument-hint: \"[instructions]\"\n---\nWrap it. Additional instructions: $ARGUMENTS",
            ),
        ],
        &[
            ExpectedTemplate {
                name: "cl",
                description: "Audit changelog entries before release",
                hint: None,
                content: "Audit changelog entries for all commits since the last release.",
                path: "native/cl.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "empty-hint",
                description: "A command with empty hint",
                hint: None,
                content: "Do something",
                path: "native/empty-hint.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "pr",
                description: "Review PRs from URLs with structured issue and code analysis",
                hint: Some("<PR-URL>"),
                content: "You are given one or more GitHub PR URLs: $@",
                path: "native/pr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "wr",
                description: "Finish the current task end-to-end with changelog, commit, and push",
                hint: Some("[instructions]"),
                content: "Wrap it. Additional instructions: $ARGUMENTS",
                path: "native/wr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
        ],
    ),
    (
        &[
            (
                "cl.md",
                "---\ndescription: Audit changelog entries before release\n---\nAudit changelog entries for all commits since the last release.",
            ),
            (
                "empty-hint.md",
                "---\ndescription: A command with empty hint\nargument-hint: \"\"\n---\nDo something",
            ),
            (
                "is.md",
                "---\ndescription: Analyze GitHub issues (bugs or feature requests)\nargument-hint: \"<issue>\"\n---\nAnalyze GitHub issue(s): $ARGUMENTS",
            ),
            (
                "pr.md",
                "---\ndescription: Review PRs from URLs with structured issue and code analysis\nargument-hint: \"<PR-URL>\"\n---\nYou are given one or more GitHub PR URLs: $@",
            ),
            (
                "wr.md",
                "---\ndescription: Finish the current task end-to-end with changelog, commit, and push\nargument-hint: \"[instructions]\"\n---\nWrap it. Additional instructions: $ARGUMENTS",
            ),
        ],
        &[
            ExpectedTemplate {
                name: "cl",
                description: "Audit changelog entries before release",
                hint: None,
                content: "Audit changelog entries for all commits since the last release.",
                path: "native/cl.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "empty-hint",
                description: "A command with empty hint",
                hint: None,
                content: "Do something",
                path: "native/empty-hint.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "is",
                description: "Analyze GitHub issues (bugs or feature requests)",
                hint: Some("<issue>"),
                content: "Analyze GitHub issue(s): $ARGUMENTS",
                path: "native/is.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "pr",
                description: "Review PRs from URLs with structured issue and code analysis",
                hint: Some("<PR-URL>"),
                content: "You are given one or more GitHub PR URLs: $@",
                path: "native/pr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
            ExpectedTemplate {
                name: "wr",
                description: "Finish the current task end-to-end with changelog, commit, and push",
                hint: Some("[instructions]"),
                content: "Wrap it. Additional instructions: $ARGUMENTS",
                path: "native/wr.md",
                scope: SourceScope::Temporary,
                base: "native",
            },
        ],
    ),
];

/// Check native hint snapshots by name without assuming enumeration order.
#[test]
fn prompt_hints_retain_optional_string_values() {
    for (files, expected) in PROMPT_HINTS_RETAIN_OPTIONAL_STRING_VALUES_CASES {
        let dir = support::Directory::new();
        for (name, text) in *files {
            let _ = dir.file(&format!("native/{name}"), text);
        }
        let folder = dir.0.join("native");
        let paths = vec![support::text(&folder).into()];
        let actual = load_prompt_templates(
            LoadPromptTemplatesOptions {
                cwd: support::text(&dir.0),
                home: support::text(&dir.0),
                agent_dir: support::text(&dir.0),
                config_dir_name: ".maestro",
                prompt_paths: &paths,
                include_defaults: false,
            },
            &maestro_resources::NativeResourceOperations,
        )
        .unwrap();
        let paths: Vec<_> = expected.iter().map(|t| dir.0.join(t.path)).collect();
        let bases: Vec<_> = expected.iter().map(|t| dir.0.join(t.base)).collect();
        let expected: Vec<_> = expected
            .iter()
            .zip(&paths)
            .zip(&bases)
            .map(|((t, p), base)| ExpectedTemplate {
                path: support::text(p),
                base: support::text(base),
                ..*t
            })
            .collect();
        assert_eq!(actual.len(), expected.len());
        for expected in &expected {
            let record = actual
                .iter()
                .find(|record| record.name == expected.name)
                .unwrap();
            assert_record(record, expected);
        }
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const PLACEHOLDER_GRAMMAR_HANDLES_ZERO_HUGE_AND_NONMATCHING_NUMBERS_CASES: &[(
    &str,
    &[&str],
    &str,
)] = &[
    ("$00|$01|$0002|$000", &["a", "b", "c"], "|a|b|"),
    (
        "$9999999999999999999999999999999999999999",
        &["a", "b", "c"],
        "",
    ),
    (
        "${@:00}|${@:01:02}|${@:0:0}",
        &["a", "b", "c"],
        "a b c|a b|",
    ),
    (
        "${@:1:99999999999999999999999999999999999}",
        &["a", "b", "c"],
        "a b c",
    ),
    (
        "${@:9999999999999999999999999999999999999:3}",
        &["a", "b", "c"],
        "",
    ),
    (
        "${@:}|${@:1:}|${@:-1}|${@:1:-1}|${@:1:2:3}|${@: 1}|${@:1.5}|${@:１}",
        &["a", "b", "c"],
        "${@:}|${@:1:}|${@:-1}|${@:1:-1}|${@:1:2:3}|${@: 1}|${@:1.5}|${@:１}",
    ),
    ("$１|$٢|${@:٢}", &["a", "b", "c"], "$１|$٢|${@:٢}"),
    ("$+1|$-1|$1e2|$1_2", &["a", "b", "c"], "$+1|$-1|ae2|a_2"),
    ("$$1|$$@|$$ARGUMENTS", &["a", "b", "c"], "$a|$a b c|$a b c"),
    ("${@:2}${@:1:1}", &["a", "b", "c"], "b ca"),
    ("$ARGUMENTSX|$@X", &["a", "b", "c"], "a b cX|a b cX"),
    (
        "$1|$2|$3|$@|$ARGUMENTS|${@:1}|${@:0:2}|${@:2:0}",
        &[],
        "|||||||",
    ),
    (
        "$1|$2|$3|$@|$ARGUMENTS|${@:1}|${@:0:2}|${@:2:0}",
        &["only"],
        "only|||only|only|only|only|",
    ),
    (
        "$1|$2|$3|$@|$ARGUMENTS|${@:1}|${@:0:2}|${@:2:0}",
        &["a", "", " c "],
        "a|| c |a   c |a   c |a   c |a |",
    ),
    ("${@:0:1}|${@:1:1}|${@:2:1}", &[], "||"),
    ("${@:0:1}|${@:1:1}|${@:2:1}", &["only"], "only|only|"),
    ("${@:0:1}|${@:1:1}|${@:2:1}", &["a", "", " c "], "a|a|"),
    (
        "$00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001|${@:00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002:1}",
        &["a", "b", "c"],
        "a|b",
    ),
];

/// Exercise original inputs through the public template seam.
#[test]
fn placeholder_grammar_handles_zero_huge_and_nonmatching_numbers() {
    for (text, arguments, expected) in
        PLACEHOLDER_GRAMMAR_HANDLES_ZERO_HUGE_AND_NONMATCHING_NUMBERS_CASES
    {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const INSERTED_ARGUMENTS_CANNOT_COMPLETE_TEMPLATE_TOKENS_CASES: &[(&str, &[&str], &str)] = &[
    ("${@:1:$1}", &["2", "b"], "${@:1:2}"),
    ("${@:$1}", &["2", "b"], "${@:2}"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn inserted_arguments_cannot_complete_template_tokens() {
    for (text, arguments, expected) in INSERTED_ARGUMENTS_CANNOT_COMPLETE_TEMPLATE_TOKENS_CASES {
        let args: Vec<String> = arguments.iter().map(|s| (*s).into()).collect();
        assert_eq!(substitute_args(text, &args), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const MAESTRO_PROMPT_TEMPLATE_EXPANDS_FIRST_MATCHING_NAME_CASES: &[ExpansionCase<'_>] = &[
    (
        "plain",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "plain",
    ),
    (
        " /go x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        " /go x",
    ),
    (
        "/unknown x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "/unknown x",
    ),
    (
        "/go",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "||",
    ),
    (
        "/go a \"b c\" d",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "a|b c d|a b c d",
    ),
    (
        "/Go x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "upper:x",
    ),
    (
        "/GO x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "/GO x",
    ),
    (
        "/go\tx",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "/go\tx",
    ),
    (
        "/go\nx",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "/go\nx",
    ),
    (
        "/go\ttab z",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "tab:z",
    ),
    (
        "/go\nline z",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "newline:z",
    ),
    (
        "/",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "empty:",
    ),
    (
        "/ x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "empty:x",
    ),
    (
        "//go x",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "//go x",
    ),
    (
        "/go  a\tb",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "a|b|a b",
    ),
    (
        "/go a\nq",
        &[
            ("go", "$1|${@:2}|$@"),
            ("go", "WRONG"),
            ("Go", "upper:$@"),
            ("", "empty:$@"),
            ("go\ttab", "tab:$1"),
            ("go\nline", "newline:$1"),
        ],
        "a\nq||a\nq",
    ),
    ("/go x", &[], "/go x"),
];

/// Exercise original inputs through the public template seam.
#[test]
fn maestro_prompt_template_expands_first_matching_name() {
    for (text, templates, expected) in MAESTRO_PROMPT_TEMPLATE_EXPANDS_FIRST_MATCHING_NAME_CASES {
        let templates: Vec<_> = templates
            .iter()
            .map(|(name, content)| template(name, content))
            .collect();
        assert_eq!(expand_prompt_template(text, &templates), *expected);
    }
}

/// Distinct authored inputs and independently recorded expected outputs.
const PROMPT_EXPANSION_KEEPS_INSERTED_COMMAND_ARGUMENTS_LITERAL_CASES: &[ExpansionCase<'_>] = &[(
    "/go \"$@\" \"${@:1}\"",
    &[
        ("go", "$1|${@:2}|$@"),
        ("go", "WRONG"),
        ("Go", "upper:$@"),
        ("", "empty:$@"),
        ("go\ttab", "tab:$1"),
        ("go\nline", "newline:$1"),
    ],
    "$@|${@:1}|$@ ${@:1}",
)];

/// Exercise original inputs through the public template seam.
#[test]
fn prompt_expansion_keeps_inserted_command_arguments_literal() {
    for (text, templates, expected) in
        PROMPT_EXPANSION_KEEPS_INSERTED_COMMAND_ARGUMENTS_LITERAL_CASES
    {
        let templates: Vec<_> = templates
            .iter()
            .map(|(name, content)| template(name, content))
            .collect();
        assert_eq!(expand_prompt_template(text, &templates), *expected);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const TEMPLATE_DESCRIPTIONS_CHOOSE_AUTHORED_TEXT_OR_FIRST_NONBLANK_LINE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("\n \t\n  first line  \nsecond")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "  first line  ",
            hint: None,
            content: "\n \t\n  first line  \nsecond",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[("/outside/a.md", Node::File(Some("\n \t\n")))],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "",
            hint: None,
            content: "\n \t\n",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[("/outside/a.md", Node::File(Some("﻿\nnext")))],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "next",
            hint: None,
            content: "﻿\nnext",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[("/outside/a.md", Node::File(Some("\nnext")))],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "",
            hint: None,
            content: "\nnext",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: \"\"\nargument-hint: \"\"\n---\n first\nsecond",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "first",
            hint: None,
            content: "first\nsecond",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: \"   \"\nargument-hint: \" \"\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "   ",
            hint: Some(" "),
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: \"🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉\"\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn template_descriptions_choose_authored_text_or_first_nonblank_line() {
    for case in TEMPLATE_DESCRIPTIONS_CHOOSE_AUTHORED_TEXT_OR_FIRST_NONBLANK_LINE_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_METADATA_SELECTS_NAMED_FIELDS_WITHOUT_WHOLE_RECORD_VALIDATION_CASES: &[LoadCase<
    '_,
>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: \"  authored  \"\nargument-hint: \"  <x> [y]  \"\n---\n  body\r\nnext\r",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "  authored  ",
            hint: Some("  <x> [y]  "),
            content: "body\nnext",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: null\nargument-hint: null\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[("/outside/a.md", Node::File(Some("---\n123\n---\nbody")))],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\n[one, two]\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nextra: {deep: [{nested: true}]}\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\n\"descr\\u0069ption\": correct\n\"argument-\\u0068int\": \"<x>\"\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "correct",
            hint: Some("<x>"),
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: a\n---suffix\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a",
            hint: None,
            content: "suffix\nbody",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: a\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "---",
            hint: None,
            content: "---\ndescription: a\nbody",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\nDescription: wrong\nargumentHint: wrong\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_metadata_selects_named_fields_without_whole_record_validation() {
    for case in PROMPT_METADATA_SELECTS_NAMED_FIELDS_WITHOUT_WHOLE_RECORD_VALIDATION_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_METADATA_PARSE_FAILURES_OMIT_THE_FILE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: first\ndescription: last\n---\nbody",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: [\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_metadata_parse_failures_omit_the_file() {
    for case in PROMPT_METADATA_PARSE_FAILURES_OMIT_THE_FILE_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_METADATA_OMITS_NONSTRING_SELECTED_FIELDS_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: false\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: true\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: 0\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: 42\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: []\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: [text]\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: {}\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: {nested: value}\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: .nan\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: .inf\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: false\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: true\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: 0\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: 42\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: []\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: [text]\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: {}\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: {nested: value}\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: .nan\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\nargument-hint: .inf\n---\nbody")),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_metadata_omits_nonstring_selected_fields() {
    for case in PROMPT_METADATA_OMITS_NONSTRING_SELECTED_FIELDS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const TEMPLATE_DESCRIPTION_LIMIT_KEEPS_WHOLE_SCALARS_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            hint: None,
            content: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            hint: None,
            content: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx...",
            hint: None,
            content: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉",
            hint: None,
            content: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉z",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉...",
            hint: None,
            content: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx🎉z",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉...",
            hint: None,
            content: "🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉🎉",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "a\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}",
            )),
        )],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/outside/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}...",
            hint: None,
            content: "a\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}",
            path: "/outside/a.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn template_description_limit_keeps_whole_scalars() {
    for case in TEMPLATE_DESCRIPTION_LIMIT_KEEPS_WHOLE_SCALARS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_DISCOVERY_PRESERVES_ORDER_AND_SHALLOW_MARKDOWN_SELECTION_CASES: &[LoadCase<'_>] =
    &[LoadCase {
        entries: &[
            (
                "/user/prompts",
                Node::Dir(&[
                    "z.md",
                    ".hidden.md",
                    "a.MD",
                    "nested",
                    "a.md",
                    ".md",
                    "twice.md.md",
                ]),
            ),
            ("/user/prompts/z.md", Node::File(Some("user-z"))),
            ("/user/prompts/.hidden.md", Node::File(Some("hidden"))),
            ("/user/prompts/a.MD", Node::File(None)),
            ("/user/prompts/nested", Node::Dir(&[])),
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/user/prompts/.md", Node::File(Some("dotname"))),
            ("/user/prompts/twice.md.md", Node::File(Some("twice"))),
            ("/work/.maestro/prompts", Node::Dir(&["a.md"])),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
            (
                "/extra",
                Node::Dir(&["q.md", ".gitignore", ".ignore", ".fdignore"]),
            ),
            ("/extra/q.md", Node::File(Some("explicit-q"))),
            ("/extra/.gitignore", Node::File(None)),
            ("/extra/.ignore", Node::File(None)),
            ("/extra/.fdignore", Node::File(None)),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: true,
        },
        paths: &["/extra", "/user/prompts/a.md"],
        expected: &[
            ExpectedTemplate {
                name: "z",
                description: "user-z",
                hint: None,
                content: "user-z",
                path: "/user/prompts/z.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: ".hidden",
                description: "hidden",
                hint: None,
                content: "hidden",
                path: "/user/prompts/.hidden.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/user/prompts/a.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: "",
                description: "dotname",
                hint: None,
                content: "dotname",
                path: "/user/prompts/.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: "twice.md",
                description: "twice",
                hint: None,
                content: "twice",
                path: "/user/prompts/twice.md.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "project-a",
                hint: None,
                content: "project-a",
                path: "/work/.maestro/prompts/a.md",
                scope: SourceScope::Project,
                base: "/work/.maestro/prompts",
            },
            ExpectedTemplate {
                name: "q",
                description: "explicit-q",
                hint: None,
                content: "explicit-q",
                path: "/extra/q.md",
                scope: SourceScope::Temporary,
                base: "/extra",
            },
            ExpectedTemplate {
                name: "a",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/user/prompts/a.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
        ],
    }];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_discovery_preserves_order_and_shallow_markdown_selection() {
    for case in PROMPT_DISCOVERY_PRESERVES_ORDER_AND_SHALLOW_MARKDOWN_SELECTION_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_LOADER_DISABLES_ONLY_DEFAULT_DISCOVERY_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        (
            "/extra",
            Node::Dir(&["q.md", ".gitignore", ".ignore", ".fdignore"]),
        ),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/.gitignore", Node::File(None)),
        ("/extra/.ignore", Node::File(None)),
        ("/extra/.fdignore", Node::File(None)),
    ],
    failures: &[],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &["/extra"],
    expected: &[ExpectedTemplate {
        name: "q",
        description: "explicit-q",
        hint: None,
        content: "explicit-q",
        path: "/extra/q.md",
        scope: SourceScope::Temporary,
        base: "/extra",
    }],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_loader_disables_only_default_discovery() {
    for case in PROMPT_LOADER_DISABLES_ONLY_DEFAULT_DISCOVERY_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const MAESTRO_PROMPT_LOADER_RETAINS_DUPLICATE_PATHS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        (
            "/extra",
            Node::Dir(&["q.md", ".gitignore", ".ignore", ".fdignore"]),
        ),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/.gitignore", Node::File(None)),
        ("/extra/.ignore", Node::File(None)),
        ("/extra/.fdignore", Node::File(None)),
    ],
    failures: &[],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &["/extra", "/extra/q.md", "/extra"],
    expected: &[
        ExpectedTemplate {
            name: "q",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/q.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
        ExpectedTemplate {
            name: "q",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/q.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
        ExpectedTemplate {
            name: "q",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/q.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
    ],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn maestro_prompt_loader_retains_duplicate_paths() {
    for case in MAESTRO_PROMPT_LOADER_RETAINS_DUPLICATE_PATHS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_LOADER_FOLLOWS_FILE_LINKS_AND_SKIPS_OTHER_KINDS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        (
            "/extra",
            Node::Dir(&[
                "link.md",
                "broken.md",
                "sub.md",
                "device.md",
                "no.txt",
                "q.md",
            ]),
        ),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/no.txt", Node::File(None)),
        ("/extra/device.md", Node::Other),
        ("/extra/link.md", Node::Link("/extra/q.md")),
        ("/extra/broken.md", Node::Link("/absent")),
        ("/extra/sub.md", Node::Link("/extra")),
    ],
    failures: &[],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &[
        "/extra",
        "/extra/broken.md",
        "/extra/no.txt",
        "/extra/device.md",
        "/absent",
        "/extra/link.md",
    ],
    expected: &[
        ExpectedTemplate {
            name: "link",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/link.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
        ExpectedTemplate {
            name: "q",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/q.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
        ExpectedTemplate {
            name: "link",
            description: "explicit-q",
            hint: None,
            content: "explicit-q",
            path: "/extra/link.md",
            scope: SourceScope::Temporary,
            base: "/extra",
        },
    ],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_loader_follows_file_links_and_skips_other_kinds() {
    for case in PROMPT_LOADER_FOLLOWS_FILE_LINKS_AND_SKIPS_OTHER_KINDS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_PROVENANCE_USES_AUTHORED_PATHS_AND_USER_PRECEDENCE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
            ("/user/prompts-more/a.md", Node::File(Some("outside"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &[
            "/user/prompts/a.md",
            "/work/.maestro/prompts/a.md",
            "/user/prompts-more/a.md",
        ],
        expected: &[
            ExpectedTemplate {
                name: "a",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/user/prompts/a.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "project-a",
                hint: None,
                content: "project-a",
                path: "/work/.maestro/prompts/a.md",
                scope: SourceScope::Project,
                base: "/work/.maestro/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "outside",
                hint: None,
                content: "outside",
                path: "/user/prompts-more/a.md",
                scope: SourceScope::Temporary,
                base: "/user/prompts-more",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/work/.maestro/prompts", Node::Dir(&["a.md"])),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/work/.maestro",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: true,
        },
        paths: &["/work/.maestro/prompts/a.md"],
        expected: &[
            ExpectedTemplate {
                name: "a",
                description: "project-a",
                hint: None,
                content: "project-a",
                path: "/work/.maestro/prompts/a.md",
                scope: SourceScope::User,
                base: "/work/.maestro/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "project-a",
                hint: None,
                content: "project-a",
                path: "/work/.maestro/prompts/a.md",
                scope: SourceScope::User,
                base: "/work/.maestro/prompts",
            },
            ExpectedTemplate {
                name: "a",
                description: "project-a",
                hint: None,
                content: "project-a",
                path: "/work/.maestro/prompts/a.md",
                scope: SourceScope::User,
                base: "/work/.maestro/prompts",
            },
        ],
    },
    LoadCase {
        entries: &[("/a.md", Node::File(Some("a")))],
        failures: &[],
        kinds: &[],
        process: "/",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/a.md"],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a",
            hint: None,
            content: "a",
            path: "/a.md",
            scope: SourceScope::User,
            base: "",
        }],
    },
    LoadCase {
        entries: &[
            ("/user/prompts/a.MD", Node::File(None)),
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/extra/q.md", Node::File(Some("explicit-q"))),
            ("/extra/user.md", Node::Link("/user/prompts/a.md")),
            ("/user/prompts/extra.md", Node::Link("/extra/q.md")),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &[
            "/extra/user.md",
            "/user/prompts/extra.md",
            "/user/prompts/a.MD",
        ],
        expected: &[
            ExpectedTemplate {
                name: "user",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/extra/user.md",
                scope: SourceScope::Temporary,
                base: "/extra",
            },
            ExpectedTemplate {
                name: "extra",
                description: "explicit-q",
                hint: None,
                content: "explicit-q",
                path: "/user/prompts/extra.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(Some("bad"))),
            ("/scan/last.md", Node::File(Some("last"))),
        ],
        failures: &[],
        kinds: &[("/scan/first.md", &[ResourceFileType::Directory])],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan/first.md",
            },
            ExpectedTemplate {
                name: "bad",
                description: "bad",
                hint: None,
                content: "bad",
                path: "/scan/bad.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "last",
                description: "last",
                hint: None,
                content: "last",
                path: "/scan/last.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
        ],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_provenance_uses_authored_paths_and_user_precedence() {
    for case in PROMPT_PROVENANCE_USES_AUTHORED_PATHS_AND_USER_PRECEDENCE_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_PATHS_KEEP_ENTRY_SPECIFIC_RESOLUTION_AND_SPELLING_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/extra/q.md", Node::File(Some("explicit-q"))),
            ("/home/user/t.md", Node::File(Some("home"))),
            ("/home/user", Node::Dir(&["t.md"])),
            ("/home/user/extra/a.md", Node::File(Some("tildeuser"))),
            ("/work/relative.md", Node::File(Some("relative"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &[
            "﻿ ~/t.md  ",
            "~extra/a.md",
            "~",
            "relative.md",
            "/extra/../extra/q.md",
            "/extra/q.md",
        ],
        expected: &[
            ExpectedTemplate {
                name: "t",
                description: "home",
                hint: None,
                content: "home",
                path: "/home/user/t.md",
                scope: SourceScope::Temporary,
                base: "/home/user",
            },
            ExpectedTemplate {
                name: "a",
                description: "tildeuser",
                hint: None,
                content: "tildeuser",
                path: "/home/user/extra/a.md",
                scope: SourceScope::Temporary,
                base: "/home/user/extra",
            },
            ExpectedTemplate {
                name: "t",
                description: "home",
                hint: None,
                content: "home",
                path: "/home/user/t.md",
                scope: SourceScope::Temporary,
                base: "/home/user",
            },
            ExpectedTemplate {
                name: "relative",
                description: "relative",
                hint: None,
                content: "relative",
                path: "/work/relative.md",
                scope: SourceScope::Temporary,
                base: "/work",
            },
            ExpectedTemplate {
                name: "q",
                description: "explicit-q",
                hint: None,
                content: "explicit-q",
                path: "/extra/../extra/q.md",
                scope: SourceScope::Temporary,
                base: "/extra/../extra",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/work/relative.md", Node::File(Some("relative"))),
            ("/work", Node::Dir(&["relative.md"])),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &[""],
        expected: &[ExpectedTemplate {
            name: "relative",
            description: "relative",
            hint: None,
            content: "relative",
            path: "/work/relative.md",
            scope: SourceScope::Temporary,
            base: "/work",
        }],
    },
    LoadCase {
        entries: &[
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/extra/q.md", Node::File(Some("explicit-q"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/extra/../extra/q.md", "/user/prompts/../prompts/a.md"],
        expected: &[
            ExpectedTemplate {
                name: "q",
                description: "explicit-q",
                hint: None,
                content: "explicit-q",
                path: "/extra/../extra/q.md",
                scope: SourceScope::Temporary,
                base: "/extra/../extra",
            },
            ExpectedTemplate {
                name: "a",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/user/prompts/../prompts/a.md",
                scope: SourceScope::User,
                base: "/user/prompts",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/ambient/user/prompts", Node::Dir(&["a.md"])),
            ("/ambient/user/prompts/a.md", Node::File(Some("a"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: true,
        },
        paths: &[],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a",
            hint: None,
            content: "a",
            path: "user/prompts/a.md",
            scope: SourceScope::Temporary,
            base: "user/prompts",
        }],
    },
    LoadCase {
        entries: &[
            ("/ambient", Node::Dir(&["a.md"])),
            ("/ambient/a.md", Node::File(Some("a"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: true,
        },
        paths: &[],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a",
            hint: None,
            content: "a",
            path: "a.md",
            scope: SourceScope::Temporary,
            base: ".",
        }],
    },
    LoadCase {
        entries: &[("/ambient/work/x.md", Node::File(Some("x")))],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["x.md"],
        expected: &[ExpectedTemplate {
            name: "x",
            description: "x",
            hint: None,
            content: "x",
            path: "/ambient/work/x.md",
            scope: SourceScope::Temporary,
            base: "/ambient/work",
        }],
    },
    LoadCase {
        entries: &[
            ("/home/user/t.md", Node::File(Some("home"))),
            ("/home/user", Node::Dir(&["t.md"])),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/other/../user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["~", "~/t.md"],
        expected: &[
            ExpectedTemplate {
                name: "t",
                description: "home",
                hint: None,
                content: "home",
                path: "/home/user/t.md",
                scope: SourceScope::Temporary,
                base: "/home/user",
            },
            ExpectedTemplate {
                name: "t",
                description: "home",
                hint: None,
                content: "home",
                path: "/home/user/t.md",
                scope: SourceScope::Temporary,
                base: "/home/user",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/work/.custom/prompts", Node::Dir(&["a.md"])),
            ("/work/.custom/prompts/a.md", Node::File(Some("a"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".custom",
            prompt_paths: &[],
            include_defaults: true,
        },
        paths: &[],
        expected: &[ExpectedTemplate {
            name: "a",
            description: "a",
            hint: None,
            content: "a",
            path: "/work/.custom/prompts/a.md",
            scope: SourceScope::Project,
            base: "/work/.custom/prompts",
        }],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_paths_keep_entry_specific_resolution_and_spelling() {
    for case in PROMPT_PATHS_KEEP_ENTRY_SPECIFIC_RESOLUTION_AND_SPELLING_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_FILE_FAILURES_CONTINUE_THE_SAME_SCAN_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(None)),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
        failures: &[("read", "/scan/bad.md", 0)],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan", "/after.md"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "last",
                description: "last",
                hint: None,
                content: "last",
                path: "/scan/last.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "after",
                description: "after",
                hint: None,
                content: "after",
                path: "/after.md",
                scope: SourceScope::Temporary,
                base: "/",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(Some("---\na: [\n---\nbad"))),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan", "/after.md"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "last",
                description: "last",
                hint: None,
                content: "last",
                path: "/scan/last.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "after",
                description: "after",
                hint: None,
                content: "after",
                path: "/after.md",
                scope: SourceScope::Temporary,
                base: "/",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            (
                "/scan/bad.md",
                Node::File(Some("---\ndescription: 42\n---\nbad")),
            ),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
        failures: &[],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan", "/after.md"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "last",
                description: "last",
                hint: None,
                content: "last",
                path: "/scan/last.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "after",
                description: "after",
                hint: None,
                content: "after",
                path: "/after.md",
                scope: SourceScope::Temporary,
                base: "/",
            },
        ],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_file_failures_continue_the_same_scan() {
    for case in PROMPT_FILE_FAILURES_CONTINUE_THE_SAME_SCAN_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_SOURCE_METADATA_FAILURE_ENDS_ONLY_CURRENT_SCAN_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(None)),
            ("/scan/last.md", Node::File(None)),
            ("/after.md", Node::File(Some("after"))),
        ],
        failures: &[("stat", "/scan/bad.md", 0)],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan", "/after.md"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "after",
                description: "after",
                hint: None,
                content: "after",
                path: "/after.md",
                scope: SourceScope::Temporary,
                base: "/",
            },
        ],
    },
    LoadCase {
        entries: &[
            ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::Link("/scan/first.md")),
            ("/scan/last.md", Node::File(None)),
            ("/after.md", Node::File(Some("after"))),
        ],
        failures: &[("stat", "/scan/bad.md", 2)],
        kinds: &[],
        process: "/ambient",
        options: LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &[],
            include_defaults: false,
        },
        paths: &["/scan", "/after.md"],
        expected: &[
            ExpectedTemplate {
                name: "first",
                description: "first",
                hint: None,
                content: "first",
                path: "/scan/first.md",
                scope: SourceScope::Temporary,
                base: "/scan",
            },
            ExpectedTemplate {
                name: "after",
                description: "after",
                hint: None,
                content: "after",
                path: "/after.md",
                scope: SourceScope::Temporary,
                base: "/",
            },
        ],
    },
];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_source_metadata_failure_ends_only_current_scan() {
    for case in PROMPT_SOURCE_METADATA_FAILURE_ENDS_ONLY_CURRENT_SCAN_CASES {
        let calls = run_case(case);
        assert!(
            !calls
                .iter()
                .any(|(op, p)| op == "read" && (p == "/scan/bad.md" || p == "/scan/last.md"))
        );
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_DIRECTORY_FAILURE_ALLOWS_LATER_EXPLICIT_PATHS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/scan", Node::Dir(&[])),
        ("/after.md", Node::File(Some("after"))),
    ],
    failures: &[("dir", "/scan", 0)],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &["/scan", "/after.md"],
    expected: &[ExpectedTemplate {
        name: "after",
        description: "after",
        hint: None,
        content: "after",
        path: "/after.md",
        scope: SourceScope::Temporary,
        base: "/",
    }],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_directory_failure_allows_later_explicit_paths() {
    for case in PROMPT_DIRECTORY_FAILURE_ALLOWS_LATER_EXPLICIT_PATHS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const PROMPT_LINK_METADATA_FAILURE_SKIPS_ONLY_THAT_LINK_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/scan", Node::Dir(&["first.md", "bad.md", "last.md"])),
        ("/scan/first.md", Node::File(Some("first"))),
        ("/scan/bad.md", Node::Link("/scan/first.md")),
        ("/scan/last.md", Node::File(Some("last"))),
        ("/after.md", Node::File(Some("after"))),
    ],
    failures: &[("stat", "/scan/bad.md", 0)],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &["/scan", "/after.md"],
    expected: &[
        ExpectedTemplate {
            name: "first",
            description: "first",
            hint: None,
            content: "first",
            path: "/scan/first.md",
            scope: SourceScope::Temporary,
            base: "/scan",
        },
        ExpectedTemplate {
            name: "last",
            description: "last",
            hint: None,
            content: "last",
            path: "/scan/last.md",
            scope: SourceScope::Temporary,
            base: "/scan",
        },
        ExpectedTemplate {
            name: "after",
            description: "after",
            hint: None,
            content: "after",
            path: "/after.md",
            scope: SourceScope::Temporary,
            base: "/",
        },
    ],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn prompt_link_metadata_failure_skips_only_that_link() {
    for case in PROMPT_LINK_METADATA_FAILURE_SKIPS_ONLY_THAT_LINK_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const SCOPED_PROMPT_CANDIDATES_AVOID_UNNEEDED_METADATA_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/user/prompts", Node::Dir(&["a.md"])),
        ("/user/prompts/a.md", Node::File(Some("a"))),
    ],
    failures: &[("stat", "/user/prompts/a.md", 0)],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: true,
    },
    paths: &[],
    expected: &[ExpectedTemplate {
        name: "a",
        description: "a",
        hint: None,
        content: "a",
        path: "/user/prompts/a.md",
        scope: SourceScope::User,
        base: "/user/prompts",
    }],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn scoped_prompt_candidates_avoid_unneeded_metadata() {
    for case in SCOPED_PROMPT_CANDIDATES_AVOID_UNNEEDED_METADATA_CASES {
        let calls = run_case(case);
        assert!(!calls.iter().any(|(op, _)| op == "stat"));
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const EXPLICIT_PROMPT_METADATA_FAILURE_ALLOWS_LATER_PATHS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/scan/first.md", Node::File(None)),
        ("/after.md", Node::File(Some("after"))),
    ],
    failures: &[("stat", "/scan/first.md", 0)],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: false,
    },
    paths: &["/scan/first.md", "/after.md"],
    expected: &[ExpectedTemplate {
        name: "after",
        description: "after",
        hint: None,
        content: "after",
        path: "/after.md",
        scope: SourceScope::Temporary,
        base: "/",
    }],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn explicit_prompt_metadata_failure_allows_later_paths() {
    for case in EXPLICIT_PROMPT_METADATA_FAILURE_ALLOWS_LATER_PATHS_CASES {
        run_case(case);
    }
}

/// Distinct supplied filesystem inputs with complete expected records.
const ABSENT_PROMPT_ROOTS_YIELD_AN_EMPTY_LIST_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[],
    failures: &[],
    kinds: &[],
    process: "/ambient",
    options: LoadPromptTemplatesOptions {
        cwd: "/work",
        home: "/home/user",
        agent_dir: "/user",
        config_dir_name: ".maestro",
        prompt_paths: &[],
        include_defaults: true,
    },
    paths: &[],
    expected: &[],
}];

/// Exercise supplied template observations through the public loader.
#[test]
fn absent_prompt_roots_yield_an_empty_list() {
    for case in ABSENT_PROMPT_ROOTS_YIELD_AN_EMPTY_LIST_CASES {
        run_case(case);
    }
}

/// Anchored operands never observe the unavailable process directory.
#[test]
fn anchored_prompt_paths_do_not_read_the_process_directory() {
    let entries = &[("/outside/ambient-template.md", Node::File(Some("body")))];
    let mut operations = Observations::new(entries);
    operations.cwd = None;
    let paths = vec!["/outside/ambient-template.md".into()];
    let actual = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        },
        &operations,
    )
    .unwrap();
    assert_templates(
        &actual,
        &[PromptTemplate {
            name: "ambient-template".into(),
            description: "body".into(),
            argument_hint: None,
            content: "body".into(),
            file_path: "/outside/ambient-template.md".into(),
            source_info: create_synthetic_source_info(
                "/outside/ambient-template.md".into(),
                SyntheticSourceOptions {
                    source: "local".into(),
                    scope: None,
                    origin: None,
                    base_dir: Some("/outside".into()),
                },
            ),
        }],
    );
    assert!(!operations.calls.borrow().iter().any(|(op, _)| op == "cwd"));
}

/// Project-root resolution precedes defaults and explicit-path dispatch.
#[test]
fn unanchored_prompt_project_root_propagates_directory_failure() {
    for paths in [vec!["/outside/ambient-template.md".into()], Vec::new()] {
        let mut operations = Observations::new(&[]);
        operations.cwd = None;
        let error = load_prompt_templates(
            LoadPromptTemplatesOptions {
                cwd: "relative",
                home: "/home/user",
                agent_dir: "/user",
                config_dir_name: ".maestro",
                prompt_paths: &paths,
                include_defaults: false,
            },
            &operations,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(*operations.calls.borrow(), [("cwd".into(), String::new())]);
    }
}

/// A source-resolution failure inside explicit dispatch prevents the file read.
#[test]
fn prompt_source_resolution_failure_is_silently_caught() {
    let entries = &[("/outside/ambient-template.md", Node::File(None))];
    let mut operations = Observations::new(entries);
    operations.cwd = None;
    let paths = vec!["/outside/ambient-template.md".into()];
    let actual = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "relative",
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        },
        &operations,
    )
    .unwrap();
    assert!(actual.is_empty());
    assert_eq!(
        *operations.calls.borrow(),
        [
            ("exists".into(), "/outside/ambient-template.md".into()),
            ("stat".into(), "/outside/ambient-template.md".into()),
            ("cwd".into(), String::new()),
        ]
    );
}

/// Missing explicit files skip before source resolution observes process state.
#[test]
fn missing_prompt_paths_do_not_resolve_their_source() {
    let mut operations = Observations::new(&[]);
    operations.cwd = None;
    let paths = vec!["/absent-template.md".into()];
    let actual = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: "/work",
            home: "/home/user",
            agent_dir: "relative",
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        },
        &operations,
    )
    .unwrap();
    assert!(actual.is_empty());
    assert_eq!(
        *operations.calls.borrow(),
        [("exists".into(), "/absent-template.md".into())]
    );
}

/// Authored files paired with expected native records.
type NativeCase<'a> = (&'a [(&'a str, &'a str)], &'a [ExpectedTemplate<'a>]);
/// Input command, ordered templates and expected text.
type ExpansionCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);

/// Native discovery follows file links but skips directory and broken links.
#[cfg(unix)]
#[test]
fn native_prompt_discovery_reads_authored_file_links() {
    let dir = support::Directory::new();
    let target = dir.file("target.md", "linked body");
    let plain = dir.file("prompts/.hidden.md", "plain body");
    let folder = plain.parent().unwrap();
    std::os::unix::fs::symlink(target, folder.join("alias.md")).unwrap();
    std::os::unix::fs::symlink(folder, folder.join("directory.md")).unwrap();
    std::os::unix::fs::symlink(dir.0.join("absent"), folder.join("broken.md")).unwrap();
    let paths = vec![support::text(folder).into()];
    let operations = support::Controlled {
        failures: Vec::new(),
        order: vec![(
            folder.into(),
            vec!["alias.md", "broken.md", "directory.md", ".hidden.md"],
        )],
    };
    let actual = load_prompt_templates(
        LoadPromptTemplatesOptions {
            cwd: support::text(&dir.0),
            home: support::text(&dir.0),
            agent_dir: "",
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        },
        &operations,
    )
    .unwrap();
    let expected_paths = [folder.join("alias.md"), folder.join(".hidden.md")];
    let expected: Vec<_> = [("alias", "linked body"), (".hidden", "plain body")]
        .iter()
        .zip(&expected_paths)
        .map(|((name, body), path)| ExpectedTemplate {
            name,
            description: body,
            hint: None,
            content: body,
            path: support::text(path),
            scope: SourceScope::Temporary,
            base: support::text(folder),
        })
        .collect();
    assert_records(&actual, &expected);
}
