//! Native command-interpreter arguments observed below the shell boundary.
use super::windows_command;
use serde::Deserialize;
use serde_json::Value;
/// Attributed corpus row, with each input decoded by its owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Owning witness.
    test: String,
    /// Typed native-boundary operands.
    input: Value,
    /// Typed native-boundary observation.
    expected: Value,
}
/// Command-interpreter normalization operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Recorded platform.
    platform: String,
    /// Original executable.
    command: String,
    /// Tokens before shell interpretation, with the file path last.
    args: Vec<String>,
    /// Explicit interpreter or default selection.
    comspec: Option<String>,
}
/// Complete observed native launch.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Program, argv and native quoting policy.
    launch: Launch,
}
/// Native shell boundary result, not the shell's subsequent interpretation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    /// Executable to spawn.
    file: String,
    /// Exact unparsed native arguments.
    args: Vec<String>,
    /// Whether the command-line string bypasses Windows argument quoting.
    windows_verbatim_arguments: bool,
}
#[test]
fn external_editor_windows_uses_shell_while_unix_executes_directly() {
    let rows: Vec<Row> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/extension_editor.json"
    ))
    .unwrap();
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| {
            row.test == "external_editor_windows_uses_shell_while_unix_executes_directly"
                && row.input.get("command").is_some()
        })
        .collect();
    assert_eq!(rows.len(), 6);
    for row in rows {
        let mut input: Input = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        assert_eq!(input.platform, "win32");
        let path = input.args.pop().unwrap();
        let command = std::iter::once(input.command)
            .chain(input.args)
            .collect::<Vec<_>>()
            .join(" ");
        let (file, args, verbatim) = windows_command(&command, &path, input.comspec.as_deref());
        assert_eq!(file, expected.launch.file);
        assert_eq!(args, expected.launch.args);
        assert_eq!(verbatim, expected.launch.windows_verbatim_arguments);
    }
}

#[test]
fn external_editor_windows_keeps_temp_path_one_literal_operand() {
    let path = r"C:\Temp Dir & files\editor's.md";
    let (_, args, verbatim) = windows_command("editor --wait", path, None);
    assert!(verbatim);
    assert_eq!(
        args,
        [
            "/d",
            "/s",
            "/c",
            r#""editor --wait "C:\Temp Dir & files\editor's.md"""#
        ]
    );
    let (_, args, verbatim) = windows_command("editor --wait", path, Some("bash.exe"));
    assert!(!verbatim);
    assert_eq!(
        args,
        ["-c", r"editor --wait 'C:\Temp Dir & files\editor'\''s.md'"]
    );
}
