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
    let fixed = format!("=== SESSION: {name} ===\n[USER]\n=== END SESSION ===\n")
        .encode_utf16()
        .count();
    message("user", serde_json::json!("x".repeat(units - fixed)))
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
    let project = format!("{base}/{name}");
    std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
    sessions(
        base,
        &project,
        &[
            ("a.jsonl", &sized_transcript("a.jsonl", sizes.0)),
            ("b.jsonl", &sized_transcript("b.jsonl", sizes.1)),
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

/// Install a controlled `maestro` child in `directory` that prints the events and
/// records its argv.
///
/// # Errors
/// Returns the native install or write failure.
#[cfg(unix)]
fn child(directory: &str, exit: i32, receipt: &str) -> std::io::Result<()> {
    let bin = format!("{directory}/bin");
    std::fs::create_dir_all(&bin)?;
    let mut script = String::from("#!/bin/sh\n");
    for event in EVENTS {
        let _ = writeln!(script, "printf '%s\\n' '{event}'");
    }
    let _ = writeln!(
        script,
        "printf '%s\\000' '{MARK}' \"$@\" >> \"{receipt}\"\nprintf 'controlled stderr\\n' >&2\nexit {exit}"
    );
    let staged = format!("{bin}/maestro.staged");
    std::fs::write(&staged, script)?;
    let installer = std::env::var_os("INSTALL").unwrap_or_else(|| "install".into());
    let status = std::process::Command::new(installer)
        .args(["-m", "0700", &staged, &format!("{bin}/maestro")])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("the controlled child is installable"))
    }
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

/// The absolute path of the installer, kept in the isolated child's environment.
#[cfg(unix)]
fn installer() -> String {
    if let Ok(install) = std::env::var("INSTALL") {
        return install;
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("install"))
        .find(|path| path.is_file())
        .expect("install is on the search path")
        .display()
        .to_string()
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
        &[("PATH", path), ("INSTALL", &installer())],
        |base| {
            child(base, 0, &format!("{base}/argv")).expect("the controlled child is installed");
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
                "controlled stderr\ncontrolled stderr\nAgent finished but did not write final summary\n"
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
        &[("PATH", path), ("INSTALL", &installer())],
        |base| {
            child(base, 0, &format!("{base}/argv")).expect("the controlled child is installed");
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
                    "{}Agent finished but did not write final summary\n",
                    "controlled stderr\n".repeat(3)
                )
            );
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_prompts_keep_paths_and_all_instruction_paragraphs() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_prompts_keep_paths_and_all_instruction_paragraphs",
        &[("PATH", path), ("INSTALL", &installer())],
        |base| {
            let receipt = format!("{base}/argv");
            child(base, 0, &receipt).expect("the controlled child is installed");
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
            std::fs::write(
                format!("{output}/session-transcripts-000.summary.txt"),
                "PATTERN",
            )
            .unwrap();
            let run = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(run.succeeded(), "{run:?}");
            let arguments = invocations(&receipt);
            let arguments = &arguments[0];
            assert_eq!(
                arguments[..5],
                ["--mode", "json", "--tools", "read,write", "-p"]
            );
            let prompt = &arguments[5];
            let agents = format!("{base}/home/.maestro/agent/AGENTS.md\n{project}/AGENTS.md");
            assert!(
                prompt.contains(&format!(
                    "STEP 1: Read the existing AGENTS.md file(s) to see what's already encoded:\n{agents}\n\nSTEP 2: READING THE TRANSCRIPT:"
                )),
                "{prompt:?}"
            );
            assert!(prompt.contains(
                "Only after reading the ENTIRE file, perform the analysis and write the summary"
            ));
            assert!(prompt.contains("- Do not include any other text outside this format"));
            assert!(prompt.contains(&format!(
                "The file {output}/session-transcripts-000.txt has "
            )));
            assert!(prompt.contains(&format!(
                "lines. Read it in full using chunked reads, then write your analysis to {output}/session-transcripts-000.summary.txt"
            )));
        },
    );
}

#[cfg(unix)]
#[test]
fn analysis_classifies_spawn_exit_and_missing_summary_outcomes() {
    let path = "$BASE/bin";
    with_environment(
        "analysis_classifies_spawn_exit_and_missing_summary_outcomes",
        &[("PATH", path), ("INSTALL", &installer())],
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
            child(base, 3, &format!("{base}/argv")).expect("the failing child is installed");
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
            child(base, 0, &format!("{base}/argv")).expect("the controlled child is installed");
            let unwritten = run_transcripts(&["--analyze", "--output", &output, &project], false)
                .expect("the command writes its transcripts");
            assert!(unwritten.succeeded(), "{unwritten:?}");
            assert!(
                unwritten.stderr.contains(
                    "Agent finished but did not write session-transcripts-000.summary.txt"
                ),
                "{unwritten:?}"
            );
            assert!(
                unwritten
                    .stdout
                    .contains("No summary files created. Nothing to aggregate."),
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
        &[("PATH", path), ("INSTALL", &installer())],
        |base| {
            let receipt = format!("{base}/argv");
            child(base, 0, &receipt).expect("the controlled child is installed");
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
                .find(|prompt| prompt.contains("You are aggregating"))
                .expect("the aggregation prompt is recorded");
            assert!(
                prompt.contains(&format!(
                    "STEP 2: Read ALL of the following summary files:\n{output}/session-transcripts-000.summary.txt\n{output}/stale.summary.txt"
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
