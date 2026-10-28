//! The tool report over literal synthetic session directories.
#![cfg(not(target_arch = "wasm32"))]

use crate::analysis_fixtures::Run;
use crate::analysis_fixtures::{
    opened, page_data, root, run_tools, scratch, with_controlled_viewer,
};
use serde_json::Value;

/// The shared identity pack the page takes its colors from.
const BRAND: &str = include_str!("../../../assets/brand/brand.json");
/// The pack's own path, used only to place its mark files.
const BRAND_PATH: &str = "assets/brand/brand.json";
/// The help text the command prints before reading anything.
const HELP: &str = "Usage: scripts/tool-stats.ts [--sessions-dir <dir>] [--output <file.html>]\n";

/// One recorded entry line.
fn line(value: impl Into<Value>) -> String {
    maestro_json::compact_json(&value.into()).expect("a recorded entry is JSON")
}

/// A tool call recorded by an assistant message.
fn call(id: &str, name: &str, arguments: impl Into<Value>) -> String {
    let arguments = arguments.into();
    line(serde_json::json!({
        "type": "message",
        "message": {"role": "assistant", "content": [
            {"type": "toolCall", "id": id, "name": name, "arguments": arguments},
        ]},
    }))
}

/// A tool result recorded for `id`.
fn result(id: &str, name: &str, content: impl Into<Value>, error: bool) -> String {
    let content = content.into();
    line(serde_json::json!({
        "type": "message",
        "message": {
            "role": "toolResult",
            "toolCallId": id,
            "toolName": name,
            "content": content,
            "isError": error,
        },
    }))
}

/// Write `files` into `directory`, creating its parents.
fn write(directory: &str, files: &[(&str, String)]) {
    std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
    for (name, content) in files {
        std::fs::write(std::path::Path::new(directory).join(name), content)
            .expect("the fixture file is writable");
    }
}

/// Run the report over `sessions` and read back the page it wrote.
fn report(scratch: &maestro_test_directory::Scratch, sessions: &str) -> (Run, String) {
    let output = maestro_path::join(&[&root(scratch), "report.html"]);
    let run = run_tools(&["--sessions-dir", sessions, "--output", &output])
        .expect("the report reads its directory");
    let page = std::fs::read_to_string(&output).unwrap_or_default();
    (run, page)
}

#[test]
fn tool_arguments_and_recursive_file_selection() {
    with_controlled_viewer("tool_arguments_and_recursive_file_selection", |_base| {
        let help = run_tools(&["--help"]).expect("help is printed");
        assert!(help.succeeded());
        assert_eq!(help.stdout, HELP);
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        write(
            &sessions,
            &[
                (
                    "a.jsonl",
                    call("1", "read", serde_json::json!({"path": "a"})),
                ),
                ("notes.txt", "ignored".to_owned()),
            ],
        );
        write(
            &maestro_path::join(&[&sessions, "nested.jsonl"]),
            &[(
                "b.jsonl",
                call("2", "write", serde_json::json!({"path": "b"})),
            )],
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            maestro_path::join(&[&sessions, "a.jsonl"]),
            maestro_path::join(&[&sessions, "link.jsonl"]),
        )
        .expect("the fixture link is creatable");
        let output = maestro_path::join(&[&root(&scratch), "ignored.html"]);
        let report = maestro_path::join(&[&root(&scratch), "report.html"]);
        let run = run_tools(&[
            "--unknown",
            "value",
            "-o",
            &report,
            "--sessions-dir",
            &sessions,
        ])
        .expect("the report reads its directory");
        assert!(run.succeeded());
        assert_eq!(run.stdout, format!("Wrote {report}\n"));
        let page = std::fs::read_to_string(&report).unwrap();
        let data = page_data(&page);
        assert_eq!(data["files"], serde_json::json!(2));
        assert_eq!(data["sessionsDir"], serde_json::json!(sessions));
        assert_eq!(data["parseErrors"], serde_json::json!(0));
        let names = data["tools"]
            .as_array()
            .expect("tools are listed")
            .iter()
            .map(|row| row["name"].as_str().expect("a row name").to_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["read", "write"]);
        assert!(!std::path::Path::new(&output).exists());
    });
}

#[test]
fn tool_counts_calls_results_errors_and_orphans() {
    with_controlled_viewer("tool_counts_calls_results_errors_and_orphans", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        let entries = [
            call("1", "bash", serde_json::json!({"command": "git status"})),
            call("2", "bash", serde_json::json!({"command": 7})),
            result("1", "bash", serde_json::json!("abcd"), true),
            result("3", "orphan", serde_json::json!("abcd"), false),
            result("1", "bash", serde_json::json!(""), false),
            "not json".to_owned(),
            String::new(),
            line(serde_json::json!({"type": "other", "message": {"role": "assistant"}})),
        ];
        let content = entries.join("\n");
        write(&sessions, &[("a.jsonl", content)]);
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        assert_eq!(data["parseErrors"], serde_json::json!(1));
        let tools = data["tools"].as_array().expect("tools are listed");
        let bash = tools
            .iter()
            .find(|row| row["name"] == serde_json::json!("bash"))
            .expect("the bash row is listed");
        assert_eq!(bash["calls"], serde_json::json!(2));
        assert_eq!(bash["results"], serde_json::json!(2));
        assert_eq!(bash["errors"], serde_json::json!(1));
        assert_eq!(bash["samples"], serde_json::json!([1, 0]));
        let orphan = tools
            .iter()
            .find(|row| row["name"] == serde_json::json!("orphan"))
            .expect("the orphan row is listed");
        assert_eq!(orphan["calls"], serde_json::json!(0));
        assert_eq!(orphan["results"], serde_json::json!(1));
    });
}

#[test]
fn tool_pairing_keeps_last_call_across_files() {
    with_controlled_viewer("tool_pairing_keeps_last_call_across_files", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        write(
            &sessions,
            &[
                (
                    "a.jsonl",
                    call("7", "bash", serde_json::json!({"command": "cargo test"})),
                ),
                (
                    "b.jsonl",
                    [
                        call("7", "bash", serde_json::json!({"command": "cargo build"})),
                        result("7", "bash", serde_json::json!("abcdefgh"), false),
                    ]
                    .join("\n"),
                ),
            ],
        );
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        let bash = data["bashCommands"]
            .as_array()
            .expect("commands are listed")
            .iter()
            .map(|row| {
                (
                    row["name"].as_str().expect("a command name").to_owned(),
                    row["calls"].as_u64(),
                    row["estimatedTokens"].as_u64(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            bash,
            [
                ("cargo build".to_owned(), Some(1), Some(2)),
                ("cargo test".to_owned(), Some(1), Some(0)),
            ]
        );
    });
}

#[test]
fn tool_result_text_keeps_images_and_other_blocks() {
    with_controlled_viewer("tool_result_text_keeps_images_and_other_blocks", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        let blocks = serde_json::json!([
            {"type": "text", "text": "a"},
            {"type": "image", "data": "raw"},
            {"type": "thinking", "thinking": "h"},
            {"type": "text", "text": ""},
            {"type": "text", "text": "b"},
        ]);
        write(
            &sessions,
            &[
                ("a.jsonl", result("1", "read", blocks, false)),
                (
                    "b.jsonl",
                    result("2", "read", serde_json::json!({"text": "ignored"}), false),
                ),
                (
                    "c.jsonl",
                    result(
                        "3",
                        "read",
                        serde_json::json!([{"2": 2, "1": 1, "type": "new"}]),
                        false,
                    ),
                ),
            ],
        );
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        let read = &data["tools"][0];
        // 43 units for the joined blocks, none for an object content, 26 for one block.
        assert_eq!(read["samples"], serde_json::json!([11, 0, 7]));
        assert_eq!(read["estimatedTokens"], serde_json::json!(18));
    });
}

#[test]
fn tool_estimates_tokens_and_bucket_boundaries() {
    with_controlled_viewer("tool_estimates_tokens_and_bucket_boundaries", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        let samples = [0_usize, 1, 4, 5, 200, 32_000 * 4, 32_001 * 4];
        let entries = samples
            .iter()
            .enumerate()
            .map(|(index, units)| {
                result(
                    &index.to_string(),
                    "read",
                    serde_json::json!("x".repeat(*units)),
                    false,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        write(&sessions, &[("a.jsonl", entries)]);
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        assert_eq!(
            data["bucketLabels"],
            serde_json::json!([
                "0-50",
                "50-100",
                "100-250",
                "250-500",
                "500-1000",
                "1000-2000",
                "2000-4000",
                "4000-8000",
                "8000-16000",
                "16000-32000",
                "32000+",
            ])
        );
        let read = &data["tools"][0];
        assert_eq!(
            read["samples"],
            serde_json::json!([0, 1, 1, 2, 50, 32000, 32001])
        );
        assert_eq!(
            read["histogram"],
            serde_json::json!([5, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1])
        );
    });
}

/// The commands whose separators, prefixes, quoting and scripts the report keys.
const SEPARATOR_COMMANDS: [&str; 16] = [
    "",
    "   ",
    "git status",
    "git -C dir status",
    "A=1 B=two sudo cargo test --locked",
    "sudo -u root cmd",
    "echo a && rm x",
    "echo a||rm x",
    "echo a;rm x",
    "echo a|cat",
    "echo a\nrm x",
    "a & b",
    "\"quoted bin\" arg",
    "É=1 cmd",
    "a\u{feff}b",
    "a\u{85}b",
];

#[test]
fn bash_command_keys_keep_prefix_and_separator_rules() {
    with_controlled_viewer(
        "bash_command_keys_keep_prefix_and_separator_rules",
        |_base| {
            let scratch = scratch();
            let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
            let entries = SEPARATOR_COMMANDS
                .iter()
                .enumerate()
                .map(|(index, command)| {
                    call(
                        &index.to_string(),
                        "bash",
                        serde_json::json!({"command": command}),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            write(&sessions, &[("a.jsonl", entries)]);
            let (run, page) = report(&scratch, &sessions);
            assert!(run.succeeded());
            let data = page_data(&page);
            let mut keys = data["bashCommands"]
                .as_array()
                .expect("commands are listed")
                .iter()
                .map(|row| row["name"].as_str().expect("a command name").to_owned())
                .collect::<Vec<_>>();
            keys.sort();
            assert_eq!(
                keys,
                [
                    "\"quoted bin\"",
                    "-u root",
                    "a &",
                    "a b",
                    "a\u{85}b",
                    "cargo test",
                    "echo a",
                    "git",
                    "git status",
                    "unknown",
                    "É=1 cmd",
                ]
            );
        },
    );
}

#[test]
fn bash_command_keys_keep_a_final_command_word() {
    with_controlled_viewer("bash_command_keys_keep_a_final_command_word", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        let commands = ["FOO=bar", "FOO=bar baz", "sudo", "A=1 sudo"];
        let entries = commands
            .iter()
            .enumerate()
            .map(|(index, command)| {
                call(
                    &index.to_string(),
                    "bash",
                    serde_json::json!({"command": command}),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        write(&sessions, &[("a.jsonl", entries)]);
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        let mut keys = data["bashCommands"]
            .as_array()
            .expect("commands are listed")
            .iter()
            .map(|row| {
                (
                    row["name"].as_str().expect("a command name").to_owned(),
                    row["calls"].as_u64().expect("recorded calls"),
                )
            })
            .collect::<Vec<_>>();
        keys.sort();
        // A prefix is stripped only while a command word remains, so `FOO=bar` is the
        // command itself and `A=1 sudo` keeps `sudo` as its command.
        assert_eq!(
            keys,
            [
                ("FOO=bar".to_owned(), 1),
                ("baz".to_owned(), 1),
                ("sudo".to_owned(), 2),
            ]
        );
    });
}

#[test]
fn tool_results_for_empty_commands_keep_no_bash_row() {
    with_controlled_viewer(
        "tool_results_for_empty_commands_keep_no_bash_row",
        |_base| {
            let scratch = scratch();
            let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
            write(
                &sessions,
                &[(
                    "a.jsonl",
                    [
                        call("1", "bash", serde_json::json!({"command": ""})),
                        call("2", "bash", serde_json::json!({"command": "git status"})),
                        result("1", "bash", serde_json::json!("abcdefgh"), false),
                        result("2", "bash", serde_json::json!("abcd"), false),
                    ]
                    .join("\n"),
                )],
            );
            let (run, page) = report(&scratch, &sessions);
            assert!(run.succeeded());
            let data = page_data(&page);
            let bash = data["bashCommands"]
                .as_array()
                .expect("commands are listed");
            // The result answering the empty command is attributed to the tool row only.
            assert_eq!(bash.len(), 1);
            assert_eq!(bash[0]["name"], serde_json::json!("git status"));
            assert_eq!(bash[0]["calls"], serde_json::json!(1));
            assert_eq!(bash[0]["estimatedTokens"], serde_json::json!(1));
            let tools = data["tools"].as_array().expect("tools are listed");
            assert_eq!(tools[0]["name"], serde_json::json!("bash"));
            assert_eq!(tools[0]["results"], serde_json::json!(2));
            assert_eq!(tools[0]["estimatedTokens"], serde_json::json!(3));
        },
    );
}

#[test]
fn tool_rows_sort_stably_and_limit_bash_rows() {
    with_controlled_viewer("tool_rows_sort_stably_and_limit_bash_rows", |_base| {
        let scratch = scratch();
        let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
        let mut entries = Vec::new();
        for index in 0..60 {
            let command = format!("cmd{index:02} arg");
            entries.push(call(
                &format!("c{index:02}"),
                "bash",
                serde_json::json!({"command": command}),
            ));
        }
        entries.push(result("c00", "bash", serde_json::json!("abcdefgh"), false));
        entries.push(result("c01", "bash", serde_json::json!("abcdefgh"), false));
        entries.push(result("c02", "read", serde_json::json!("abcdefgh"), false));
        entries.push(result("c03", "read", serde_json::json!("abcdefgh"), false));
        write(&sessions, &[("a.jsonl", entries.join("\n"))]);
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        let data = page_data(&page);
        let tools = data["tools"].as_array().expect("tools are listed");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], serde_json::json!("bash"));
        assert_eq!(tools[0]["samples"].as_array().map(Vec::len), Some(2));
        assert_eq!(tools[1]["name"], serde_json::json!("read"));
        assert_eq!(tools[1]["avg"].as_f64(), Some(2.0));
        let bash = data["bashCommands"]
            .as_array()
            .expect("commands are listed");
        assert_eq!(bash.len(), 50);
        assert_eq!(bash[0]["name"], serde_json::json!("cmd00 arg"));
        assert_eq!(bash[0]["estimatedTokens"], serde_json::json!(2));
        assert_eq!(bash[1]["name"], serde_json::json!("cmd01 arg"));
        assert_eq!(bash[2]["name"], serde_json::json!("cmd02 arg"));
    });
}

#[test]
fn tool_report_keeps_empty_and_nonempty_page_states() {
    with_controlled_viewer(
        "tool_report_keeps_empty_and_nonempty_page_states",
        |_base| {
            let scratch = scratch();
            let empty = maestro_path::join(&[&root(&scratch), "empty"]);
            std::fs::create_dir_all(&empty).expect("the fixture directory is creatable");
            let (run, page) = report(&scratch, &empty);
            assert!(run.succeeded());
            let data = page_data(&page);
            assert_eq!(data["files"], serde_json::json!(0));
            assert_eq!(data["tools"], serde_json::json!([]));
            assert_eq!(data["bashCommands"], serde_json::json!([]));
            assert!(page.contains("0 session files from <code>"));
            assert!(page.contains("Generated 2026-03-09T16:00:00.000Z."));
            assert!(page.contains("function esc(s){return String(s).replace("));
            assert!(page.contains("row?summary(row):'No data'"));
            assert!(page.contains("<h2 class=\"font-semibold mb-3\">Tool calls</h2>"));
            assert_eq!(page.matches("<canvas").count(), 5);
        },
    );
}

#[test]
fn tool_report_treats_data_and_paths_as_text() {
    with_controlled_viewer("tool_report_treats_data_and_paths_as_text", |_base| {
        let scratch = scratch();
        let name = "sessions<img src=x>";
        // Windows refuses `<` and `>` in a name, so the premise cannot be exercised there.
        if !scratch
            .keeps_distinct(&[std::ffi::OsStr::new(name)])
            .unwrap()
        {
            eprintln!("premise not met: this host refuses the name {name:?}");
            return;
        }
        let sessions = maestro_path::join(&[&root(&scratch), name]);
        write(
            &sessions,
            &[(
                "a.jsonl",
                result(
                    "1",
                    "</script><script>globalThis.payloadExecuted=1</script>",
                    serde_json::json!("literal"),
                    false,
                ),
            )],
        );
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        assert!(page.contains("sessions&lt;img src=x&gt;"));
        assert!(!page.contains("<img src=x>"));
        assert!(!page.contains("</script><script>globalThis.payloadExecuted=1</script>"));
        let data = page_data(&page);
        assert_eq!(
            data["tools"][0]["name"],
            serde_json::json!("</script><script>globalThis.payloadExecuted=1</script>")
        );
        assert_eq!(data["sessionsDir"], serde_json::json!(sessions));
    });
}

#[test]
fn tool_report_uses_shared_brand_and_pinned_asset_links() {
    with_controlled_viewer(
        "tool_report_uses_shared_brand_and_pinned_asset_links",
        |_base| {
            let scratch = scratch();
            let sessions = maestro_path::join(&[&root(&scratch), "sessions"]);
            std::fs::create_dir_all(&sessions).expect("the fixture directory is creatable");
            let (run, page) = report(&scratch, &sessions);
            assert!(run.succeeded());
            assert!(page.contains(
                "<script src=\"https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4\"></script>"
            ));
            assert!(page.contains(
            "<script src=\"https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js\"></script>"
        ));
            assert_eq!(page.matches("cdn.jsdelivr.net").count(), 2);
            let pack = maestro_theme::BrandPack::parse(BRAND_PATH, BRAND)
                .expect("the shared pack is valid");
            let css = pack
                .presentation(maestro_theme::BrandMode::Dark)
                .expect("the dark appearance is defined")
                .css_properties();
            for role in [
                "--maestro-color-background",
                "--maestro-color-panel",
                "--maestro-color-text",
                "--maestro-color-muted",
                "--maestro-color-border",
                "--maestro-color-accent",
                "--maestro-font-display",
                "--maestro-font-body",
                "--maestro-font-mono",
            ] {
                assert!(
                    page.contains(&format!("{role}:")),
                    "the page declares {role}"
                );
            }
            assert!(!page.contains("bg-zinc-950"));
            assert!(!page.contains("#60a5fa"));
            assert!(page.contains(&format!(
                "const accent=\"{}\";",
                css["--maestro-color-accent"]
            )));
        },
    );
}

/// A fixture sessions directory holding one call, its result and a payload name.
fn fixture_records() -> String {
    [
        call("1", "bash", serde_json::json!({"command": "git status"})),
        call("2", "read", serde_json::json!({"path": "a"})),
        result("1", "bash", serde_json::json!("abcdefgh"), false),
        result("2", "read", serde_json::json!("x".repeat(400)), false),
        result(
            "3",
            "</script><script>globalThis.payloadExecuted=1</script>",
            serde_json::json!("literal"),
            false,
        ),
    ]
    .join("\n")
}

#[test]
fn tool_report_page_matches_the_committed_fixture() {
    with_controlled_viewer("tool_report_page_matches_the_committed_fixture", |_base| {
        let scratch = scratch();
        let name = "sessions-<img src=x>";
        // Windows refuses `<` and `>` in a name, so the premise cannot be exercised there.
        if !scratch
            .keeps_distinct(&[std::ffi::OsStr::new(name)])
            .unwrap()
        {
            eprintln!("premise not met: this host refuses the name {name:?}");
            return;
        }
        let sessions = maestro_path::join(&[&root(&scratch), name]);
        write(&sessions, &[("a.jsonl", fixture_records())]);
        let (run, page) = report(&scratch, &sessions);
        assert!(run.succeeded());
        // The scratch root is the only host-specific part of the page.
        let normalized = page.replace(&root(&scratch), "/fixture");
        assert_eq!(
            normalized,
            include_str!("fixtures/analysis/tool-report.html"),
            "regenerate the committed page when the report template changes"
        );
    });
}

#[test]
fn tool_report_opens_the_page_with_the_recorded_arguments() {
    with_controlled_viewer(
        "tool_report_opens_the_page_with_the_recorded_arguments",
        |base| {
            let sessions = maestro_path::join(&[base, "sessions"]);
            write(&sessions, &[("a.jsonl", String::new())]);
            let output = maestro_path::join(&[base, "report.html"]);
            // Help writes no page, so it hands nothing to the opener.
            run_tools(&["--help"]).expect("help is printed");
            assert!(
                opened().is_empty(),
                "a report that wrote no page opens none"
            );
            let run = run_tools(&["--sessions-dir", &sessions, "--output", &output])
                .expect("the report reads its directory");
            assert!(run.succeeded(), "{run:?}");
            assert_eq!(run.stdout, format!("Wrote {output}\n"));
            // The opener receives the page path, and no viewer process starts.
            assert_eq!(
                opened(),
                [output],
                "the opener receives the written page path"
            );
        },
    );
}
