//! The wrapped read tool and the same caller over native and supplied operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use crate::loop_support;
use crate::ls_support::posix_spelling;
use crate::read_support;
use crate::runtime::run;

use loop_support::run_turn;
use maestro_agent::{AgentTool, AgentToolResult};
use maestro_models::DiagnosticCode;
use maestro_tools::{ReadOperations, ReadToolOptions, create_read_tool};
use read_support::*;
use serde_json::{Value, json};
use std::sync::Arc;

/// Run the erased read tool through its executable form.
fn read_through(
    options: ReadToolOptions,
    cwd: &str,
    path: &str,
) -> Result<AgentToolResult, maestro_models::DiagnosticErrorInfo> {
    let tool = create_read_tool(cwd.into(), options);
    run((tool.execute)(
        "id".into(),
        json!({"path": path}),
        None,
        None,
    ))
}

#[test]
fn native_and_supplied_reads_share_the_caller() {
    let dir = Scratch::new().unwrap();
    dir.file("rel.txt", b"native bytes");
    let native = read_through(ReadToolOptions::default(), &dir.text(), "rel.txt").unwrap();
    assert_eq!(
        serde_json::to_value(&native.content).unwrap(),
        json!([text_json("native bytes")])
    );
    assert!(native.details.is_none());

    let ops = Arc::new(ScriptedOps::new(b"supplied bytes"));
    let options = || ReadToolOptions {
        auto_resize_images: None,
        operations: Some(Arc::clone(&ops) as Arc<dyn ReadOperations>),
    };
    let supplied = read_through(options(), "/work", "rel.txt").unwrap();
    assert_eq!(
        serde_json::to_value(&supplied.content).unwrap(),
        json!([text_json("supplied bytes")])
    );
    let log: Vec<_> = ops.log().iter().map(|call| posix_spelling(call)).collect();
    assert_eq!(
        log,
        [
            "access:/work/rel.txt",
            "mime:/work/rel.txt",
            "read:/work/rel.txt"
        ]
    );

    let failing = Arc::new(ScriptedOps::new(b"x").at(Stage::Access, Behavior::Fail, None));
    let options = ReadToolOptions {
        auto_resize_images: None,
        operations: Some(failing as Arc<dyn ReadOperations>),
    };
    assert_eq!(
        read_through(options, "/work", "rel.txt").err(),
        Some(ScriptedOps::failure(Stage::Access))
    );
    let missing = read_through(ReadToolOptions::default(), &dir.text(), "absent.txt")
        .err()
        .unwrap();
    assert_eq!(missing.code, Some(DiagnosticCode::Number(2.0)));
}

/// A tool whose result carries an explicit null in `details`.
fn null_details_tool() -> AgentTool {
    AgentTool {
        description: None,
        definition: maestro_models::Tool {
            name: "nullish".into(),
            description: "explicit null".into(),
            parameters: json!({"type": "object"}).into(),
        },
        label: "nullish".into(),
        prepare_arguments: None,
        execute: Arc::new(|_, _, _, _| {
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: Some(Value::Null),
                    terminate: None,
                })
            })
        }),
        execution_mode: None,
    }
}

/// Run one turn of read and null-details calls and return the tool-result messages as the model receives them.
fn tool_results(dir: &Scratch, calls: &[(&str, &str, Value)]) -> Vec<Value> {
    let read = create_read_tool(dir.text(), ReadToolOptions::default());
    run_turn(vec![read, null_details_tool()], calls).results
}

#[test]
fn wrapped_read_runs_through_raw_agent_loop() {
    let dir = Scratch::new().unwrap();
    dir.file("small.txt", b"Hello, world!\nLine 2");
    dir.file("long.txt", vec!["ab"; 2500].join("\n").as_bytes());
    dir.file("pixel", &base64_png());
    let wire = tool_results(
        &dir,
        &[
            ("small", "read", json!({"path": "small.txt"})),
            ("image", "read", json!({"path": "pixel"})),
            ("long", "read", json!({"path": "long.txt"})),
            ("missing", "read", json!({"path": "missing.txt"})),
            ("null", "nullish", json!({})),
        ],
    );
    assert_eq!(wire.len(), 5);
    assert_eq!(
        wire[0]["content"],
        json!([text_json("Hello, world!\nLine 2")])
    );
    assert!(wire[0].get("details").is_none() && wire[0]["isError"] == json!(false));
    assert_eq!(
        wire[1]["content"][0],
        text_json("Read image file [image/png]")
    );
    assert_eq!(wire[1]["content"][1]["mimeType"], "image/png");
    assert!(wire[1].get("details").is_none());
    let kept = vec!["ab"; 2000].join("\n");
    assert_eq!(
        wire[2]["content"],
        json!([text_json(&format!(
            "{kept}\n\n[Showing lines 1-2000 of 2500. Use offset=2001 to continue.]"
        ))])
    );
    assert_eq!(wire[2]["details"]["truncation"]["truncatedBy"], "lines");
    assert_eq!(wire[2]["details"]["truncation"]["totalLines"], 2500);
    assert_eq!(wire[3]["isError"], json!(true));
    assert_eq!(
        wire[3]["content"],
        json!([text_json(
            &std::fs::read(dir.path().join("missing.txt"))
                .unwrap_err()
                .to_string()
        )])
    );
    assert!(wire[4].as_object().unwrap().contains_key("details"));
    assert_eq!(wire[4]["details"], Value::Null);
}

/// The bytes of a one-pixel PNG.
fn base64_png() -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==")
        .unwrap()
}
