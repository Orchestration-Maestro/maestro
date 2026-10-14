//! Session-document behavior through public export operations.
#![cfg(test)]
mod support;
use maestro_export::*;
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use serde_json::{Value, json};
use std::{collections::VecDeque, io};
use support::*;

#[test]
fn preconditions_precede_render_assets_and_write() {
    let scratch = Scratch::new();
    let session = SessionManager::in_memory(Some("/work")).unwrap();
    let operations = Operations::default();
    assert!(matches!(
        export_session_to_html(&session, None, None, &operations),
        Err(ExportError::InMemorySession)
    ));
    let unpublished =
        SessionManager::create("/work", Some(scratch.0.to_str().unwrap()), "/unused").unwrap();
    let missing = Operations {
        present: false,
        ..Operations::default()
    };
    assert!(matches!(
        export_session_to_html(&unpublished, None, None, &missing),
        Err(ExportError::UnpublishedSession)
    ));
    let empty = View {
        file: Some(String::new()),
        ..View::default()
    };
    assert!(matches!(
        export_session_to_html(&empty, None, None, &operations),
        Err(ExportError::InMemorySession)
    ));
    assert!(operations.events.borrow().is_empty());
    assert!(missing.events.borrow().is_empty());
    let renderer = Renderer::from(Returns {
        call: vec![json!("must not render")],
        result: Vec::new(),
    });
    for file in [None, Some(String::new()), Some("unpublished.jsonl".into())] {
        let view = View {
            file,
            ..call_view("unreached")
        };
        assert!(
            export_session_to_html(
                &view,
                None,
                Some(ExportOptions {
                    tool_renderer: Some(&renderer),
                    ..ExportOptions::default()
                }),
                &missing
            )
            .is_err()
        );
    }
    assert!(renderer.calls.borrow().is_empty());
}
#[test]
fn file_precondition_precedes_session_open() {
    let scratch = Scratch::new();
    let operations = Operations {
        present: false,
        ..Operations::default()
    };
    let path = scratch.path("missing.jsonl");
    assert!(
        matches!(export_from_file(&path, None, &operations), Err(ExportError::MissingFile(ref input)) if input == &path)
    );
    assert!(!std::path::Path::new(&path).exists());
    assert!(operations.events.borrow().is_empty());
}
#[test]
fn complete_tree_and_selected_leaf_survive_export() {
    let scratch = Scratch::new();
    let entries = vec![
        json!({"type":"custom","id":"root","parentId":null,"data":null,"unknown":""}),
        json!({"type":"custom","id":"left","parentId":"root","data":[]}),
        json!({"type":"unknown","id":"right","parentId":"root","data":{"nested":null}}),
    ];
    let path = scratch.session("branched.jsonl", &entries);
    let mut session = SessionManager::open(&path, None, None).unwrap();
    session.branch("left").unwrap();
    let operations = Operations::default();
    export_session_to_html(&session, None, None, &operations).unwrap();
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(data["entries"], json!(entries));
    assert_eq!(data["leafId"], "left");
    assert_eq!(data["header"], *session.get_header().unwrap());
    let view = View::default();
    export_session_to_html(&view, None, None, &operations).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&payload(&html(&operations))).unwrap()["leafId"],
        Value::Null
    );
}
#[test]
fn state_projection_preserves_optional_metadata() {
    for row in corpus().states {
        let tools: Option<Vec<maestro_models::Tool>> = row
            .input
            .get("tools")
            .map(|value| serde_json::from_value(value.clone()).unwrap());
        let state = (row.input.get("absent") != Some(&json!(true))).then_some(ExportState {
            system_prompt: row.input.get("systemPrompt").and_then(Value::as_str),
            tools: tools.as_deref(),
        });
        let operations = Operations::default();
        export_session_to_html(&View::default(), state, None, &operations).unwrap();
        let mut data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
        let object = data.as_object_mut().unwrap();
        for key in ["header", "entries", "leafId"] {
            object.shift_remove(key);
        }
        assert_eq!(data, row.expected);
    }
}
#[test]
fn canonical_payload_is_utf8_base64() {
    document("unicode");
}
#[test]
fn default_names_and_option_routes_preserve_authored_paths() {
    for row in corpus().names {
        let operations = Operations {
            app: row.app_name.unwrap_or_else(|| "maestro".into()),
            ..Operations::default()
        };
        let view = View {
            file: Some(format!("/sessions/{}", row.input)),
            ..View::default()
        };
        assert_eq!(
            export_session_to_html(&view, None, None, &operations).unwrap(),
            row.expected
        );
    }
    let scratch = Scratch::new();
    let path = scratch.session("tree.jsonl", &[]);
    for row in corpus().options {
        let options = if row.input.get("absent") == Some(&json!(true)) {
            None
        } else if let Some(path) = row.input.as_str() {
            Some(ExportOptions::from(path))
        } else {
            Some(ExportOptions {
                output_path: row.input.get("outputPath").and_then(Value::as_str),
                ..ExportOptions::default()
            })
        };
        let operations = Operations::default();
        let result = if row.operation == "session" {
            export_session_to_html(&View::default(), None, options, &operations)
        } else {
            assert_eq!(row.operation, "file");
            export_from_file(&path, options, &operations)
        };
        assert_eq!(result.unwrap(), row.expected);
        assert_eq!(operations.output.borrow().as_ref().unwrap().0, row.expected);
    }
}
#[test]
fn file_export_uses_session_open_and_omits_renderer() {
    let scratch = Scratch::new();
    let entries = vec![
        json!({"type":"message","id":"a","parentId":null,"message":{"role":"assistant","content":[{"type":"toolCall","id":"tool","name":"custom","arguments":{}}]}}),
    ];
    let path = scratch.session("real.jsonl", &entries);
    let renderer = Renderer::from(Returns {
        call: vec![json!({"throw":"unreachable"})],
        result: Vec::new(),
    });
    let operations = Operations::default();
    export_from_file(
        &path,
        Some(ExportOptions {
            tool_renderer: Some(&renderer),
            ..ExportOptions::default()
        }),
        &operations,
    )
    .unwrap();
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(data["entries"], json!(entries));
    assert_eq!(data["leafId"], "a");
    for field in ["systemPrompt", "tools", "renderedTools"] {
        assert!(data.get(field).is_none());
    }
    assert!(renderer.calls.borrow().is_empty());
}
#[test]
fn file_open_recovery_is_visible() {
    let scratch = Scratch::new();
    let empty = scratch.path("empty.jsonl");
    std::fs::write(&empty, "").unwrap();
    let operations = Operations::default();
    export_from_file(&empty, None, &operations).unwrap();
    let recovered = std::fs::read_to_string(&empty).unwrap();
    assert!(recovered.contains("\"type\":\"session\""));
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(data["header"]["type"], "session");
    let failed = Operations::default();
    assert!(matches!(
        export_from_file(scratch.0.to_str().unwrap(), None, &failed),
        Err(ExportError::Io(_))
    ));
    assert!(failed.events.borrow().is_empty());
}
#[test]
fn output_write_overwrites_and_propagates_failure() {
    let scratch = Scratch::new();
    let theme = native_theme(&scratch);
    let bundle = bundle(&scratch, &Operations::default().assets);
    let native = NativeExportOperations {
        app_name: "maestro",
        template_dir: &bundle,
        theme: &theme,
    };
    let input = scratch.session("tree.jsonl", &[]);
    let output = scratch.path("out.html");
    std::fs::write(&output, "old").unwrap();
    export_from_file(&input, Some(output.as_str().into()), &native).unwrap();
    assert!(
        std::fs::read_to_string(&output)
            .unwrap()
            .contains("id=\"session-data\"")
    );
    let missing = scratch.path("absent/out.html");
    assert!(
        matches!(export_from_file(&input, Some(missing.as_str().into()), &native), Err(ExportError::Io(error)) if error.kind() == io::ErrorKind::NotFound)
    );
    assert!(!std::path::Path::new(&scratch.path("absent")).exists());
    assert!(export_from_file(&input, Some(scratch.0.to_str().unwrap().into()), &native).is_err());
    let failing = Operations {
        write_failure: true,
        ..Operations::default()
    };
    assert!(matches!(
        export_session_to_html(&View::default(), None, None, &failing),
        Err(ExportError::Io(_))
    ));
    assert_eq!(failing.events.borrow().last().unwrap(), "write");
    assert_eq!(failing.themes.borrow().len(), 4);
}
#[test]
fn asset_reads_stop_before_theme_on_failure() {
    for (index, row) in corpus().asset_failures.into_iter().enumerate() {
        let scratch = Scratch::new();
        let theme = native_theme(&scratch);
        let directory = bundle(&scratch, &Operations::default().assets);
        std::fs::remove_file(maestro_path::join(&[&directory, &row.asset])).unwrap();
        let native = NativeExportOperations {
            app_name: "maestro",
            template_dir: &directory,
            theme: &theme,
        };
        let observed = NativeObserved {
            native,
            events: std::cell::RefCell::default(),
        };
        let path = scratch.session("tree.jsonl", &[]);
        assert!(
            matches!(export_from_file(&path, None, &observed), Err(ExportError::Io(error)) if error.kind() == io::ErrorKind::NotFound)
        );
        assert_eq!(row.expected.kind, "ENOENT");
        assert_eq!(*observed.events.borrow(), ASSETS[..=index]);
        assert!(row.expected.theme_calls.is_empty());
    }
    let scratch = Scratch::new();
    let theme = native_theme(&scratch);
    let directory = bundle(&scratch, &Operations::default().assets);
    let row = corpus().asset_utf8;
    std::fs::write(maestro_path::join(&[&directory, "template.js"]), row.bytes).unwrap();
    let native = NativeExportOperations {
        app_name: "maestro",
        template_dir: &directory,
        theme: &theme,
    };
    let output = scratch.path("utf8.html");
    let input = scratch.session("tree.jsonl", &[]);
    let view = View {
        file: Some(input),
        ..View::default()
    };
    export_session_to_html(&view, None, Some(output.as_str().into()), &native).unwrap();
    assert_eq!(std::fs::read_to_string(output).unwrap(), row.expected);
}
#[test]
fn theme_observations_are_not_cached() {
    let row = corpus().observations.remove(0);
    let operations = Operations::default();
    *operations.resolved.borrow_mut() = row.resolved.into();
    *operations.explicit.borrow_mut() = row.explicit.into();
    export_session_to_html(&View::default(), None, None, &operations).unwrap();
    assert_eq!(html(&operations), row.expected.html.unwrap());
    assert!(row.expected.error.is_none());
    assert_eq!(*operations.themes.borrow(), row.expected.calls);
}
#[test]
fn theme_failure_follows_renderer_and_precedes_write() {
    for row in corpus().observations.into_iter().skip(1) {
        let operations = Operations::default();
        *operations.resolved.borrow_mut() = row.resolved.into();
        *operations.explicit.borrow_mut() = row.explicit.into();
        let renderer = Renderer::from(Returns {
            call: vec![json!("C")],
            result: Vec::new(),
        });
        let view = call_view("a");
        let result = export_session_to_html(
            &view,
            None,
            Some(ExportOptions {
                tool_renderer: Some(&renderer),
                ..ExportOptions::default()
            }),
            &operations,
        );
        assert_eq!(result.unwrap_err().to_string(), row.expected.error.unwrap());
        assert!(row.expected.html.is_none());
        assert_eq!(*operations.themes.borrow(), row.expected.calls);
        assert_eq!(renderer.calls.borrow().len(), 1);
        assert_eq!(
            *operations.events.borrow(),
            ["Template", "Css", "JavaScript", "Markdown", "Highlight"]
        );
    }
}
#[test]
fn derived_colors_cover_parser_and_luminance_boundaries() {
    for row in corpus().colors {
        let operations = Operations::default();
        *operations.resolved.borrow_mut() = VecDeque::from([json!({"userMessageBg":row.input})]);
        export_session_to_html(&View::default(), None, None, &operations).unwrap();
        let html = html(&operations);
        for (key, value) in row.expected.as_object().unwrap() {
            let property = match key.as_str() {
                "pageBg" => "exportPageBg",
                "cardBg" => "exportCardBg",
                "infoBg" => "exportInfoBg",
                _ => panic!("unexpected background field"),
            };
            assert!(
                html.contains(&format!("--{property}: {};", value.as_str().unwrap())),
                "background differs for {}",
                row.input
            );
        }
    }
}
#[test]
fn explicit_backgrounds_override_independently() {
    for name in [
        "malformed",
        "missing",
        "empty_base",
        "page_only",
        "card_only",
        "info_only",
        "all",
        "empty_explicit",
    ] {
        document(name);
    }
}
#[test]
fn theme_properties_keep_canonical_key_order() {
    document("ordered");
}
#[test]
fn brand_pack_swap_changes_embedded_styling() {
    brand_swap();
}
#[test]
fn document_slots_keep_literal_assets_and_payload() {
    let row = corpus().literal_insertion;
    let mut operations = Operations::default();
    for (index, name) in ASSETS.into_iter().enumerate() {
        operations.assets[index] = row.assets[name].as_str().unwrap().into();
    }
    *operations.resolved.borrow_mut() = VecDeque::from([row.colors]);
    let view = View {
        data: row.data,
        ..View::default()
    };
    export_session_to_html(&view, None, None, &operations).unwrap();
    assert_eq!(html(&operations), row.expected);
    let decoded: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(decoded["header"], view.data["header"]);
    for row in corpus().slot_shapes {
        let operations = Operations {
            assets: [
                row.template,
                CSS.into(),
                "/* application */".into(),
                "/* markdown */".into(),
                "/* highlighting */".into(),
            ],
            ..Operations::default()
        };
        export_session_to_html(&View::default(), None, None, &operations).unwrap();
        assert_eq!(html(&operations), row.expected);
    }
}
#[test]
fn document_shell_keeps_mounts_and_strings() {
    let html = document("empty");
    for text in [
        "Session Export",
        "Open sidebar",
        "Search...",
        "Hide settings entries",
        "Default",
        "Default minus tool results",
        "No-tools",
        "Only user messages",
        "User",
        "Only labeled entries",
        "Labeled",
        "Show everything",
        "All",
        "Close",
        "✕",
        "Resize session tree sidebar",
    ] {
        assert!(html.contains(text));
    }
    for mount in [
        "hamburger",
        "sidebar-overlay",
        "sidebar",
        "tree-search",
        "sidebar-close",
        "tree-container",
        "tree-status",
        "sidebar-resizer",
        "content",
        "header-container",
        "messages",
        "image-modal",
        "modal-image",
        "session-data",
    ] {
        assert!(html.contains(&format!("id=\"{mount}\"")));
    }
    assert!(html.contains("role=\"separator\" aria-orientation=\"vertical\""));
}
#[test]
fn custom_calls_select_only_eligible_tools() {
    record("selection");
}
#[test]
fn custom_call_truth_controls_record_creation() {
    record("call_truth");
    record("empty_call_id_kept");
}
#[test]
fn custom_results_handle_existing_and_orphan_ids() {
    record("existing_record_overrides_template");
    record("results");
}
#[test]
fn repeated_ids_replace_calls_and_merge_results() {
    record("call_replaces_result");
    record("result_clears_absent_fields");
    record("absent_result_keeps_call");
}
#[test]
fn repeated_results_restore_declared_field_order() {
    let mut view = call_view("x");
    for _ in 0..2 {
        view.data["entries"].as_array_mut().unwrap().push(json!({
            "type": "message",
            "message": {"role": "toolResult", "toolCallId": "x", "toolName": "custom"}
        }));
    }
    let renderer = Renderer::from(Returns {
        call: vec![json!("C")],
        result: vec![
            json!({"expanded": "E1"}),
            json!({"collapsed": "C2", "expanded": "E2"}),
        ],
    });
    let operations = Operations::default();
    export_session_to_html(
        &view,
        None,
        Some(ExportOptions {
            tool_renderer: Some(&renderer),
            ..ExportOptions::default()
        }),
        &operations,
    )
    .unwrap();
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(
        serde_json::to_string(&data["renderedTools"]).unwrap(),
        r#"{"x":{"callHtml":"C","resultHtmlCollapsed":"C2","resultHtmlExpanded":"E2"}}"#
    );
}
#[test]
fn rendered_record_fields_preserve_missing_and_empty() {
    record("absence_and_null");
}
#[test]
fn rendered_keys_follow_first_insertion_and_numeric_order() {
    record("ordered_keys");
}
#[test]
fn direct_renderer_failure_aborts_export() {
    record("call_failure");
    record("result_failure");
}
#[test]
fn late_invalid_record_keeps_prior_renderer_effects() {
    record("malformed_later_record");
    record("malformed_later_block");
    let mut view = call_view("a");
    view.data["entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"message","message":null}));
    let renderer = Renderer::from(Returns {
        call: vec![json!({"throw":"earlier renderer failure"})],
        result: Vec::new(),
    });
    let operations = Operations::default();
    let error = export_session_to_html(
        &view,
        None,
        Some(ExportOptions {
            tool_renderer: Some(&renderer),
            ..ExportOptions::default()
        }),
        &operations,
    )
    .unwrap_err();
    assert!(matches!(error, ExportError::Renderer(_)));
    assert_eq!(renderer.calls.borrow().len(), 1);
    assert!(operations.events.borrow().is_empty());
}
#[test]
fn ordinary_call_ids_cannot_access_prototypes() {
    record("prototype_id_call");
    record("prototype_id_result");
}
#[test]
fn outputs_and_bundles_swap_without_caller_changes() {
    adapter_swap();
}
#[test]
fn empty_renderer_map_is_omitted_without_filtering_entries() {
    let view = call_view("a");
    let renderer = Renderer::from(Returns::default());
    for supplied in [None, Some(&renderer as &dyn ToolHtmlRenderer)] {
        let operations = Operations::default();
        export_session_to_html(
            &view,
            None,
            Some(ExportOptions {
                tool_renderer: supplied,
                ..ExportOptions::default()
            }),
            &operations,
        )
        .unwrap();
        let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
        assert!(data.get("renderedTools").is_none());
        assert_eq!(data["entries"], view.data["entries"]);
    }
    let empty = Renderer::from(Returns {
        call: Vec::new(),
        result: vec![json!({})],
    });
    let result_view = View {
        data: json!({"header":null,"entries":[{"type":"message","message":{"role":"toolResult","toolCallId":"empty","toolName":"custom"}}],"leafId":null}),
        ..View::default()
    };
    let operations = Operations::default();
    export_session_to_html(
        &result_view,
        None,
        Some(ExportOptions {
            tool_renderer: Some(&empty),
            ..ExportOptions::default()
        }),
        &operations,
    )
    .unwrap();
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(data["renderedTools"], json!({"empty":{}}));
}
#[test]
fn public_options_forward_theme_and_renderer() {
    let scratch = Scratch::new();
    let path = scratch.session("tree.jsonl", &[]);
    for name in ["bright", "", "dark"] {
        let renderer = Renderer::from(Returns {
            call: vec![json!("CALL")],
            result: Vec::new(),
        });
        let operations = Operations::default();
        let options = ExportOptions {
            output_path: Some("chosen.html"),
            theme_name: Some(name),
            tool_renderer: Some(&renderer),
        };
        assert_eq!(
            export_session_to_html(&call_view("a"), None, Some(options), &operations).unwrap(),
            "chosen.html"
        );
        assert_eq!(renderer.calls.borrow().len(), 1);
        assert_eq!(
            *operations.themes.borrow(),
            vec![
                json!(["resolved", name]),
                json!(["export", name]),
                json!(["resolved", name]),
                json!(["export", name])
            ]
        );
        let operations = Operations::default();
        let options = ExportOptions {
            output_path: Some("file.html"),
            theme_name: Some(name),
            tool_renderer: Some(&renderer),
        };
        assert_eq!(
            export_from_file(&path, Some(options), &operations).unwrap(),
            "file.html"
        );
        assert_eq!(renderer.calls.borrow().len(), 1);
        assert_eq!(
            *operations.themes.borrow(),
            vec![
                json!(["resolved", name]),
                json!(["export", name]),
                json!(["resolved", name]),
                json!(["export", name])
            ]
        );
    }
}

#[path = "component_support/mod.rs"]
#[allow(dead_code)]
mod component_support;

#[test]
fn standard_renderer_integrates_with_document_export() {
    use maestro_tui::Component;
    use std::rc::Rc;
    struct Row;
    impl Component for Row {
        fn render(&self, _: usize) -> Vec<String> {
            vec!["custom result".into()]
        }
    }
    let theme = component_support::theme();
    let mut tool = component_support::definition::<(), Value, Value, Value>();
    tool.render_result = Some(maestro_tools::ToolRenderResult::Typed(Rc::new(
        |_, _, _, _| Ok(Rc::new(Row)),
    )));
    let tool = Rc::new(tool);
    for available in [true, false] {
        let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
            get_tool_definition: |_: &str| Ok(available.then(|| Rc::clone(&tool))),
            theme: &theme,
            cwd: "/work",
            width: None,
            environment: component_support::environment(),
        });
        let view = View {
            data: json!({"header":null,"leafId":null,"entries":[{"type":"message","message":{"role":"toolResult","toolCallId":"id","toolName":"custom","content":[],"isError":false}}]}),
            ..View::default()
        };
        let operations = Operations::default();
        export_session_to_html(
            &view,
            None,
            Some(ExportOptions {
                tool_renderer: Some(&renderer),
                ..ExportOptions::default()
            }),
            &operations,
        )
        .unwrap();
        let decoded: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
        if available {
            assert_eq!(
                decoded["renderedTools"],
                json!({"id":{"resultHtmlExpanded":"<div class=\"ansi-line\">custom result</div>"}})
            );
        } else {
            assert!(decoded.get("renderedTools").is_none());
        }
    }
}
