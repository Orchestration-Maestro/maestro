//! Transcript extraction, file splitting and controlled analysis children.
#![cfg(not(target_arch = "wasm32"))]

use crate::analysis_fixtures::run_transcripts;
#[cfg(unix)]
use crate::isolated_home::with_environment;
use crate::isolated_home::with_isolated_home;
use serde_json::Value;
#[cfg(unix)]
use std::fmt::Write;
use std::path::Path;

/// One recorded message line.
fn message(role: &str, content: impl Into<Value>) -> String {
    let content = content.into();
    maestro_json::compact_json(&serde_json::json!({
        "type": "message",
        "message": {"role": role, "content": content},
    }))
    .expect("a recorded entry is JSON")
}

/// A session journal holding one text message of each role.
fn journal(with_roles: bool) -> String {
    if with_roles {
        [
            message("user", serde_json::json!("hello\nworld")),
            message("assistant", serde_json::json!("answer\nagain")),
        ]
        .join("\n")
    } else {
        message("user", serde_json::json!("hello\nworld"))
    }
}

/// The session directory of `cwd` under `base`'s home, holding `files`.
fn sessions(base: &str, cwd: &str, files: &[(&str, &str)]) -> String {
    let directory =
        maestro_session::sessions_dir(&format!("{base}/home/.maestro/agent/sessions"), cwd);
    let directory = directory.as_str();
    std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
    for (name, content) in files {
        std::fs::write(Path::new(directory).join(name), content)
            .expect("the fixture file is writable");
    }
    directory.to_owned()
}

/// A journal whose wrapped transcript is exactly `units` UTF-16 units long.
fn sized_transcript(name: &str, units: usize) -> String {
    sized_transcript_of(name, units, 'x')
}

/// A journal of `units` UTF-16 units whose message text is `unit` repeated.
fn sized_transcript_of(name: &str, units: usize, unit: char) -> String {
    let fixed = format!("=== SESSION: {name} ===\n[USER]\n=== END SESSION ===\n")
        .encode_utf16()
        .count();
    let padding = units - fixed;
    assert!(
        padding.is_multiple_of(unit.len_utf16()),
        "the requested size fits whole `{unit}` units"
    );
    let filler = padding / unit.len_utf16();
    message("user", serde_json::json!(unit.to_string().repeat(filler)))
}

#[test]
fn transcript_arguments_keep_output_and_cwd_selection() {
    with_isolated_home(
        "transcript_arguments_keep_output_and_cwd_selection",
        |base| {
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let directory = sessions(base, &project, &[("a.jsonl", &journal(true))]);
            // The command prints the working directory it resolved, natively joined.
            let first = maestro_path::join(&[base, "first"]);
            let second = maestro_path::join(&[base, "second"]);
            let run = run_transcripts(&["--output", &first, "--ignored", &project], false)
                .expect("the command reads its directory");
            assert!(run.succeeded(), "{run:?}");
            assert!(
                run.stdout
                    .starts_with(&format!("Found 1 session files in {directory}\n"))
            );
            assert!(Path::new(&format!("{first}/session-transcripts-000.txt")).exists());
            assert!(!Path::new(&second).exists());
            // Only the first flag and its value are skipped, so a second output value is
            // the positional path.
            let repeated =
                run_transcripts(&["--output", &first, "--output", &second, &project], false)
                    .expect("the command reads its directory");
            assert!(repeated.failed(), "{repeated:?}");
            assert_eq!(
                repeated.stderr,
                format!(
                    "No sessions found for {second}\nExpected: {}\n",
                    maestro_session::sessions_dir(
                        &format!("{base}/home/.maestro/agent/sessions"),
                        &second
                    )
                    .as_str()
                )
            );
            let positional = run_transcripts(&["--help", "--output", &first, &project], false)
                .expect("the command reads its directory");
            assert!(positional.succeeded(), "{positional:?}");
            let ignored = run_transcripts(&["-h", "--output", &first], false)
                .expect("the command reads its directory");
            assert!(ignored.failed());
            assert!(ignored.stderr.starts_with("No sessions found for "));
            let separator = std::path::MAIN_SEPARATOR;
            assert!(
                ignored
                    .stderr
                    .contains(&format!("{separator}-h\nExpected: "))
            );
        },
    );
}

#[test]
fn transcript_reads_independent_jsonl_and_role_order() {
    with_isolated_home(
        "transcript_reads_independent_jsonl_and_role_order",
        |base| {
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let first = [
                message("user", serde_json::json!("one")),
                "not json".to_owned(),
                message("system", serde_json::json!("ignored")),
                maestro_json::compact_json(&serde_json::json!({"type": "session"})).unwrap(),
                message("assistant", serde_json::json!("two")),
            ]
            .join("\n");
            let second = message("user", serde_json::json!("three"));
            let directory = sessions(base, &project, &[("b.jsonl", &second), ("a.jsonl", &first)]);
            let output = format!("{base}/out");
            let run = run_transcripts(&["--output", &output, &project], false)
                .expect("the command reads its directory");
            assert!(run.succeeded());
            let written = std::fs::read_to_string(format!("{output}/session-transcripts-000.txt"))
                .expect("the transcript file is readable");
            assert_eq!(
                written,
                "=== SESSION: a.jsonl ===\n[USER]\none\n---\n[ASSISTANT]\ntwo\n=== END SESSION ===\n\n=== SESSION: b.jsonl ===\n[USER]\nthree\n=== END SESSION ==="
            );
            assert!(
                run.stdout
                    .contains(&format!("Found 2 session files in {directory}\n"))
            );
        },
    );
}

#[test]
fn transcript_text_blocks_keep_only_nonblank_text() {
    with_isolated_home("transcript_text_blocks_keep_only_nonblank_text", |base| {
        let project = format!("{base}/project");
        std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
        let blocks = serde_json::json!([
            {"type": "text", "text": "a"},
            {"type": "image", "data": "raw"},
            {"type": "thinking", "thinking": "h"},
            {"type": "text", "text": ""},
            {"type": "text", "text": "b"},
        ]);
        let text = [
            message("user", blocks),
            message("user", serde_json::json!("   ")),
            message("user", serde_json::json!("\u{feff}")),
            message("assistant", serde_json::json!("\u{85}kept")),
            message("assistant", serde_json::json!("")),
            message(
                "assistant",
                serde_json::json!([{"type": "text", "text": 12}]),
            ),
        ]
        .join("\n");
        sessions(base, &project, &[("a.jsonl", &text)]);
        let output = format!("{base}/out");
        let run = run_transcripts(&["--output", &output, &project], false)
            .expect("the command reads its directory");
        assert!(run.succeeded());
        let written = std::fs::read_to_string(format!("{output}/session-transcripts-000.txt"))
            .expect("the transcript file is readable");
        assert_eq!(
            written,
            "=== SESSION: a.jsonl ===\n[USER]\na\nb\n---\n[ASSISTANT]\n\u{85}kept\n=== END SESSION ==="
        );
    });
}

#[test]
fn transcript_blocks_keep_only_the_text_discriminant() {
    with_isolated_home(
        "transcript_blocks_keep_only_the_text_discriminant",
        |base| {
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            // A block of another kind may carry a `text` member; only `type: "text"` counts.
            let blocks = || {
                serde_json::json!([
                    {"type": "thinking", "text": "leaked", "thinking": "reasoning"},
                    {"type": "toolCall", "text": "leaked too", "name": "read"},
                    {"type": "text", "text": "kept"},
                ])
            };
            let text = [message("user", blocks()), message("assistant", blocks())].join("\n");
            sessions(base, &project, &[("a.jsonl", &text)]);
            let output = format!("{base}/out");
            let run = run_transcripts(&["--output", &output, &project], false)
                .expect("the command reads its directory");
            assert!(run.succeeded(), "{run:?}");
            let written = std::fs::read_to_string(format!("{output}/session-transcripts-000.txt"))
                .expect("the transcript file is readable");
            assert_eq!(
                written,
                "=== SESSION: a.jsonl ===\n[USER]\nkept\n---\n[ASSISTANT]\nkept\n=== END SESSION ==="
            );
        },
    );
}

/// One split case: two sessions of `sizes`, and the lines it must report.
fn split_case(base: &str, name: &str, sizes: (usize, usize), expected: &[&str]) {
    split_case_of(base, name, sizes, 'x', expected);
}

/// One split case of `unit`-filled sessions, and the lines it must report.
fn split_case_of(base: &str, name: &str, sizes: (usize, usize), unit: char, expected: &[&str]) {
    let project = format!("{base}/{name}");
    std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
    sessions(
        base,
        &project,
        &[
            ("a.jsonl", &sized_transcript_of("a.jsonl", sizes.0, unit)),
            ("b.jsonl", &sized_transcript_of("b.jsonl", sizes.1, unit)),
        ],
    );
    let output = format!("{base}/{name}-out");
    let run = run_transcripts(&["--output", &output, &project], false)
        .expect("the command reads its directory");
    assert!(run.succeeded(), "{name}: {run:?}");
    for line in expected {
        assert!(run.stdout.contains(line), "{name}: {run:?}");
    }
}

#[test]
fn transcript_splits_at_utf16_limit_without_splitting_sessions() {
    with_isolated_home(
        "transcript_splits_at_utf16_limit_without_splitting_sessions",
        |base| {
            // The separator adds two units, so a 99,998-unit pair still shares a file.
            split_case(
                base,
                "joined",
                (50_000, 49_996),
                &[
                    "Wrote session-transcripts-000.txt (99998 chars)\n",
                    "Created 1 transcript file(s)",
                ],
            );
            // One unit more exceeds the limit and starts a second file.
            split_case(
                base,
                "exact",
                (50_001, 49_998),
                &[
                    "Wrote session-transcripts-000.txt (50001 chars)\n",
                    "Wrote session-transcripts-001.txt (49998 chars)\n",
                    "Created 2 transcript file(s)",
                ],
            );
            // A session of exactly the limit still shares nothing with its neighbour.
            split_case(
                base,
                "separate",
                (100_000, 100_000),
                &[
                    "Wrote session-transcripts-000.txt (100000 chars)\n",
                    "Wrote session-transcripts-001.txt (100000 chars)\n",
                    "Created 2 transcript file(s)",
                ],
            );
        },
    );
}

#[test]
fn transcript_splits_by_utf16_units_with_supplementary_characters() {
    with_isolated_home(
        "transcript_splits_by_utf16_units_with_supplementary_characters",
        |base| {
            // A supplementary character is two UTF-16 units, so a character count would
            // pack each pair into one file and report half its size.
            split_case_of(
                base,
                "supplementary-joined",
                (50_000, 49_996),
                '\u{1f600}',
                &[
                    "Wrote session-transcripts-000.txt (99998 chars)\n",
                    "Created 1 transcript file(s)",
                ],
            );
            split_case_of(
                base,
                "supplementary-separate",
                (50_002, 49_998),
                '\u{1f600}',
                &[
                    "Wrote session-transcripts-000.txt (50002 chars)\n",
                    "Wrote session-transcripts-001.txt (49998 chars)\n",
                    "Created 2 transcript file(s)",
                ],
            );
        },
    );
}

#[test]
fn transcript_oversized_session_stays_whole() {
    with_isolated_home("transcript_oversized_session_stays_whole", |base| {
        let project = format!("{base}/project");
        std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
        sessions(
            base,
            &project,
            &[
                ("a.jsonl", &sized_transcript("a.jsonl", 40_000)),
                ("b.jsonl", &sized_transcript("b.jsonl", 100_001)),
                ("c.jsonl", &sized_transcript("c.jsonl", 30_000)),
            ],
        );
        let output = format!("{base}/out");
        let run = run_transcripts(&["--output", &output, &project], false)
            .expect("the command reads its directory");
        assert!(run.succeeded());
        assert!(
            run.stdout
                .contains("Wrote session-transcripts-000.txt (40000 chars)\n")
        );
        assert!(
            run.stdout
                .contains("Wrote session-transcripts-001.txt (100001 chars) - oversized\n")
        );
        assert!(
            run.stdout
                .contains("Wrote session-transcripts-002.txt (30000 chars)\n")
        );
        assert!(run.stdout.contains("Created 3 transcript file(s)"));
        let whole = std::fs::read_to_string(format!("{output}/session-transcripts-001.txt"))
            .expect("the oversized transcript is readable");
        assert!(whole.starts_with("=== SESSION: b.jsonl ===\n[USER]\nxxxx"));
        assert!(whole.ends_with("=== END SESSION ==="));
        assert_eq!(whole.encode_utf16().count(), 100_001);
    });
}

#[test]
fn transcript_empty_missing_and_io_failures() {
    with_isolated_home("transcript_empty_missing_and_io_failures", |base| {
        let project = format!("{base}/project");
        std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
        let output = format!("{base}/out");
        let missing = run_transcripts(&["--output", &output, &project], false)
            .expect("the command reports missing sessions");
        assert!(missing.failed());
        assert!(missing.stderr.starts_with("No sessions found for "));
        assert!(
            Path::new(&output).is_dir(),
            "the output directory is created first"
        );
        sessions(base, &project, &[]);
        let empty = run_transcripts(&["--output", &output, &project], false)
            .expect("the command reports empty sessions");
        assert!(empty.failed());
        assert_eq!(empty.stderr, "No transcripts found\n");
        let other = format!("{base}/other");
        sessions(
            base,
            &other,
            &[("a.jsonl", &message("system", serde_json::json!("x")))],
        );
        let untranscribed = run_transcripts(&["--output", &output, &other], false)
            .expect("the command reports missing transcripts");
        assert!(untranscribed.failed());
        assert_eq!(untranscribed.stderr, "No transcripts found\n");
    });
}

#[test]
fn transcript_directory_lookup_matches_session_writer() {
    with_isolated_home(
        "transcript_directory_lookup_matches_session_writer",
        |base| {
            for project in ["a/b", "c:d", "e\\f"] {
                let cwd = format!("{base}/{project}");
                // Windows refuses `:` in a directory name, so that spelling cannot be
                // exercised there.
                match std::fs::create_dir_all(&cwd) {
                    Ok(()) => {}
                    Err(_) if cfg!(windows) => {
                        eprintln!("premise not met: this host refuses the directory {project:?}");
                        continue;
                    }
                    Err(error) => panic!("the fixture directory is creatable: {error}"),
                }
                sessions(base, &cwd, &[("a.jsonl", &journal(true))]);
                let output = format!("{base}/out");
                let run = run_transcripts(&["--output", &output, &cwd], false)
                    .expect("the command reads its directory");
                assert!(
                    run.succeeded(),
                    "the writer's directory is found for {project}"
                );
                assert!(run.stdout.starts_with("Found 1 session files in "));
            }
        },
    );
}

/// The events the controlled child prints, one JSON object per line.
#[cfg(unix)]
const EVENTS: [&str; 8] = [
    r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"thinking now"}}"#,
    r#"{"type":"tool_execution_start","toolName":"read","args":{"path":"literal file","offset":1,"limit":1000}}"#,
    r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"tail"}}"#,
    r#"{"type":"tool_execution_start","toolName":"write","args":{"path":"out"}}"#,
    r#"{"type":"tool_execution_start","toolName":"custom"}"#,
    r#"{"type":"tool_execution_start","toolName":"read","args":{"path":"literal file"}}"#,
    r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"turn tail"}}"#,
    r#"{"type":"turn_end"}"#,
];

/// Install a controlled `maestro` child below `directory` that prints the events and
/// records its argv.
#[cfg(unix)]
fn child(directory: &str, exit: u8, receipt: &str) {
    let mut output = String::new();
    for event in EVENTS {
        let _ = writeln!(output, "{event}");
    }
    crate::native::command(
        std::path::Path::new(directory).join("bin").as_path(),
        "maestro",
        &serde_json::json!({
            "scenario": "output",
            "stdout": output.as_bytes(),
            "stderr": b"controlled stderr\n",
            "code": exit,
            "argv_log": {"path": receipt, "mark": MARK},
        }),
    );
}

/// Install a controlled `maestro` child below `directory` that appends `text` to `path`
/// before it reports the recorded status.
#[cfg(unix)]
fn child_appending(directory: &str, path: &str, text: &str, exit: u8, receipt: &str) {
    crate::native::command(
        std::path::Path::new(directory).join("bin").as_path(),
        "maestro",
        &serde_json::json!({
            "scenario": "output",
            "stdout": b"".as_slice(),
            "stderr": b"".as_slice(),
            "code": exit,
            "append": {"path": path, "cwd": false, "bytes": text.as_bytes()},
            "argv_log": {"path": receipt, "mark": MARK},
        }),
    );
}

/// The line that opens one recorded invocation.
#[cfg(unix)]
const MARK: &str = "=== invocation ===";

/// The recorded invocations, each a list of its arguments.
#[cfg(unix)]
fn invocations(receipt: &str) -> Vec<Vec<String>> {
    let bytes = std::fs::read(receipt).expect("the child argv is recorded");
    let mut invocations: Vec<Vec<String>> = Vec::new();
    for field in bytes.split(|byte| *byte == 0) {
        let argument = String::from_utf8_lossy(field).into_owned();
        if argument == MARK {
            invocations.push(Vec::new());
        } else if let Some(arguments) = invocations.last_mut() {
            arguments.push(argument);
        }
    }
    invocations
}

/// A prepared project with one session and a selected output directory.
#[cfg(unix)]
fn prepared(base: &str) -> (String, String) {
    let project = format!("{base}/project");
    std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
    sessions(base, &project, &[("a.jsonl", &journal(true))]);
    let output = format!("{base}/out");
    (project, output)
}

#[cfg(unix)]
#[test]
fn analysis_stream_keeps_text_tool_and_turn_branches() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_stream_keeps_text_tool_and_turn_branches",
        &[("PATH", path)],
        |base| {
            child(base, 0, &format!("{base}/argv"));
            let (project, output) = prepared(base);
            std::fs::create_dir_all(&output).expect("the output directory is creatable");
            std::fs::write(
                format!("{output}/session-transcripts-000.summary.txt"),
                "PATTERN",
            )
            .expect("the summary file is writable");
            let plain = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(plain.succeeded(), "{plain:?}");
            assert_eq!(
                plain.stderr,
                "controlled stderr\ncontrolled stderr\nThe agent finished without writing the final summary\n"
            );
            let expected = "  thinking now\n  [read] literal file offset=1 limit=1000\n  tail\n  [write] out\n  [custom] \n  [read] literal file\n  turn tail\n";
            assert_eq!(plain.stdout.matches(expected).count(), 2);
            let colored = run_transcripts(&["--analyze", "--output", &output, &project], true)
                .expect("the command writes its transcripts");
            assert!(colored.stdout.contains("\u{1b}[2m  thinking now\u{1b}[22m"));
            assert!(colored.stdout.contains("\u{1b}[36m  [custom] \u{1b}[39m"));
            assert!(
                colored
                    .stderr
                    .contains("\u{1b}[31mcontrolled stderr\n\u{1b}[39m")
            );
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_runs_each_file_before_aggregation() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_runs_each_file_before_aggregation",
        &[("PATH", path)],
        |base| {
            child(base, 0, &format!("{base}/argv"));
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let large = sized_transcript("a.jsonl", 60_000);
            let other = sized_transcript("b.jsonl", 60_000);
            sessions(base, &project, &[("a.jsonl", &large), ("b.jsonl", &other)]);
            let output = format!("{base}/out");
            std::fs::create_dir_all(&output).expect("the output directory is creatable");
            for index in 0..2 {
                std::fs::write(
                    format!("{output}/session-transcripts-{index:03}.summary.txt"),
                    "PATTERN",
                )
                .expect("the summary file is writable");
            }
            let run = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(run.succeeded(), "{run:?}");
            let analyzing = run
                .stdout
                .lines()
                .filter(|line| line.starts_with("Analyzing "))
                .collect::<Vec<_>>();
            assert_eq!(analyzing.len(), 2);
            let aggregated = run
                .stdout
                .find("Aggregating findings into final summary...")
                .expect("the aggregation step runs");
            assert!(run.stdout.rfind("Analyzing ").unwrap() < aggregated);
            assert!(run.stdout.contains("Created 2 summary files"));
            assert_eq!(
                run.stderr,
                format!(
                    "{}The agent finished without writing the final summary\n",
                    "controlled stderr\n".repeat(3)
                )
            );
        },
    );
}

/// Standard output that hands each finished line to the test before the command continues.
#[cfg(unix)]
struct Lines {
    /// Bytes of the line being written.
    pending: Vec<u8>,
    /// Rendezvous with the test: a send returns once the test has taken the line.
    sender: std::sync::mpsc::SyncSender<String>,
}

#[cfg(unix)]
impl std::io::Write for Lines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&self.pending[..end]).into_owned();
            self.pending.drain(..=end);
            self.sender.send(line).map_err(std::io::Error::other)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The lines the command prints up to and including its next analysis or aggregation header,
/// which must start with `header`.
#[cfg(unix)]
fn printed_through(lines: &std::sync::mpsc::Receiver<String>, header: &str) -> Vec<String> {
    let mut seen = Vec::new();
    loop {
        let line = lines.recv().expect("the command prints the next header");
        let done = line.starts_with("Analyzing ") || line.starts_with("Aggregating ");
        seen.push(line);
        if done {
            assert!(
                seen.last().is_some_and(|last| last.starts_with(header)),
                "{seen:?}"
            );
            return seen;
        }
    }
}

/// A project with two transcript files and a summary file for each, and its output directory.
#[cfg(unix)]
fn two_files_with_summaries(base: &str) -> (String, String) {
    let (project, output) = prepared(base);
    sessions(
        base,
        &project,
        &[
            ("a.jsonl", &sized_transcript("a.jsonl", 60_000)),
            ("b.jsonl", &sized_transcript("b.jsonl", 60_000)),
        ],
    );
    std::fs::create_dir_all(&output).expect("the output directory is creatable");
    for index in 0..2 {
        std::fs::write(
            format!("{output}/session-transcripts-{index:03}.summary.txt"),
            "PATTERN",
        )
        .expect("the summary file is writable");
    }
    (project, output)
}

/// Install a `maestro` child that records `ready` on `log` when it starts, waits on `permit`,
/// prints one text event and the end of its turn, then records `ended` on `log`.
///
/// Every child records on the one FIFO, so its contents are the order in which children start
/// and end.
#[cfg(unix)]
fn held_child(
    base: &str,
    log: &maestro_test_process::gate::Gate,
    permit: &maestro_test_process::gate::Gate,
) {
    let events = format!(
        "{}\n{}\n",
        r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"first"}}"#,
        r#"{"type":"turn_end"}"#
    );
    crate::native::command(
        Path::new(base).join("bin").as_path(),
        "maestro",
        &serde_json::json!({
            "scenario": "hold",
            "ready": log.path(),
            "permit": permit.path(),
            "stdout": events.as_bytes(),
            "stderr": [],
            "append": {
                "path": log.path(),
                "cwd": false,
                "variable": null,
                "bytes": b"ended",
            },
        }),
    );
}

/// Assert the next five bytes any child recorded on `log` are `marker`.
#[cfg(unix)]
fn next_marker(log: &maestro_test_process::gate::Gate, marker: [u8; 5]) {
    assert_eq!(log.take(5).expect("a child records"), marker);
}

#[cfg(unix)]
#[test]
fn analysis_children_run_one_at_a_time_in_file_order_with_the_aggregating_child_last() {
    use maestro_test_process::gate::Gate;
    let path = "$BASE/bin";
    with_environment(
        "analysis_children_run_one_at_a_time_in_file_order_with_the_aggregating_child_last",
        &[("PATH", path)],
        |base| {
            let (project, output) = two_files_with_summaries(base);
            std::thread::scope(|scope| {
                // The gates and the receiver belong to this closure, so a failed assertion
                // releases every child and the command's blocked print.
                let (sender, lines) = std::sync::mpsc::sync_channel(0);
                let log = Gate::new(&Path::new(base).join("log")).expect("the log gate");
                let permit = Gate::new(&Path::new(base).join("permit")).expect("the permit gate");
                held_child(base, &log, &permit);
                let arguments = ["--analyze", "--output", &output, &project].map(str::to_owned);
                let command = scope.spawn(move || {
                    let mut stdout = Lines {
                        pending: Vec::new(),
                        sender,
                    };
                    crate::support::run_io(maestro_app::developer_tools::session_transcripts::main(
                        &arguments,
                        false,
                        &mut stdout,
                        &mut Vec::new(),
                    ))
                });
                // Each release lets exactly one child finish, so the markers arrive in the
                // order the run starts and completes its children. A child is held from its
                // start marker until its release, and the next one starts only after its end
                // marker: a run that starts every child at once usually writes a second start
                // marker before the first end marker. No signal orders a late child's start
                // marker ahead of the first end, so that run can still pass; only a launcher
                // seam would make the check exact. The child's own output is printed only
                // after its end, and the next header only after that output.
                let started = || next_marker(&log, *b"ready");
                let ended = || next_marker(&log, *b"ended");
                printed_through(&lines, "Analyzing session-transcripts-000.txt");
                started();
                permit.release().expect("the first child is released");
                ended();
                let second = printed_through(&lines, "Analyzing session-transcripts-001.txt");
                started();
                assert!(second.contains(&"  first".to_owned()), "{second:?}");
                assert!(
                    second.contains(&"  -> session-transcripts-000.summary.txt".to_owned()),
                    "{second:?}"
                );
                permit.release().expect("the second child is released");
                ended();
                let third = printed_through(&lines, "Aggregating findings");
                started();
                assert!(
                    third.contains(&"  -> session-transcripts-001.summary.txt".to_owned()),
                    "{third:?}"
                );
                permit.release().expect("the aggregating child is released");
                ended();
                while lines.recv().is_ok() {}
                let code = command
                    .join()
                    .expect("the command ends")
                    .expect("the command ran");
                assert_eq!(code, std::process::ExitCode::SUCCESS);
            });
        },
    );
}

/// Assert both recorded prompts name their instruction files and summary paths in order.
#[cfg(unix)]
fn assert_prompt_paths(receipt: &str, agents: &str, output: &str, summaries: &[String]) {
    let arguments = invocations(receipt);
    assert_eq!(arguments.len(), 2);
    for invocation in &arguments {
        assert_eq!(
            invocation[..5],
            ["--mode", "json", "--tools", "read,write", "-p"]
        );
    }
    let prompt = &arguments[0][5];
    assert!(
        prompt.contains(&format!(
            "STEP 1: Open the existing AGENTS.md file(s) to learn which patterns they already hold:\n{agents}\n\nSTEP 2: HOW TO READ THE TRANSCRIPT:"
        )),
        "{prompt:?}"
    );
    assert!(
        prompt.contains("Analyze and write the summary only once the ENTIRE file has been read")
    );
    assert!(prompt.contains("- Add no text outside this format"));
    assert!(prompt.contains(&format!(
        "The file {output}/session-transcripts-000.txt has "
    )));
    assert!(prompt.contains(&format!(
        "lines. Read all of it in chunks, then write your analysis to {}",
        summaries[0]
    )));
    let aggregation = &arguments[1][5];
    assert!(
        aggregation.contains(&format!(
            "STEP 1: Open the existing AGENTS.md file(s) to learn which patterns they already hold:\n{agents}\n\nSTEP 2: Read EVERY summary file listed here:\n{}\n\n",
            summaries.join("\n")
        )),
        "{aggregation:?}"
    );
    assert!(
        aggregation.ends_with(&format!(
            "Write the final summary to {output}/FINAL-SUMMARY.txt"
        )),
        "{aggregation:?}"
    );
}

#[cfg(unix)]
#[test]
fn analysis_prompts_keep_paths_and_all_instruction_paragraphs() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_prompts_keep_paths_and_all_instruction_paragraphs",
        &[("PATH", path)],
        |base| {
            let receipt = format!("{base}/argv");
            child(base, 0, &receipt);
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            std::fs::write(format!("{project}/AGENTS.md"), "rules")
                .expect("the rules are writable");
            std::fs::create_dir_all(format!("{base}/home/.maestro/agent"))
                .expect("the agent directory is creatable");
            std::fs::write(format!("{base}/home/.maestro/agent/AGENTS.md"), "rules")
                .expect("the rules are writable");
            sessions(base, &project, &[("a.jsonl", &journal(true))]);
            let output = format!("{base}/out");
            std::fs::create_dir_all(&output).expect("the output directory is creatable");
            let summaries = [
                format!("{output}/session-transcripts-000.summary.txt"),
                format!("{output}/session-transcripts-001.summary.txt"),
            ];
            for summary in &summaries {
                std::fs::write(summary, "PATTERN").expect("the summary file is writable");
            }
            let run = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(run.succeeded(), "{run:?}");
            let agents = format!("{base}/home/.maestro/agent/AGENTS.md\n{project}/AGENTS.md");
            assert_prompt_paths(&receipt, &agents, &output, &summaries);
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_reports_a_child_summary_only_after_the_child_exits() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_reports_a_child_summary_only_after_the_child_exits",
        &[("PATH", path)],
        |base| {
            let (project, output) = prepared(base);
            let summary = format!("{output}/session-transcripts-000.summary.txt");
            let receipt = format!("{base}/argv");
            // The child writes its summary and fails: its write is counted, its success is not.
            child_appending(base, &summary, "PATTERN\n", 3, &receipt);
            let failed = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(failed.succeeded(), "{failed:?}");
            assert!(
                failed
                    .stderr
                    .contains("Failed to analyze session-transcripts-000.txt"),
                "{failed:?}"
            );
            assert!(
                !failed
                    .stdout
                    .contains("  -> session-transcripts-000.summary.txt"),
                "{failed:?}"
            );
            assert!(
                failed.stdout.contains("Created 1 summary files"),
                "{failed:?}"
            );
            // Without the earlier write, only a child that writes its own summary and exits
            // successfully can produce the reported line.
            std::fs::remove_file(&summary).expect("the earlier summary is removable");
            child_appending(base, &summary, "PATTERN\n", 0, &receipt);
            let written = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(written.succeeded(), "{written:?}");
            assert!(
                written
                    .stdout
                    .contains("  -> session-transcripts-000.summary.txt"),
                "{written:?}"
            );
            assert!(
                written
                    .stdout
                    .contains("Aggregating findings into final summary..."),
                "{written:?}"
            );
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_classifies_spawn_exit_and_missing_summary_outcomes() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_classifies_spawn_exit_and_missing_summary_outcomes",
        &[("PATH", path)],
        |base| {
            std::fs::create_dir_all(format!("{base}/bin"))
                .expect("the child directory is creatable");
            let (project, output) = prepared(base);
            // No child in the search path.
            let missing = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(missing.succeeded(), "{missing:?}");
            assert!(
                missing.stderr.contains("  Failed to spawn maestro: "),
                "{missing:?}"
            );
            // A child that exits nonzero.
            child(base, 3, &format!("{base}/argv"));
            let failed = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(failed.succeeded(), "{failed:?}");
            assert!(
                failed
                    .stderr
                    .contains("Failed to analyze session-transcripts-000.txt"),
                "{failed:?}"
            );
            // A child that succeeds without writing the summary it was asked for.
            child(base, 0, &format!("{base}/argv"));
            let unwritten = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(unwritten.succeeded(), "{unwritten:?}");
            assert!(
                unwritten.stderr.contains(
                    "The agent finished without writing session-transcripts-000.summary.txt"
                ),
                "{unwritten:?}"
            );
            assert!(
                unwritten
                    .stdout
                    .contains("No summary file was written, so there is nothing to combine."),
                "{unwritten:?}"
            );
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_aggregates_existing_summary_files() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_aggregates_existing_summary_files",
        &[("PATH", path)],
        |base| {
            let receipt = format!("{base}/argv");
            child(base, 0, &receipt);
            let (project, output) = prepared(base);
            std::fs::create_dir_all(&output).expect("the output directory is creatable");
            std::fs::write(
                format!("{output}/session-transcripts-000.summary.txt"),
                "PATTERN",
            )
            .expect("the summary file is writable");
            std::fs::write(format!("{output}/stale.summary.txt"), "PATTERN")
                .expect("the stale summary is writable");
            std::fs::write(format!("{output}/other.txt"), "ignored")
                .expect("the unrelated file is writable");
            std::fs::write(format!("{output}/FINAL-SUMMARY.txt"), "PATTERN")
                .expect("the final summary is writable");
            let run = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(run.succeeded(), "{run:?}");
            assert!(run.stdout.contains("Created 2 summary files"));
            assert!(run.stdout.contains("=== Final Summary Created ==="));
            let prompt = invocations(&receipt)
                .into_iter()
                .filter_map(|arguments| arguments.into_iter().nth(5))
                .find(|prompt| prompt.contains("You are combining"))
                .expect("the aggregation prompt is recorded");
            assert!(
                prompt.contains(&format!(
                    "STEP 2: Read EVERY summary file listed here:\n{output}/session-transcripts-000.summary.txt\n{output}/stale.summary.txt"
                )),
                "{prompt:?}"
            );
            assert!(prompt.contains(&format!(
                "Write the final summary to {output}/FINAL-SUMMARY.txt"
            )));
            assert!(!prompt.contains("other.txt"));
        },
    );
}
