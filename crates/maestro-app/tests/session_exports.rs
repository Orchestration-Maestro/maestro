//! JSONL and HTML exports of the session.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, assistant, build_tree, corpus, harness, on_task_set, sdk_registered, sdk_tools,
    tool_definition, user,
};
use base64::Engine as _;
use maestro_export::{ExportAsset, ExportOperations};
use maestro_session::{SessionManager, session_manager::ReadonlySessionManager};
use maestro_theme::{ColorMode, LiveTheme, Theme, ThemeError, ThemeExportColors, ThemeOptions};
use maestro_tools::ToolRenderEnvironment;
use maestro_tui::{TerminalImage, components::Text, tui::ComponentHandle};
use serde_json::{Value, json};
use std::{cell::RefCell, io, rc::Rc};

#[path = "../../maestro-test-terminal/tests/support/manual_runtime.rs"]
#[allow(dead_code)]
mod manual_runtime;

/// Writes JSONL to a nested missing directory and checks the recorded lines.
#[test]
fn jsonl_export_writes_header_and_linear_branch() {
    let expected = corpus("jsonl_export");
    on_task_set(async {
        let h = harness(HarnessOptions::default()).await;
        let root = h.scratch.path().display().to_string();
        let names = build_tree(&mut h.session.session_manager().borrow_mut());
        let nested = format!("{root}/exports/deep/branch.jsonl");
        let path = h.session.export_to_jsonl(Some(&nested)).unwrap();
        assert_eq!(path.replace(&root, "$ROOT"), expected["returned"]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.ends_with('\n'), expected["endsWithNewline"]);
        assert_eq!(text.split('\n').count() - 1, 5);
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
        header["cwd"] = json!(header["cwd"].as_str().unwrap().replace(&root, "$ROOT"));
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
    on_task_set(async {
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
        let named = stamp
            .replace(&path, regex::NoExpand("session-$STAMP.jsonl"))
            .replace(&root, "$ROOT");
        assert_eq!(named, expected["defaultName"]);
        assert_eq!(
            std::path::Path::new(&path).exists(),
            expected["defaultExists"]
        );
    });
}

/// Export assets whose template is the payload alone, with recorded theme lookups.
#[derive(Default)]
struct Recording {
    /// Theme names passed to color lookups.
    themes: RefCell<Vec<Option<String>>>,
}

impl ExportOperations for Recording {
    fn app_name(&self) -> &'static str {
        "maestro"
    }
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String> {
        Ok(if asset == ExportAsset::Template {
            "{{SESSION_DATA}}".into()
        } else {
            String::new()
        })
    }
    fn get_resolved_theme_colors(
        &self,
        theme: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        self.themes.borrow_mut().push(theme.map(str::to_owned));
        Ok(vec![("userMessageBg".into(), "#343541".into())])
    }
    fn get_theme_export_colors(&self, _: Option<&str>) -> ThemeExportColors {
        ThemeExportColors::default()
    }
}

/// The decoded payload of the document written at `path`.
fn payload(path: &str) -> Value {
    let document = std::fs::read_to_string(path).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(document)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// A rendering environment with a published plain theme, as the host publishes one.
fn environment() -> Rc<ToolRenderEnvironment> {
    let theme = Theme::new([], [], ColorMode::Color256, ThemeOptions::default()).unwrap();
    Rc::new(ToolRenderEnvironment {
        runtime: Rc::new(manual_runtime::ManualRuntime::default()),
        readme_path: String::new(),
        live_theme: LiveTheme::from_theme(Rc::new(theme)),
        highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        terminal_image: TerminalImage::default(),
    })
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
    on_task_set(async {
        let h = harness(HarnessOptions::default()).await;
        let error = h
            .session
            .export_to_html(None, &Recording::default(), environment())
            .unwrap_err();
        assert_eq!(json!({"error": error.message}), expected["inMemory"]);
    });
}

#[test]
fn html_export_carries_prompt_tools_and_theme() {
    let expected = corpus("html_export");
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(async {
        let manager = file_session(
            &root,
            &[user(&json!("hello"), 1), assistant("hi", 2, &json!({}))],
        );
        let h = harness(HarnessOptions {
            models: vec![("plain", false)],
            settings: json!({"theme": "light"}),
            session_manager: Some(manager),
            ..HarnessOptions::default()
        })
        .await;
        h.session.set_tool_definitions(sdk_tools());
        h.session
            .set_active_tools_by_name(&["gamma".into(), "alpha".into()])
            .unwrap();
        let operations = Recording::default();
        std::fs::create_dir(format!("{root}/exports")).unwrap();
        let output = format!("{root}/exports/session.html");
        let path = h
            .session
            .export_to_html(Some(&output), &operations, environment())
            .unwrap();
        assert_eq!(path.replace(&root, "$ROOT"), expected["returned"]);
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
    });
}

#[test]
fn html_export_renders_registered_tool_calls() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(async {
        let call = json!({"type": "toolCall", "id": "call-1", "name": "painted", "arguments": {}});
        let answer = assistant("", 2, &json!({"content": [call], "stopReason": "toolUse"}));
        let manager = file_session(&root, &[user(&json!("paint"), 1), answer]);
        let h = harness(HarnessOptions {
            session_manager: Some(manager),
            ..HarnessOptions::default()
        })
        .await;
        let mut painted = tool_definition("painted", None, None);
        painted.render_call = Some(Rc::new(|_, _, _| {
            Ok(Rc::new(Text::new("Painted call".into(), 0, 0, None)) as ComponentHandle)
        }));
        let registered = sdk_registered(painted);
        h.session
            .set_tool_definitions([("painted".to_owned(), registered)].into_iter().collect());
        let output = format!("{root}/out.html");
        let path = h
            .session
            .export_to_html(Some(&output), &Recording::default(), environment())
            .unwrap();
        let rendered = payload(&path)["renderedTools"]["call-1"].to_string();
        assert!(rendered.contains("Painted call"), "{rendered}");
    });
}
