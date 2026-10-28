//! Prompt parsing, ordered loading and expansion through public interfaces.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_resources::{
    LoadPromptTemplatesOptions, PromptTemplate, SourceOrigin, SourceScope, SyntheticSourceOptions,
    create_synthetic_source_info, expand_prompt_template, load_prompt_templates,
    parse_command_args, substitute_args,
};

/// A fixture entry, created below the case's root.
#[cfg(unix)]
enum Node<'a> {
    /// Regular file with its text, or `None` for a file nobody may read.
    File(Option<&'a str>),
    /// Directory; its entries are the fixture entries below it.
    Dir,
    /// Directory nobody may list.
    Locked,
    /// Symlink to an authored fixture path.
    Link(&'a str),
    /// Another file-system kind (a socket).
    Other,
}

/// The real spelling of fixture path `path`: absolute paths move below `root`.
fn at(root: &str, path: &str) -> String {
    match path {
        "/" => root.into(),
        path if path.starts_with('/') => {
            format!("{root}{}", path.replace('/', std::path::MAIN_SEPARATOR_STR))
        }
        path => path.into(),
    }
}

/// Create `entries` below `root` and take away the permissions the locked ones lose.
/// Returns the guards that give them back, or `None` when a privileged run reads them anyway.
#[cfg(unix)]
fn materialize(root: &str, entries: &[(&str, Node<'_>)]) -> Option<Vec<support::Restore>> {
    let mut locked = Vec::new();
    for (path, node) in entries {
        let path = std::path::PathBuf::from(at(root, path));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        match node {
            Node::File(text) => std::fs::write(&path, text.unwrap_or_default()).unwrap(),
            Node::Dir | Node::Locked => std::fs::create_dir_all(&path).unwrap(),
            Node::Link(target) => std::os::unix::fs::symlink(at(root, target), &path).unwrap(),
            Node::Other => drop(std::os::unix::net::UnixListener::bind(&path).unwrap()),
        }
        if matches!(node, Node::File(None) | Node::Locked) {
            locked.push(path);
        }
    }
    locked
        .iter()
        .map(|path| support::deny(path).map(|(guard, _)| guard))
        .collect()
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

/// Compare all output fields with borrowed expected records whose paths lie below `root`.
#[cfg(unix)]
fn assert_records(actual: &[PromptTemplate], expected: &[ExpectedTemplate<'_>], root: &str) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_record(actual, expected, root);
    }
}

/// Check one complete record without assuming a directory enumeration order.
fn assert_record(actual: &PromptTemplate, expected: &ExpectedTemplate<'_>, root: &str) {
    let path = at(root, expected.path);
    assert_eq!(actual.name, expected.name);
    assert_eq!(actual.description, expected.description);
    assert_eq!(actual.argument_hint.as_deref(), expected.hint);
    assert_eq!(actual.content, expected.content);
    assert_eq!(actual.file_path, path);
    assert_eq!(actual.source_info.path, path);
    assert_eq!(actual.source_info.source, "local");
    assert_eq!(actual.source_info.scope, expected.scope);
    assert_eq!(actual.source_info.origin, SourceOrigin::TopLevel);
    assert_eq!(
        actual.source_info.base_dir.as_deref(),
        Some(at(root, expected.base).as_str())
    );
}

/// One file-system fixture and its expected loaded records.
#[cfg(unix)]
struct LoadCase<'a> {
    /// Fixture entries below the case root.
    entries: &'a [(&'a str, Node<'a>)],
    /// Process directory.
    process: &'a str,
    /// Caller-selected roots/paths.
    options: LoadPromptTemplatesOptions<'a>,
    /// Authored explicit paths.
    paths: &'a [&'a str],
    /// Complete retained records.
    expected: &'a [ExpectedTemplate<'a>],
}

/// Run each case below a fresh root, in a process of this test's own, since every case
/// moves the process directory.
#[cfg(unix)]
fn run_cases(test: &str, cases: &[LoadCase<'_>]) {
    if maestro_test_process::run_alone(test).unwrap() {
        return;
    }
    for case in cases {
        let dir = support::Directory::new();
        let paths: Vec<&str> = case.entries.iter().map(|(path, _)| *path).collect();
        let folded = dir.0.folded_names(&paths).unwrap();
        if !folded.is_empty() {
            eprintln!("{test} case needs names this host keeps apart: {folded:?}");
            continue;
        }
        let root = support::text(dir.0.path());
        let Some(_locks) = materialize(root, case.entries) else {
            continue;
        };
        let process = at(root, case.process);
        std::fs::create_dir_all(&process).unwrap();
        std::env::set_current_dir(&process).unwrap();
        let paths: Vec<String> = case.paths.iter().map(|path| at(root, path)).collect();
        let [cwd, home, agent_dir] =
            [case.options.cwd, case.options.home, case.options.agent_dir].map(|p| at(root, p));
        let actual = load_prompt_templates(LoadPromptTemplatesOptions {
            cwd: &cwd,
            home: &home,
            agent_dir: &agent_dir,
            prompt_paths: &paths,
            ..case.options
        })
        .unwrap();
        assert_records(&actual, case.expected, root);
    }
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
        let folder = dir.0.path().join("native");
        let paths = vec![support::text(&folder).into()];
        let actual = load_prompt_templates(LoadPromptTemplatesOptions {
            cwd: support::text(dir.0.path()),
            home: support::text(dir.0.path()),
            agent_dir: support::text(dir.0.path()),
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        })
        .unwrap();
        let paths: Vec<_> = expected
            .iter()
            .map(|t| support::under(dir.0.path(), t.path))
            .collect();
        let bases: Vec<_> = expected
            .iter()
            .map(|t| support::under(dir.0.path(), t.base))
            .collect();
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
            assert_record(record, expected, "");
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
#[cfg(unix)]
const TEMPLATE_DESCRIPTIONS_CHOOSE_AUTHORED_TEXT_OR_FIRST_NONBLANK_LINE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("\n \t\n  first line  \nsecond")),
        )],
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
#[cfg(unix)]
#[test]
fn template_descriptions_choose_authored_text_or_first_nonblank_line() {
    run_cases(
        "template_descriptions_choose_authored_text_or_first_nonblank_line",
        TEMPLATE_DESCRIPTIONS_CHOOSE_AUTHORED_TEXT_OR_FIRST_NONBLANK_LINE_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
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
#[cfg(unix)]
#[test]
fn prompt_metadata_selects_named_fields_without_whole_record_validation() {
    run_cases(
        "prompt_metadata_selects_named_fields_without_whole_record_validation",
        PROMPT_METADATA_SELECTS_NAMED_FIELDS_WITHOUT_WHOLE_RECORD_VALIDATION_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_METADATA_PARSE_FAILURES_OMIT_THE_FILE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "---\ndescription: first\ndescription: last\n---\nbody",
            )),
        )],
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
#[cfg(unix)]
#[test]
fn prompt_metadata_parse_failures_omit_the_file() {
    run_cases(
        "prompt_metadata_parse_failures_omit_the_file",
        PROMPT_METADATA_PARSE_FAILURES_OMIT_THE_FILE_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_METADATA_OMITS_NONSTRING_SELECTED_FIELDS_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some("---\ndescription: false\n---\nbody")),
        )],
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
#[cfg(unix)]
#[test]
fn prompt_metadata_omits_nonstring_selected_fields() {
    run_cases(
        "prompt_metadata_omits_nonstring_selected_fields",
        PROMPT_METADATA_OMITS_NONSTRING_SELECTED_FIELDS_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const TEMPLATE_DESCRIPTION_LIMIT_KEEPS_WHOLE_SCALARS_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[(
            "/outside/a.md",
            Node::File(Some(
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            )),
        )],
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
#[cfg(unix)]
#[test]
fn template_description_limit_keeps_whole_scalars() {
    run_cases(
        "template_description_limit_keeps_whole_scalars",
        TEMPLATE_DESCRIPTION_LIMIT_KEEPS_WHOLE_SCALARS_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_DISCOVERY_PRESERVES_ORDER_AND_SHALLOW_MARKDOWN_SELECTION_CASES: &[LoadCase<'_>] =
    &[LoadCase {
        entries: &[
            ("/user/prompts", Node::Dir),
            ("/user/prompts/z.md", Node::File(Some("user-z"))),
            ("/user/prompts/.hidden.md", Node::File(Some("hidden"))),
            ("/user/prompts/a.MD", Node::File(None)),
            ("/user/prompts/nested/inner.md", Node::File(Some("nested"))),
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/user/prompts/.md", Node::File(Some("dotname"))),
            ("/user/prompts/twice.md.md", Node::File(Some("twice"))),
            ("/work/.maestro/prompts", Node::Dir),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
            ("/extra", Node::Dir),
            ("/extra/q.md", Node::File(Some("explicit-q"))),
            ("/extra/.gitignore", Node::File(Some("q.md"))),
            ("/extra/.ignore", Node::File(Some("q.md"))),
            ("/extra/.fdignore", Node::File(Some("q.md"))),
        ],
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
                name: ".hidden",
                description: "hidden",
                hint: None,
                content: "hidden",
                path: "/user/prompts/.hidden.md",
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
                name: "a",
                description: "user-a",
                hint: None,
                content: "user-a",
                path: "/user/prompts/a.md",
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
                name: "z",
                description: "user-z",
                hint: None,
                content: "user-z",
                path: "/user/prompts/z.md",
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
#[cfg(unix)]
#[test]
fn prompt_discovery_preserves_order_and_shallow_markdown_selection() {
    run_cases(
        "prompt_discovery_preserves_order_and_shallow_markdown_selection",
        PROMPT_DISCOVERY_PRESERVES_ORDER_AND_SHALLOW_MARKDOWN_SELECTION_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_LOADER_DISABLES_ONLY_DEFAULT_DISCOVERY_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/extra", Node::Dir),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/.gitignore", Node::File(Some("q.md"))),
        ("/extra/.ignore", Node::File(Some("q.md"))),
        ("/extra/.fdignore", Node::File(Some("q.md"))),
    ],
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
#[cfg(unix)]
#[test]
fn prompt_loader_disables_only_default_discovery() {
    run_cases(
        "prompt_loader_disables_only_default_discovery",
        PROMPT_LOADER_DISABLES_ONLY_DEFAULT_DISCOVERY_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const MAESTRO_PROMPT_LOADER_RETAINS_DUPLICATE_PATHS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/extra", Node::Dir),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/.gitignore", Node::File(Some("q.md"))),
        ("/extra/.ignore", Node::File(Some("q.md"))),
        ("/extra/.fdignore", Node::File(Some("q.md"))),
    ],
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
#[cfg(unix)]
#[test]
fn maestro_prompt_loader_retains_duplicate_paths() {
    run_cases(
        "maestro_prompt_loader_retains_duplicate_paths",
        MAESTRO_PROMPT_LOADER_RETAINS_DUPLICATE_PATHS_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_LOADER_FOLLOWS_FILE_LINKS_AND_SKIPS_OTHER_KINDS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/extra", Node::Dir),
        ("/extra/q.md", Node::File(Some("explicit-q"))),
        ("/extra/no.txt", Node::File(None)),
        ("/extra/device.md", Node::Other),
        ("/extra/link.md", Node::Link("/extra/q.md")),
        ("/extra/broken.md", Node::Link("/absent")),
        ("/extra/sub.md", Node::Link("/extra")),
    ],
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
#[cfg(unix)]
#[test]
fn prompt_loader_follows_file_links_and_skips_other_kinds() {
    run_cases(
        "prompt_loader_follows_file_links_and_skips_other_kinds",
        PROMPT_LOADER_FOLLOWS_FILE_LINKS_AND_SKIPS_OTHER_KINDS_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_PROVENANCE_USES_AUTHORED_PATHS_AND_USER_PRECEDENCE_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/user/prompts/a.md", Node::File(Some("user-a"))),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
            ("/user/prompts-more/a.md", Node::File(Some("outside"))),
        ],
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
            ("/work/.maestro/prompts", Node::Dir),
            ("/work/.maestro/prompts/a.md", Node::File(Some("project-a"))),
        ],
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
];

/// Exercise supplied template observations through the public loader.
#[cfg(unix)]
#[test]
fn prompt_provenance_uses_authored_paths_and_user_precedence() {
    run_cases(
        "prompt_provenance_uses_authored_paths_and_user_precedence",
        PROMPT_PROVENANCE_USES_AUTHORED_PATHS_AND_USER_PRECEDENCE_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_PATHS_KEEP_ENTRY_SPECIFIC_RESOLUTION_AND_SPELLING_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/extra/q.md", Node::File(Some("explicit-q"))),
            ("/home/user/t.md", Node::File(Some("home"))),
            ("/home/user", Node::Dir),
            ("/home/user/extra/a.md", Node::File(Some("tildeuser"))),
            ("/work/relative.md", Node::File(Some("relative"))),
        ],
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
            ("/work", Node::Dir),
        ],
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
            ("/ambient/user/prompts", Node::Dir),
            ("/ambient/user/prompts/a.md", Node::File(Some("a"))),
        ],
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
            ("/ambient", Node::Dir),
            ("/ambient/a.md", Node::File(Some("a"))),
            ("/work/.maestro/prompts", Node::Dir),
            ("/work/.maestro/prompts/later.md", Node::File(Some("later"))),
        ],
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
            name: "later",
            description: "later",
            hint: None,
            content: "later",
            path: "/work/.maestro/prompts/later.md",
            scope: SourceScope::Project,
            base: "/work/.maestro/prompts",
        }],
    },
    LoadCase {
        entries: &[("/ambient/work/x.md", Node::File(Some("x")))],
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
            ("/home/user", Node::Dir),
            ("/home/other", Node::Dir),
        ],
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
            ("/work/.custom/prompts", Node::Dir),
            ("/work/.custom/prompts/a.md", Node::File(Some("a"))),
        ],
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
#[cfg(unix)]
#[test]
fn prompt_paths_keep_entry_specific_resolution_and_spelling() {
    run_cases(
        "prompt_paths_keep_entry_specific_resolution_and_spelling",
        PROMPT_PATHS_KEEP_ENTRY_SPECIFIC_RESOLUTION_AND_SPELLING_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_FILE_FAILURES_CONTINUE_THE_SAME_SCAN_CASES: &[LoadCase<'_>] = &[
    LoadCase {
        entries: &[
            ("/scan", Node::Dir),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(None)),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
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
            ("/scan", Node::Dir),
            ("/scan/first.md", Node::File(Some("first"))),
            ("/scan/bad.md", Node::File(Some("---\na: [\n---\nbad"))),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
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
            ("/scan", Node::Dir),
            ("/scan/first.md", Node::File(Some("first"))),
            (
                "/scan/bad.md",
                Node::File(Some("---\ndescription: 42\n---\nbad")),
            ),
            ("/scan/last.md", Node::File(Some("last"))),
            ("/after.md", Node::File(Some("after"))),
        ],
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
#[cfg(unix)]
#[test]
fn prompt_file_failures_continue_the_same_scan() {
    run_cases(
        "prompt_file_failures_continue_the_same_scan",
        PROMPT_FILE_FAILURES_CONTINUE_THE_SAME_SCAN_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_DIRECTORY_FAILURE_ALLOWS_LATER_EXPLICIT_PATHS_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/scan", Node::Locked),
        ("/after.md", Node::File(Some("after"))),
    ],
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
#[cfg(unix)]
#[test]
fn prompt_directory_failure_allows_later_explicit_paths() {
    run_cases(
        "prompt_directory_failure_allows_later_explicit_paths",
        PROMPT_DIRECTORY_FAILURE_ALLOWS_LATER_EXPLICIT_PATHS_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const PROMPT_LINK_METADATA_FAILURE_SKIPS_ONLY_THAT_LINK_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[
        ("/scan", Node::Dir),
        ("/scan/first.md", Node::File(Some("first"))),
        ("/scan/bad.md", Node::Link("/locked/first.md")),
        ("/scan/last.md", Node::File(Some("last"))),
        ("/after.md", Node::File(Some("after"))),
        ("/locked/first.md", Node::File(Some("first"))),
        ("/locked", Node::Locked),
    ],
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
#[cfg(unix)]
#[test]
fn prompt_link_metadata_failure_skips_only_that_link() {
    run_cases(
        "prompt_link_metadata_failure_skips_only_that_link",
        PROMPT_LINK_METADATA_FAILURE_SKIPS_ONLY_THAT_LINK_CASES,
    );
}

/// Distinct supplied filesystem inputs with complete expected records.
#[cfg(unix)]
const ABSENT_PROMPT_ROOTS_YIELD_AN_EMPTY_LIST_CASES: &[LoadCase<'_>] = &[LoadCase {
    entries: &[],
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
#[cfg(unix)]
#[test]
fn absent_prompt_roots_yield_an_empty_list() {
    run_cases(
        "absent_prompt_roots_yield_an_empty_list",
        ABSENT_PROMPT_ROOTS_YIELD_AN_EMPTY_LIST_CASES,
    );
}

/// Run the calling test alone in a process whose working directory was removed. Returns
/// a fixture root there, and `None` in the caller once that run passed.
#[cfg(unix)]
fn without_working_directory(test: &str) -> Option<(support::Directory, String)> {
    if maestro_test_process::run_alone(test).unwrap() {
        return None;
    }
    let dir = support::Directory::new();
    let root = support::text(dir.0.path()).to_owned();
    dir.0.enter_removed("gone").unwrap();
    Some((dir, root))
}

/// Anchored operands never observe the unavailable process directory.
#[cfg(unix)]
#[test]
fn anchored_prompt_paths_do_not_read_the_process_directory() {
    let Some((dir, root)) =
        without_working_directory("anchored_prompt_paths_do_not_read_the_process_directory")
    else {
        return;
    };
    let paths = vec![support::text(&dir.file("outside/ambient-template.md", "body")).into()];
    let [cwd, home, agent_dir] = ["/work", "/home/user", "/user"].map(|p| at(&root, p));
    let actual = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: &cwd,
        home: &home,
        agent_dir: &agent_dir,
        config_dir_name: ".maestro",
        prompt_paths: &paths,
        include_defaults: false,
    })
    .unwrap();
    assert_records(
        &actual,
        &[ExpectedTemplate {
            name: "ambient-template",
            description: "body",
            hint: None,
            content: "body",
            path: "/outside/ambient-template.md",
            scope: SourceScope::Temporary,
            base: "/outside",
        }],
        &root,
    );
}

/// Project-root resolution precedes defaults and explicit-path dispatch.
#[cfg(unix)]
#[test]
fn unanchored_prompt_project_root_propagates_directory_failure() {
    let Some((_dir, root)) =
        without_working_directory("unanchored_prompt_project_root_propagates_directory_failure")
    else {
        return;
    };
    for paths in [vec![at(&root, "/outside/ambient-template.md")], Vec::new()] {
        let error = load_prompt_templates(LoadPromptTemplatesOptions {
            cwd: "relative",
            home: "/home/user",
            agent_dir: "/user",
            config_dir_name: ".maestro",
            prompt_paths: &paths,
            include_defaults: false,
        })
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}

/// A source-resolution failure inside explicit dispatch prevents the file read.
#[cfg(unix)]
#[test]
fn prompt_source_resolution_failure_is_silently_caught() {
    let Some((dir, root)) =
        without_working_directory("prompt_source_resolution_failure_is_silently_caught")
    else {
        return;
    };
    let paths = vec![support::text(&dir.file("outside/ambient-template.md", "body")).into()];
    let actual = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: &at(&root, "/work"),
        home: "/home/user",
        agent_dir: "relative",
        config_dir_name: ".maestro",
        prompt_paths: &paths,
        include_defaults: false,
    })
    .unwrap();
    assert!(actual.is_empty());
}

/// Authored files paired with expected native records.
type NativeCase<'a> = (&'a [(&'a str, &'a str)], &'a [ExpectedTemplate<'a>]);
/// Input command, ordered templates and expected text.
type ExpansionCase<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);

/// Native prompt loading returns ordered source records.
#[cfg(unix)]
#[test]
fn native_prompt_discovery_uses_ordered_directory_entries() {
    let dir = support::Directory::new();
    let z = dir.file("prompts/z.md", "last body");
    let a = dir.file("prompts/a.md", "first body");
    let paths = vec![support::text(a.parent().unwrap()).into()];
    let result = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: support::text(dir.0.path()),
        home: support::text(dir.0.path()),
        agent_dir: "",
        config_dir_name: ".maestro",
        prompt_paths: &paths,
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(
        result
            .iter()
            .map(|p| (
                p.name.as_str(),
                p.content.as_str(),
                p.source_info.path.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("a", "first body", support::text(&a)),
            ("z", "last body", support::text(&z))
        ]
    );
}

/// An empty home spelling resolves `~/` inside the working directory.
#[cfg(unix)]
#[test]
fn empty_home_resolves_an_explicit_prompt_directory_in_the_working_directory() {
    let dir = support::Directory::new();
    let template = dir.file("note.md", "body from the working directory");
    let paths = vec!["~/".to_owned()];
    let result = load_prompt_templates(LoadPromptTemplatesOptions {
        cwd: support::text(dir.0.path()),
        home: "",
        agent_dir: "",
        config_dir_name: ".maestro",
        prompt_paths: &paths,
        include_defaults: false,
    })
    .unwrap();
    assert_eq!(
        result
            .iter()
            .map(|p| (p.name.as_str(), p.content.as_str()))
            .collect::<Vec<_>>(),
        [("note", "body from the working directory")],
        "{template:?}"
    );
}
