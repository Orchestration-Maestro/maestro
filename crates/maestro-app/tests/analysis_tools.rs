//! The tool report over literal synthetic session directories.
#![cfg(not(target_arch = "wasm32"))]

use crate::analysis_fixtures::{Run, page_data, root, run_tools, scratch};
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
    let output = format!("{}/report.html", root(scratch));
    let run = run_tools(&["--sessions-dir", sessions, "--output", &output])
        .expect("the report reads its directory");
    let page = std::fs::read_to_string(&output).unwrap_or_default();
    (run, page)
}

#[test]
fn tool_arguments_and_recursive_file_selection() {
    let help = run_tools(&["--help"]).expect("help is printed");
    assert!(help.succeeded());
    assert_eq!(help.stdout, HELP);
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
        &format!("{sessions}/nested.jsonl"),
        &[(
            "b.jsonl",
            call("2", "write", serde_json::json!({"path": "b"})),
        )],
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        format!("{sessions}/a.jsonl"),
        format!("{sessions}/link.jsonl"),
    )
    .expect("the fixture link is creatable");
    let output = format!("{}/ignored.html", root(&scratch));
    let run = run_tools(&[
        "--unknown",
        "value",
        "-o",
        &format!("{}/report.html", root(&scratch)),
        "--sessions-dir",
        &sessions,
    ])
    .expect("the report reads its directory");
    assert!(run.succeeded());
    assert_eq!(
        run.stdout,
        format!("Wrote {}/report.html\n", root(&scratch))
    );
    let page = std::fs::read_to_string(format!("{}/report.html", root(&scratch))).unwrap();
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
}

#[test]
fn tool_counts_calls_results_errors_and_orphans() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
}

#[test]
fn tool_pairing_keeps_last_call_across_files() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
}

#[test]
fn tool_result_text_keeps_images_and_other_blocks() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
}

#[test]
fn tool_estimates_tokens_and_bucket_boundaries() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
}

#[test]
fn bash_command_keys_keep_prefix_and_separator_rules() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
    let commands = [
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
}

#[test]
fn tool_rows_sort_stably_and_limit_bash_rows() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
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
}

#[test]
fn tool_report_keeps_empty_and_nonempty_page_states() {
    let scratch = scratch();
    let empty = format!("{}/empty", root(&scratch));
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
}

#[test]
fn tool_report_row_click_updates_selected_histogram() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
    write(
        &sessions,
        &[(
            "a.jsonl",
            [
                call("1", "read", serde_json::json!({"path": "a"})),
                result("1", "read", serde_json::json!("abcd"), false),
                call("2", "bash", serde_json::json!({"command": "git status"})),
                result("2", "bash", serde_json::json!("abcdefgh"), false),
            ]
            .join("\n"),
        )],
    );
    let (run, page) = report(&scratch, &sessions);
    assert!(run.succeeded());
    let data = page_data(&page);
    assert_eq!(data["tools"].as_array().map(Vec::len), Some(2));
    // The click operation selects the row and dispatches its update together.
    assert!(page.contains(
        "row.addEventListener('click',()=>{const select=document.getElementById(el==='tools'?'toolSelect':'bashSelect');select.value=row.dataset.row;select.dispatchEvent(new Event('change'))})"
    ));
    assert!(page.contains("document.getElementById(selectId).addEventListener('change',update)"));
    assert!(page.contains("function singleHistogram(canvasId,summaryId,selectId,rows)"));
}

#[test]
fn tool_report_treats_data_and_paths_as_text() {
    let scratch = scratch();
    let sessions = format!("{}/sessions<img src=x>", root(&scratch));
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
}

#[test]
fn tool_report_uses_shared_brand_and_pinned_asset_links() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
    std::fs::create_dir_all(&sessions).expect("the fixture directory is creatable");
    let (run, page) = report(&scratch, &sessions);
    assert!(run.succeeded());
    assert!(
        page.contains(
            "<script src=\"https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4\"></script>"
        )
    );
    assert!(page.contains(
        "<script src=\"https://cdn.jsdelivr.net/npm/chart.js@4.4.9/dist/chart.umd.min.js\"></script>"
    ));
    assert_eq!(page.matches("cdn.jsdelivr.net").count(), 2);
    let pack =
        maestro_theme::BrandPack::parse(BRAND_PATH, BRAND).expect("the shared pack is valid");
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
}

#[test]
fn tool_report_launches_platform_opener_with_literal_path() {
    let scratch = scratch();
    let sessions = format!("{}/sessions", root(&scratch));
    write(&sessions, &[("a.jsonl", String::new())]);
    let output = format!("{}/report.html", root(&scratch));
    let run = run_tools(&["--sessions-dir", &sessions, "--output", &output])
        .expect("the report reads its directory");
    assert!(run.succeeded());
    assert!(std::path::Path::new(&output).exists());
    assert_eq!(
        run.stdout,
        format!("Wrote {output}\n"),
        "the page is written and reported before the viewer starts"
    );
}
