//! JSONL and HTML exports of the session.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::export_fixtures::{Recording, environment, published};
use crate::support::session_harness::{
    Harness, HarnessOptions, as_recorded, assistant, build_tree, corpus, free_to_change, harness,
    on_task_set, sdk_definition, sdk_tools, tool_definition, user,
};
use base64::Engine as _;
use maestro_app::AgentSession;
use maestro_app::configuration::config::get_theme_directories;
use maestro_export::NativeExportOperations;
use maestro_models::Local;
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_theme::{LiveTheme, ThemeDirectories, ThemeState};
use maestro_tui::{components::Text, tui::ComponentHandle};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Writes JSONL to a nested missing directory and checks the recorded lines.
#[test]
fn jsonl_export_writes_header_and_linear_branch() {
    let expected = corpus("jsonl_export");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let root = h.scratch.path().display().to_string();
        let names = build_tree(&mut h.session.session_manager().borrow_mut());
        let nested = format!("{root}/exports/deep/branch.jsonl");
        let path = h.session.export_to_jsonl(Some(&nested)).unwrap();
        assert_eq!(
            as_recorded(json!(path), &[(&root, "$ROOT")]),
            expected["returned"]
        );
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.ends_with('\n'), expected["endsWithNewline"]);
        assert_eq!(json!(text.split('\n').count() - 1), expected["lineCount"]);
        let lines: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut header = lines[0].clone();
        let keys: Vec<&String> = lines[0].as_object().unwrap().keys().collect();
        let stamp = regex::Regex::new(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$").unwrap();
        assert!(stamp.is_match(header["timestamp"].as_str().unwrap()));
        assert_eq!(header["id"], h.session.session_id());
        header["id"] = json!("$SESSION");
        header["timestamp"] = json!("$ISO");
        header["cwd"] = as_recorded(header["cwd"].clone(), &[(&root, "$ROOT")]);
        header["keys"] = json!(keys);
        assert_eq!(header, expected["header"]);
        let name = |id: &Value| {
            id.as_str().map_or(Value::Null, |id| {
                json!(names.iter().find(|(_, value)| *value == id).unwrap().0)
            })
        };
        let entries: Vec<Value> = lines[1..]
            .iter()
            .map(|entry| {
                let keys: Vec<&String> = entry.as_object().unwrap().keys().collect();
                json!({"id": name(&entry["id"]), "parentId": name(&entry["parentId"]), "keys": keys})
            })
            .collect();
        assert_eq!(json!(entries), expected["entries"]);
    });
}

/// Without a path, the export is named by the current time in the process directory.
#[test]
fn jsonl_export_default_name_uses_process_directory() {
    let expected = corpus("jsonl_export");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let root = h.scratch.path().display().to_string();
        let exports = format!("{root}/exports");
        std::fs::create_dir(&exports).unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&exports).unwrap();
        let path = h.session.export_to_jsonl(None);
        std::env::set_current_dir(original).unwrap();
        let path = path.unwrap();
        let stamp =
            regex::Regex::new(r"session-\d{4}-\d\d-\d\dT\d\d-\d\d-\d\d-\d{3}Z\.jsonl$").unwrap();
        let named = stamp.replace(&path, regex::NoExpand("session-$STAMP.jsonl"));
        assert_eq!(
            as_recorded(json!(named), &[(&root, "$ROOT")]),
            expected["defaultName"]
        );
        assert_eq!(
            std::path::Path::new(&path).exists(),
            expected["defaultExists"]
        );
    });
}

/// The decoded payload of the document written at `path`.
fn payload(path: &str) -> Value {
    let document = std::fs::read_to_string(path).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(document)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// The payload of `session` exported to `out.html` below `root` with recorded assets.
fn exported_payload(session: &AgentSession<Local>, root: &str, live_theme: LiveTheme) -> Value {
    let output = format!("{root}/out.html");
    let path = session
        .export_to_html(
            Some(&output),
            &Recording::default(),
            environment(live_theme),
        )
        .unwrap();
    payload(&path)
}

/// A session over a file whose answer calls `painted`, a registered tool with a custom
/// call rendering that never reads the theme, and `reader`, whose call rendering does.
async fn painted_harness(root: &str) -> Harness {
    let calls = json!([
        {"type": "toolCall", "id": "call-1", "name": "painted", "arguments": {}},
        {"type": "toolCall", "id": "call-2", "name": "reader", "arguments": {}},
    ]);
    let answer = assistant("", 2, &json!({"content": calls, "stopReason": "toolUse"}));
    let mut painted = tool_definition("painted", None, None);
    painted.render_call = Some(Rc::new(|_, _, _| {
        Ok(Rc::new(Text::new("Painted call".into(), 0, 0, None)) as ComponentHandle)
    }));
    let mut reader = tool_definition("reader", None, None);
    reader.render_call = Some(Rc::new(|_, theme, _| {
        let theme = theme.get().map_err(|error| themed(&error))?;
        let text = theme
            .fg(&maestro_theme::ThemeColor::ToolTitle, "Read call")
            .map_err(|error| themed(&error))?;
        Ok(Rc::new(Text::new(text, 0, 0, None)) as ComponentHandle)
    }));
    harness(HarnessOptions {
        session_manager: Some(file_session(root, &[user(&json!("paint"), 1), answer])),
        custom_tools: vec![sdk_definition(painted), sdk_definition(reader)],
        base_tools_override: Some(indexmap::IndexMap::new()),
        ..HarnessOptions::default()
    })
    .await
}

/// The diagnostic a theme failure becomes at a renderer boundary.
fn themed(error: &maestro_theme::ThemeError) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::error(error.to_string())
}

/// `e<position>` of the entry `id` in the session.
fn entry_position(manager: &SessionManager, id: &Value) -> Value {
    let entries = manager.get_entries();
    json!(format!(
        "e{}",
        entries.iter().position(|entry| entry["id"] == *id).unwrap()
    ))
}

/// A file session holding a short exchange.
fn file_session(root: &str, exchange: &[Value]) -> SessionManager {
    let sessions = format!("{root}/sessions");
    let mut manager =
        SessionManager::create(&format!("{root}/project"), Some(&sessions), root).unwrap();
    for message in exchange {
        manager.append_message(message.clone()).unwrap();
    }
    manager
}

#[test]
fn html_export_refuses_memory_session() {
    let expected = corpus("html_export");
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let error = h
            .session
            .export_to_html(None, &Recording::default(), environment(published()))
            .unwrap_err();
        assert_eq!(json!({"error": error.message}), expected["inMemory"]);
    });
}

/// Without a published theme, a session only in memory is still refused first, and a file
/// session renders the custom tools that ignore the theme while one that reads it keeps its
/// standard rendering.
#[test]
fn html_export_reads_theme_only_for_custom_rendering() {
    let expected = corpus("html_export");
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let memory = harness(HarnessOptions::default()).await;
        let error = memory
            .session
            .export_to_html(
                None,
                &Recording::default(),
                environment(LiveTheme::default()),
            )
            .unwrap_err();
        assert_eq!(json!({"error": error.message}), expected["inMemory"]);
        let h = painted_harness(&root).await;
        let data = exported_payload(&h.session, &root, LiveTheme::default());
        let painted = format!("<div class=\"ansi-line\">{:<100}</div>", "Painted call");
        assert_eq!(
            data["renderedTools"],
            json!({"call-1": {"callHtml": painted}})
        );
    });
}

#[test]
fn html_export_carries_prompt_tools_and_theme() {
    let expected = corpus("html_export");
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let manager = file_session(
            &root,
            &[user(&json!("hello"), 1), assistant("hi", 2, &json!({}))],
        );
        let h = harness(HarnessOptions {
            models: vec![("plain", false)],
            settings: json!({"theme": "light"}),
            session_manager: Some(manager),
            custom_tools: sdk_tools(),
            base_tools_override: Some(indexmap::IndexMap::new()),
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .set_active_tools_by_name(&["gamma".into(), "alpha".into()])
            .unwrap();
        let operations = Recording::default();
        std::fs::create_dir(format!("{root}/exports")).unwrap();
        let output = format!("{root}/exports/session.html");
        let path = h
            .session
            .export_to_html(Some(&output), &operations, environment(published()))
            .unwrap();
        assert_eq!(
            as_recorded(json!(path), &[(&root, "$ROOT")]),
            expected["returned"]
        );
        let data = payload(&path);
        let manager = h.session.session_manager().borrow();
        let position = |id: &Value| entry_position(&manager, id);
        let entries: Vec<Value> = data["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| json!({"id": position(&entry["id"]), "type": entry["type"]}))
            .collect();
        let observed = json!({
            "inMemory": expected["inMemory"],
            "returned": expected["returned"],
            "dataKeys": data.as_object().unwrap().keys().collect::<Vec<_>>(),
            "systemPromptIsAgentPrompt": data["systemPrompt"] == h.session.system_prompt(),
            "tools": data["tools"],
            "entries": entries,
            "leafId": position(&data["leafId"]),
            "headerIsSessionHeader": data["header"] == json!(manager.get_header()),
            "hasRenderedTools": data.get("renderedTools").is_some(),
            "themeName": h.session.settings_manager().borrow().get_theme(),
        });
        assert_eq!(observed, expected);
        let themes = operations.themes.borrow();
        assert!(!themes.is_empty() && themes.iter().all(|theme| theme.as_deref() == Some("light")));
        drop(manager);
        assert_eq!(native_payload(&h.session, &root), data);
    });
}

/// The payload `session` exports through the native assets adapter over a bundle whose
/// template is the payload alone, the shipped themes and a scratch custom theme directory.
fn native_payload(session: &AgentSession<Local>, root: &str) -> Value {
    let bundle = format!("{root}/bundle");
    std::fs::create_dir_all(format!("{bundle}/vendor")).unwrap();
    for asset in [
        "template.css",
        "template.js",
        "vendor/marked.min.js",
        "vendor/highlight.min.js",
    ] {
        std::fs::write(format!("{bundle}/{asset}"), "").unwrap();
    }
    std::fs::write(format!("{bundle}/template.html"), "{{SESSION_DATA}}").unwrap();
    let theme = ThemeState::new(ThemeDirectories {
        custom_themes_dir: format!("{root}/themes"),
        ..get_theme_directories().unwrap()
    });
    let native = NativeExportOperations {
        app_name: "maestro",
        template_dir: &bundle,
        theme: &theme,
    };
    let output = format!("{root}/exports/native.html");
    let path = session
        .export_to_html(Some(&output), &native, environment(published()))
        .unwrap();
    payload(&path)
}

/// Each tool description is resolved with the session free to change.
#[test]
fn html_export_resolves_tool_descriptions_outside_locks() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let exchange = [user(&json!("hello"), 1), assistant("hi", 2, &json!({}))];
        let h = harness(HarnessOptions {
            session_manager: Some(file_session(&root, &exchange)),
            custom_tools: sdk_tools(),
            base_tools_override: Some(indexmap::IndexMap::new()),
            ..HarnessOptions::default()
        })
        .await;
        let tool = Rc::clone(&h.session.state().read().unwrap().tools().read().unwrap()[0]);
        // The session owns this description, so it holds the session weakly.
        let session = Rc::downgrade(&h.session);
        tool.write().unwrap().description = Some(Rc::new(move || {
            let free = free_to_change(&session.upgrade().unwrap());
            Ok(if free { "free" } else { "held" }.to_owned())
        }));
        let data = exported_payload(&h.session, &root, published());
        assert_eq!(data["tools"][0]["description"], "free");
    });
}

#[test]
fn html_export_renders_custom_calls_with_the_session_free_to_change() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let call =
            json!([{"type": "toolCall", "id": "call-1", "name": "probing", "arguments": {}}]);
        let answer = assistant("", 2, &json!({"content": call, "stopReason": "toolUse"}));
        let slot = Rc::new(RefCell::new(std::rc::Weak::<AgentSession<Local>>::new()));
        let free = Rc::new(RefCell::new(Vec::new()));
        let (session, seen) = (Rc::clone(&slot), Rc::clone(&free));
        let mut probing = tool_definition("probing", None, None);
        probing.render_call = Some(Rc::new(move |_, _, _| {
            let session = session.borrow().upgrade().unwrap();
            seen.borrow_mut().push(free_to_change(&session));
            Ok(Rc::new(Text::new("Probed call".into(), 0, 0, None)) as ComponentHandle)
        }));
        let h = harness(HarnessOptions {
            session_manager: Some(file_session(&root, &[user(&json!("probe"), 1), answer])),
            custom_tools: vec![sdk_definition(probing)],
            base_tools_override: Some(indexmap::IndexMap::new()),
            ..HarnessOptions::default()
        })
        .await;
        *slot.borrow_mut() = Rc::downgrade(&h.session);
        let data = exported_payload(&h.session, &root, published());
        assert_eq!(*free.borrow(), [true]);
        let rendered = data["renderedTools"]["call-1"].to_string();
        assert!(rendered.contains("Probed call"), "{rendered}");
    });
}

#[test]
fn html_export_renders_registered_tool_calls() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let h = painted_harness(&root).await;
        let data = exported_payload(&h.session, &root, published());
        let rendered = data["renderedTools"]["call-1"].to_string();
        assert!(rendered.contains("Painted call"), "{rendered}");
    });
}
