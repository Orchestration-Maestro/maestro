//! Policies the analysis commands share with the document and session owners.
#![cfg(not(target_arch = "wasm32"))]

use crate::analysis_fixtures::{
    ZONE, page_data, root, run_cost, run_tools, run_transcripts, run_usage, scratch,
    with_controlled_viewer, with_controlled_viewer_and,
};
use crate::isolated_home::with_environment;
use crate::isolated_home::with_environment_without;

/// An assistant entry carrying a reported cost.
fn cost_entry(provider: &str, total: f64) -> String {
    format!(
        "{{\"type\":\"message\",\"timestamp\":\"2026-03-09T15:00:00.000Z\",\"message\":{{\"role\":\"assistant\",\"provider\":\"{provider}\",\"usage\":{{\"cost\":{{\"total\":{total}}}}}}}}}\n"
    )
}

/// A tool result entry of `units` text characters.
fn tool_result(units: usize) -> String {
    maestro_json::compact_json(&serde_json::json!({
        "type": "message",
        "message": {
            "role": "toolResult",
            "toolCallId": "1",
            "toolName": "read",
            "content": "x".repeat(units),
        },
    }))
    .expect("a recorded entry is JSON")
}

#[test]
fn analysis_file_readers_keep_their_bom_and_parse_error_policies() {
    with_controlled_viewer_and(
        "analysis_file_readers_keep_their_bom_and_parse_error_policies",
        &[("TZ", "America/New_York")],
        |base| {
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let directory = maestro_session::sessions_dir(
                &format!("{base}/home/.maestro/agent/sessions"),
                &project,
            );
            let directory = directory.as_str();
            std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
            // A byte-order mark opens the first line, which the trimming readers parse
            // and the kept-line readers reject.
            std::fs::write(
                format!("{directory}/a.jsonl"),
                format!(
                    "\u{feff}{}\n\n{}",
                    cost_entry("openai", 1.5),
                    tool_result(4).trim_end()
                ),
            )
            .expect("the fixture file is writable");
            let cost = run_cost(&["-d", &project, "-n", "7"]).expect("the cost report runs");
            assert!(cost.succeeded());
            assert!(cost.stdout.contains("openai"), "the trimmed line is parsed");
            assert_eq!(
                cost.stdout.matches("openai").count(),
                2,
                "the day row and the provider total"
            );
            let tools = run_tools(&[
                "--sessions-dir",
                directory,
                "--output",
                &format!("{base}/page.html"),
            ])
            .expect("the tool report runs");
            assert!(tools.succeeded());
            let page =
                std::fs::read_to_string(format!("{base}/page.html")).expect("the page is readable");
            let data = crate::analysis_fixtures::page_data(&page);
            assert_eq!(data["parseErrors"], serde_json::json!(1));
            assert_eq!(data["tools"][0]["name"], serde_json::json!("read"));
            assert_eq!(data["tools"][0]["results"], serde_json::json!(1));
            let usage =
                run_usage(&["--cwd", &project, "--days", "7"]).expect("the usage report runs");
            assert!(usage.succeeded());
            assert_eq!(usage.stdout.matches("assistant").count(), 0);
        },
    );
}

#[test]
fn decimal_day_parsing_reuses_the_integer_owner() {
    with_environment(
        "decimal_day_parsing_reuses_the_integer_owner",
        &[("TZ", "America/New_York")],
        |base| {
            let project = format!("{base}/project");
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let directory = maestro_session::sessions_dir(
                &format!("{base}/home/.maestro/agent/sessions"),
                &project,
            );
            let directory = directory.as_str();
            std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
            // Sixteen days as an explicit decimal count, a trailing word ignored, a
            // decimal prefix and a hexadecimal prefix read as zero rather than sixteen.
            for (days, expected) in [
                ("16", "last 16 days"),
                ("7suffix", "last 7 days"),
                ("+2.9", "last 2 days"),
            ] {
                let run = run_cost(&["-d", &project, "-n", days]).expect("the cost report runs");
                assert!(run.succeeded(), "{days}");
                assert!(run.stdout.contains(expected), "{days}");
            }
            for days in ["0x10", "0"] {
                let run = run_cost(&["-d", &project, "-n", days]).expect("the cost report runs");
                assert!(run.failed(), "{days} is not a positive day count");
            }
        },
    );
}

#[test]
fn analysis_large_reports_keep_every_selected_record() {
    with_controlled_viewer(
        "analysis_large_reports_keep_every_selected_record",
        |_base| {
            for count in [32_usize, 64, 128] {
                let scratch = scratch();
                let base = root(&scratch);
                let directory = format!("{base}/sessions");
                std::fs::create_dir_all(&directory).expect("the fixture directory is creatable");
                let entries = (0..count)
                    .map(|index| {
                        tool_result(index).replace(
                            "\"toolCallId\":\"1\"",
                            &format!("\"toolCallId\":\"{index}\""),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                std::fs::write(format!("{directory}/a.jsonl"), entries)
                    .expect("the fixture file is writable");
                let page = format!("{base}/page.html");
                let run = run_tools(&["--sessions-dir", &directory, "--output", &page])
                    .expect("the tool report runs");
                assert!(run.succeeded());
                let data =
                    crate::analysis_fixtures::page_data(&std::fs::read_to_string(&page).unwrap());
                assert_eq!(data["files"], serde_json::json!(1));
                let read = &data["tools"][0];
                assert_eq!(read["results"], serde_json::json!(count));
                assert_eq!(
                    read["samples"].as_array().map(Vec::len),
                    Some(count),
                    "{count} samples"
                );
                let summed = read["samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap())
                    .sum::<u64>();
                assert_eq!(read["estimatedTokens"], serde_json::json!(summed));
            }
        },
    );
}

#[test]
fn session_roots_follow_the_account_home_and_cost_keeps_the_variable() {
    with_environment_without(
        "session_roots_follow_the_account_home_and_cost_keeps_the_variable",
        &[("TZ", ZONE)],
        &["HOME"],
        |base| {
            // The reports print the working directory they resolved, natively joined.
            let project = maestro_path::join(&[base, "project"]);
            std::fs::create_dir_all(&project).expect("the fixture directory is creatable");
            let account = maestro_resources::paths::native_home_directory()
                .expect("the child has an account home directory")
                .display()
                .to_string();
            let root = format!("{account}/.maestro/agent/sessions");
            let expected = maestro_session::sessions_dir(&root, &project);
            let expected = expected.as_str();
            // The usage report names the directory it resolved from the account home.
            let transcripts =
                run_transcripts(&["--output", &format!("{base}/out"), &project], false)
                    .expect("the transcript command reads its directory");
            assert!(transcripts.failed());
            assert_eq!(
                transcripts.stderr,
                format!("No sessions found for {project}\nExpected: {expected}\n")
            );
            let usage = run_usage(&["--cwd", &project, "--days", "7"])
                .expect("the report reads its directory");
            assert!(usage.failed());
            assert_eq!(
                usage.stderr,
                format!("Sessions directory not found: {expected}\n")
            );
            // The cost report reads the variable directly and reports its absence.
            let cost = run_cost(&["-d", &project, "-n", "7"])
                .expect_err("the variable names no home directory");
            assert_eq!(cost.to_string(), "no home directory");
        },
    );
}

#[test]
fn tool_report_defaults_to_the_home_session_root() {
    with_controlled_viewer("tool_report_defaults_to_the_home_session_root", |base| {
        let root = format!("{base}/home/.maestro/agent/sessions");
        std::fs::create_dir_all(&root).expect("the fixture directory is creatable");
        std::fs::write(format!("{root}/a.jsonl"), tool_result(4))
            .expect("the fixture file is writable");
        let page = format!("{base}/page.html");
        let run = run_tools(&["--output", &page]).expect("the tool report reads its directory");
        assert!(run.succeeded(), "{run:?}");
        let data = page_data(&std::fs::read_to_string(&page).expect("the page is readable"));
        // Without `--sessions-dir` the report reads the home session root, joined natively.
        let root = maestro_path::join(&[&root]);
        assert_eq!(data["sessionsDir"], serde_json::json!(root));
        assert_eq!(data["files"], serde_json::json!(1));
    });
}
