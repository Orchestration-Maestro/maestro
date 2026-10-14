//! Controlled observations and real-file helpers for export tests.
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_export::*;
use maestro_session::session_manager::{ReadonlySessionManager, SessionTreeNode};
use maestro_theme::{ThemeError, ThemeExportColors};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque, io, path::PathBuf};

/// Read the independent document corpus.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/session_documents.json")).unwrap()
}
/// All recorded groups, each consumed by its named behavior witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Derivation cases.
    pub colors: Vec<ColorCase>,
    /// Custom record cases.
    pub records: Vec<RecordCase>,
    /// Full document cases.
    pub documents: Vec<DocumentCase>,
    /// Output naming cases.
    pub names: Vec<NameCase>,
    /// Optional metadata cases.
    pub states: Vec<InputCase>,
    /// Option-route cases.
    pub options: Vec<OptionCase>,
    /// Changing/failing theme observations.
    pub observations: Vec<ObservationCase>,
    /// Native missing-asset cases.
    pub asset_failures: Vec<AssetFailure>,
    /// Original template slot shapes.
    pub slot_shapes: Vec<SlotCase>,
    /// Invalid UTF-8 native asset.
    pub asset_utf8: Utf8Case,
    /// Literal replacement regression.
    pub literal_insertion: LiteralCase,
}
/// Input and independent expected output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputCase {
    /// Supplied input.
    pub input: Value,
    /// Recorded output.
    pub expected: Value,
}
/// One base color and its expected backgrounds.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorCase {
    /// Base color.
    pub input: String,
    /// All three expected backgrounds.
    pub expected: Value,
}
/// One tool-record walk.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordCase {
    /// Diagnostic case selection.
    pub name: String,
    /// Raw retained entries.
    pub entries: Vec<Value>,
    /// Queued callback outputs.
    pub returns: Returns,
    /// Output and callback observations.
    pub expected: RecordExpected,
}
/// Callback output queues.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Returns {
    /// Call outputs.
    #[serde(default)]
    pub call: Vec<Value>,
    /// Result outputs.
    #[serde(default)]
    pub result: Vec<Value>,
}
/// A successful output or reached failure and its preceding callbacks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordExpected {
    /// Canonical own-key map text.
    pub json: Option<String>,
    /// Failure kind or renderer text.
    pub error: Option<String>,
    /// Ordered callback inputs.
    pub calls: Vec<Value>,
}
/// Complete document input/output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCase {
    /// Case selector.
    pub name: String,
    /// Borrowed session data and metadata.
    pub data: Value,
    /// Ordered resolved colors.
    pub colors: Value,
    /// Explicit backgrounds.
    pub explicit: Value,
    /// Document, encoded payload and theme observations.
    pub expected: DocumentExpected,
}
/// Independently recorded document result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentExpected {
    /// Exact document output.
    pub html: String,
    /// Exact decoded payload bytes.
    pub payload: String,
    /// Ordered theme observations.
    pub observations: Vec<Value>,
}
/// Filename derivation input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameCase {
    /// Authored input filename.
    pub input: String,
    /// Alternate application name.
    pub app_name: Option<String>,
    /// Output filename.
    pub expected: String,
}
/// Options supplied through either public entry point.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionCase {
    /// Entry point selector.
    pub operation: String,
    /// Shorthand, absent or explicit options.
    pub input: Value,
    /// Returned path.
    pub expected: String,
}
/// Separate theme observations, including failures.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationCase {
    /// Queued resolved colors or failures.
    pub resolved: Vec<Value>,
    /// Queued explicit backgrounds.
    pub explicit: Vec<Value>,
    /// Completed document or failure and call sequence.
    pub expected: ObservationExpected,
}
/// Theme phase output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationExpected {
    /// Successful document.
    pub html: Option<String>,
    /// Theme error.
    pub error: Option<String>,
    /// Prior observations.
    pub calls: Vec<Value>,
}
/// Missing asset input/output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetFailure {
    /// Missing asset path.
    pub asset: String,
    /// Failure classification and theme observations.
    pub expected: AssetExpected,
}
/// Native read failure expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetExpected {
    /// Native error code class.
    pub kind: String,
    /// No reached theme calls.
    #[serde(rename = "themeCalls")]
    pub theme_calls: Vec<Value>,
}
/// Template slot framing.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotCase {
    /// Original template.
    pub template: String,
    /// Literal output.
    pub expected: String,
}
/// Native UTF-8 replacement.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Utf8Case {
    /// Original invalid bytes.
    pub bytes: Vec<u8>,
    /// Complete decoded document.
    pub expected: String,
}
/// Literal asset insertion with marker-looking text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiteralCase {
    /// Asset map.
    pub assets: Value,
    /// Raw session data.
    pub data: Value,
    /// Resolved colors.
    pub colors: Value,
    /// Corrected document.
    pub expected: String,
}

/// Supplied session observations without a replacement file parser.
pub struct View {
    /// Selected filename.
    pub file: Option<String>,
    /// Complete retained payload projection.
    pub data: Value,
}
impl Default for View {
    fn default() -> Self {
        Self {
            file: Some("tree.jsonl".into()),
            data: json!({"header":null,"entries":[],"leafId":null}),
        }
    }
}
impl ReadonlySessionManager for View {
    fn get_cwd(&self) -> &'static str {
        "/work"
    }
    fn get_session_dir(&self) -> &'static str {
        "/sessions"
    }
    fn get_session_id(&self) -> &'static str {
        "session"
    }
    fn get_session_file(&self) -> Option<&str> {
        self.file.as_deref()
    }
    fn get_leaf_id(&self) -> Option<&str> {
        self.data["leafId"].as_str()
    }
    fn get_leaf_entry(&self) -> Option<&Value> {
        self.get_leaf_id().and_then(|id| self.get_entry(id))
    }
    fn get_entry(&self, id: &str) -> Option<&Value> {
        self.get_entries()
            .into_iter()
            .find(|entry| entry["id"] == id)
    }
    fn get_label(&self, _: &str) -> Option<&Value> {
        None
    }
    fn get_branch(&self, _: Option<&str>) -> Result<Vec<&Value>, serde_json::Error> {
        Ok(Vec::new())
    }
    fn get_header(&self) -> Option<&Value> {
        self.data.get("header").filter(|value| !value.is_null())
    }
    fn get_entries(&self) -> Vec<&Value> {
        self.data["entries"].as_array().unwrap().iter().collect()
    }
    fn get_tree(&self) -> Vec<SessionTreeNode<'_>> {
        Vec::new()
    }
    fn get_session_name(&self) -> io::Result<Option<&str>> {
        Ok(None)
    }
}
/// Controlled bundle and output with independently queued theme observations.
pub struct Operations {
    /// Whether input publication is reported.
    pub present: bool,
    /// Configured application name.
    pub app: String,
    /// Supplied asset text, in read order.
    pub assets: [String; 5],
    /// Closed synchronous effect transcript.
    pub events: RefCell<Vec<String>>,
    /// Theme callback transcript.
    pub themes: RefCell<Vec<Value>>,
    /// Captured output path/document.
    pub output: RefCell<Option<(String, String)>>,
    /// Queued colors.
    pub resolved: RefCell<VecDeque<Value>>,
    /// Queued explicit backgrounds.
    pub explicit: RefCell<VecDeque<Value>>,
    /// Failure at the output operation.
    pub write_failure: bool,
}
/// CSS slots used by the independent corpus.
pub const CSS: &str = ":root {\n      {{THEME_VARS}}\n      --body-bg: {{BODY_BG}};\n      --container-bg: {{CONTAINER_BG}};\n      --info-bg: {{INFO_BG}};\n    }";
impl Default for Operations {
    fn default() -> Self {
        Self {
            present: true,
            app: "maestro".into(),
            assets: [
                include_str!("../../assets/document/template.html").into(),
                CSS.into(),
                "/* application */".into(),
                "/* markdown */".into(),
                "/* highlighting */".into(),
            ],
            events: RefCell::default(),
            themes: RefCell::default(),
            output: RefCell::default(),
            resolved: RefCell::new(VecDeque::from([json!({"userMessageBg":"#343541"})])),
            explicit: RefCell::new(VecDeque::from([json!({})])),
            write_failure: false,
        }
    }
}
/// Pop an observation while retaining the last supplied value for repeated calls.
fn observe(queue: &RefCell<VecDeque<Value>>) -> Value {
    let mut queue = queue.borrow_mut();
    if queue.len() > 1 {
        queue.pop_front().unwrap()
    } else {
        queue.front().unwrap().clone()
    }
}
impl ExportOperations for Operations {
    fn app_name(&self) -> &str {
        &self.app
    }
    fn exists(&self, _: &str) -> bool {
        self.present
    }
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String> {
        self.events.borrow_mut().push(format!("{asset:?}"));
        Ok(self.assets[asset_index(asset)].clone())
    }
    fn get_resolved_theme_colors(
        &self,
        theme: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        self.themes.borrow_mut().push(json!(["resolved", theme]));
        let value = observe(&self.resolved);
        if let Some(error) = value.get("error") {
            return Err(theme_error(error.as_str().unwrap()));
        }
        Ok(value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap().into()))
            .collect())
    }
    fn get_theme_export_colors(&self, theme: Option<&str>) -> ThemeExportColors {
        self.themes.borrow_mut().push(json!(["export", theme]));
        explicit(&observe(&self.explicit))
    }
    fn write(&self, path: &str, document: &str) -> io::Result<()> {
        self.events.borrow_mut().push("write".into());
        if self.write_failure {
            return Err(io::Error::other("controlled output failure"));
        }
        *self.output.borrow_mut() = Some((path.into(), document.into()));
        Ok(())
    }
}
/// Convert an explicit-background input without truthiness fallback.
pub fn explicit(value: &Value) -> ThemeExportColors {
    let field = |key| {
        value
            .get(key)
            .map(|value| value.as_str().unwrap().to_owned())
    };
    ThemeExportColors {
        page_bg: field("pageBg"),
        card_bg: field("cardBg"),
        info_bg: field("infoBg"),
    }
}
/// Asset position in the source's closed read sequence.
pub fn asset_index(asset: ExportAsset) -> usize {
    match asset {
        ExportAsset::Template => 0,
        ExportAsset::Css => 1,
        ExportAsset::JavaScript => 2,
        ExportAsset::Markdown => 3,
        ExportAsset::Highlight => 4,
    }
}
/// Asset names supplied by the native bundle owner.
pub const ASSETS: [&str; 5] = [
    "template.html",
    "template.css",
    "template.js",
    "vendor/marked.min.js",
    "vendor/highlight.min.js",
];
/// Decode the document's actual carrier.
pub fn payload(document: &str) -> String {
    let start = document
        .split("<script id=\"session-data\" type=\"application/json\">")
        .nth(1)
        .unwrap();
    String::from_utf8(
        STANDARD
            .decode(start.split("</script>").next().unwrap())
            .unwrap(),
    )
    .unwrap()
}
/// Last written document, asserting that output was actually published.
pub fn html(operations: &Operations) -> String {
    operations.output.borrow().as_ref().unwrap().1.clone()
}
/// Callback recorder with independent source outputs.
pub struct Renderer {
    /// Ordered observed arguments.
    pub calls: RefCell<Vec<Value>>,
    /// Call response queue.
    pub call: RefCell<VecDeque<Value>>,
    /// Result response queue.
    pub result: RefCell<VecDeque<Value>>,
}
impl From<Returns> for Renderer {
    fn from(returns: Returns) -> Self {
        Self {
            calls: RefCell::default(),
            call: RefCell::new(returns.call.into()),
            result: RefCell::new(returns.result.into()),
        }
    }
}
/// Distinguish absent callback fields from explicit null.
fn argument(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or_else(|| json!({"absent":true}))
}
/// Read an optional response, preserving failure and explicit empty response.
fn response(queue: &RefCell<VecDeque<Value>>) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let value = queue
        .borrow_mut()
        .pop_front()
        .unwrap_or_else(|| json!({"absent":true}));
    if let Some(error) = value.get("throw") {
        return Err(io::Error::other(error.as_str().unwrap().to_owned()).into());
    }
    if value.get("absent") == Some(&json!(true)) {
        return Ok(None);
    }
    Ok(Some(value))
}
impl ToolHtmlRenderer for Renderer {
    fn render_call(
        &self,
        id: &str,
        name: &str,
        args: Option<&Value>,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        self.calls
            .borrow_mut()
            .push(json!({"kind":"call","id":id,"name":name,"args":argument(args)}));
        Ok(response(&self.call)?.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn render_result(
        &self,
        id: &str,
        name: &str,
        result: ToolHtmlResultInput<'_>,
    ) -> Result<Option<ToolHtmlResult>, Box<dyn std::error::Error>> {
        self.calls.borrow_mut().push(json!({"kind":"result","id":id,"name":name,"content":argument(result.content),"details":argument(result.details),"isError":result.is_error}));
        Ok(response(&self.result)?.map(|value| ToolHtmlResult {
            collapsed: value
                .get("collapsed")
                .map(|value| value.as_str().unwrap().into()),
            expanded: value
                .get("expanded")
                .map(|value| value.as_str().unwrap().into()),
        }))
    }
}
/// Run one record case through the export entry point, not internal helpers.
pub fn record(name: &str) {
    let row = corpus()
        .records
        .into_iter()
        .find(|row| row.name == name)
        .unwrap();
    let renderer = Renderer::from(row.returns);
    let operations = Operations::default();
    let view = View {
        data: json!({"header":null,"entries":row.entries,"leafId":null}),
        ..View::default()
    };
    let result = export_session_to_html(
        &view,
        None,
        Some(ExportOptions {
            tool_renderer: Some(&renderer),
            ..ExportOptions::default()
        }),
        &operations,
    );
    if let Some(expected) = row.expected.error {
        let error = result.unwrap_err();
        if expected == "invalid-record" {
            assert!(
                matches!(error, ExportError::Io(ref error) if error.kind() == io::ErrorKind::InvalidData)
            );
        } else {
            assert_eq!(error.to_string(), expected);
            assert!(std::error::Error::source(&error).is_some());
        }
        assert!(operations.events.borrow().is_empty());
    } else {
        result.unwrap();
        let payload = payload(&html(&operations));
        let data: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(data["entries"], view.data["entries"]);
        let records = data
            .get("renderedTools")
            .cloned()
            .unwrap_or_else(|| json!({}));
        assert_eq!(
            serde_json::to_string(&records).unwrap(),
            row.expected.json.unwrap()
        );
    }
    assert_eq!(*renderer.calls.borrow(), row.expected.calls);
}
/// Run a recorded document through the public exporter.
pub fn document(name: &str) -> String {
    let row = corpus()
        .documents
        .into_iter()
        .find(|row| row.name == name)
        .unwrap();
    let operations = Operations::default();
    let mut colors = row.colors;
    if name == "ordered" {
        let value = colors.as_object_mut().unwrap().shift_remove("2").unwrap();
        colors.as_object_mut().unwrap().insert("2".into(), value);
    }
    *operations.resolved.borrow_mut() = VecDeque::from([colors]);
    *operations.explicit.borrow_mut() = VecDeque::from([row.explicit]);
    let tools: Option<Vec<maestro_models::Tool>> = row
        .data
        .get("tools")
        .map(|value| serde_json::from_value(value.clone()).unwrap());
    let mut view = View {
        data: row.data,
        ..View::default()
    };
    if name == "unicode" {
        view.data["entries"][1]["numbers"][1] = json!(-0.0);
    }
    let state = ExportState {
        system_prompt: view.data.get("systemPrompt").and_then(Value::as_str),
        tools: tools.as_deref(),
    };
    export_session_to_html(&view, Some(state), None, &operations).unwrap();
    let html = html(&operations);
    assert_eq!(html, row.expected.html);
    assert_eq!(payload(&html), row.expected.payload);
    assert_eq!(*operations.themes.borrow(), row.expected.observations);
    html
}
/// Disposable native directory created exclusively.
pub struct Scratch(pub PathBuf);
impl Scratch {
    /// Allocate a process-local uniquely named directory without truncating files.
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    /// Join an authored filename for native I/O.
    pub fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().into()
    }
    /// Write a complete current session file.
    pub fn session(&self, name: &str, entries: &[Value]) -> String {
        let path = self.path(name);
        let header = json!({"type":"session","version":3,"id":"history","timestamp":"2026-01-01T00:00:00Z","cwd":self.0.to_str().unwrap()});
        let text = std::iter::once(&header)
            .chain(entries)
            .map(|entry| serde_json::to_string(entry).unwrap() + "\n")
            .collect::<String>();
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
/// A custom call whose fields distinguish rendering from omission.
pub fn call_view(id: &str) -> View {
    View {
        data: json!({"header":null,"entries":[{"type":"message","message":{"role":"assistant","content":[{"type":"toolCall","id":id,"name":"custom","arguments":{"selected":true}}]}}],"leafId":null}),
        ..View::default()
    }
}
/// Native theme state with controlled shipped documents.
pub fn native_theme(scratch: &Scratch) -> maestro_theme::ThemeState {
    let directory = scratch.path("themes");
    std::fs::create_dir(&directory).unwrap();
    for name in ["dark", "light"] {
        std::fs::write(
            maestro_path::join(&[&directory, &format!("{name}.json")]),
            r##"{"colors":{"userMessageBg":"#343541"},"vars":{}}"##,
        )
        .unwrap();
    }
    maestro_theme::ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: directory,
            custom_themes_dir: scratch.path("custom"),
        },
        std::rc::Rc::new(maestro_theme::NativeThemeOperations),
    )
}
/// Publish supplied assets to an actual native bundle.
pub fn bundle(scratch: &Scratch, assets: &[String; 5]) -> String {
    let directory = scratch.path("bundle");
    std::fs::create_dir(&directory).unwrap();
    std::fs::create_dir(maestro_path::join(&[&directory, "vendor"])).unwrap();
    for (name, text) in ASSETS.iter().zip(assets) {
        std::fs::write(maestro_path::join(&[&directory, name]), text).unwrap();
    }
    directory
}
/// Trace native calls without replacing native asset reads or theme policy.
pub struct NativeObserved<'a> {
    /// Real native adapter.
    pub native: NativeExportOperations<'a>,
    /// Read and downstream observations.
    pub events: RefCell<Vec<String>>,
}
impl ExportOperations for NativeObserved<'_> {
    fn app_name(&self) -> &str {
        self.native.app_name()
    }
    fn exists(&self, path: &str) -> bool {
        self.native.exists(path)
    }
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String> {
        self.events
            .borrow_mut()
            .push(ASSETS[asset_index(asset)].into());
        self.native.read_asset(asset)
    }
    fn get_resolved_theme_colors(
        &self,
        theme: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        self.events.borrow_mut().push("theme".into());
        self.native.get_resolved_theme_colors(theme)
    }
    fn get_theme_export_colors(&self, theme: Option<&str>) -> ThemeExportColors {
        self.events.borrow_mut().push("export".into());
        self.native.get_theme_export_colors(theme)
    }
    fn write(&self, path: &str, document: &str) -> io::Result<()> {
        self.events.borrow_mut().push("write".into());
        self.native.write(path, document)
    }
}
/// Two native same-format packs feed the unchanged exporter.
pub fn brand_swap() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/");
    let mut outputs = Vec::new();
    for name in ["brand.json", "packs/signal.json"] {
        let scratch = Scratch::new();
        let pack = maestro_theme::load_brand_pack(
            &maestro_path::join(&[root, name]),
            &maestro_theme::NativeThemeOperations,
        )
        .unwrap();
        let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
        let properties = presentation.css_properties();
        let css = properties
            .iter()
            .map(|(key, value)| format!("{key}: {value};"))
            .collect::<Vec<_>>()
            .join("\n");
        let mark = pack
            .mark_svg(
                maestro_theme::BrandMode::Dark,
                None,
                &maestro_theme::NativeThemeOperations,
            )
            .unwrap();
        let mut operations = Operations::default();
        operations.assets[1] = format!("{CSS}\n{css}");
        operations.assets[2] = format!("/* supplied mark */\n{mark}");
        let directory = bundle(&scratch, &operations.assets);
        let theme = pack_theme(&scratch, &pack);
        let native = NativeExportOperations {
            app_name: "maestro",
            template_dir: &directory,
            theme: &theme,
        };
        let input = scratch.session("tree.jsonl", &[]);
        let output = scratch.path("brand.html");
        export_from_file(&input, Some(output.as_str().into()), &native).unwrap();
        let document = std::fs::read_to_string(output).unwrap();
        assert!(document.contains(&css));
        assert!(document.contains(&mark));
        let colors = theme.get_theme_export_colors(None);
        assert!(document.contains(&format!("--body-bg: {};", colors.page_bg.unwrap())));
        outputs.push((css, mark));
    }
    assert_ne!(outputs[0], outputs[1]);
}
/// Swap native/memory/failing outputs and bundles through the same public operation.
pub fn adapter_swap() {
    let scratch = Scratch::new();
    let mut view = call_view("rendered");
    view.data["entries"][0]["id"] = json!("entry");
    view.data["entries"][0]["parentId"] = Value::Null;
    let input = scratch.session("tree.jsonl", view.data["entries"].as_array().unwrap());
    let session = maestro_session::SessionManager::open(&input, None, None).unwrap();
    let theme = native_theme(&scratch);
    let mut operations = Operations::default();
    operations.assets[2] = "first bundle".into();
    let directory = bundle(&scratch, &operations.assets);
    let native = NativeExportOperations {
        app_name: "maestro",
        template_dir: &directory,
        theme: &theme,
    };
    let output = scratch.path("out.html");
    let run = |provider: &dyn ExportOperations, html: &str| {
        let renderer = Renderer::from(Returns {
            call: vec![json!(html)],
            result: Vec::new(),
        });
        export_session_to_html(
            &session,
            None,
            Some(ExportOptions {
                output_path: Some(&output),
                tool_renderer: Some(&renderer),
                ..ExportOptions::default()
            }),
            provider,
        )
    };
    assert_eq!(run(&native, "first rendering").unwrap(), output);
    assert_eq!(run(&operations, "first rendering").unwrap(), output);
    assert_eq!(std::fs::read_to_string(&output).unwrap(), html(&operations));
    operations.assets[2] = "second bundle".into();
    run(&operations, "second rendering").unwrap();
    assert!(html(&operations).contains("second bundle"));
    assert!(!html(&operations).contains("first bundle"));
    let data: Value = serde_json::from_str(&payload(&html(&operations))).unwrap();
    assert_eq!(
        data["renderedTools"]["rendered"]["callHtml"],
        "second rendering"
    );
    let failing = Operations {
        write_failure: true,
        ..Operations::default()
    };
    assert!(matches!(
        run(&failing, "failed rendering"),
        Err(ExportError::Io(_))
    ));
}
/// Native theme read failure, generated through the owning public operation.
fn theme_error(message: &str) -> ThemeError {
    /// Supplied read failure below the theme owner's wrapper.
    struct Failure<'a>(&'a str);
    impl maestro_theme::ThemeOperations for Failure<'_> {
        fn read_to_string(&self, _: &str) -> io::Result<String> {
            Err(io::Error::other(self.0.to_owned()))
        }
        fn environment(&self, _: &str) -> Option<String> {
            None
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
        fn read_dir(&self, _: &str) -> io::Result<Vec<String>> {
            Err(io::ErrorKind::Unsupported.into())
        }
        fn sort_by_name(&self, _: &mut [maestro_theme::ThemeInfo]) -> io::Result<()> {
            Ok(())
        }
    }
    maestro_theme::load_theme_from_path(
        "missing",
        Some(maestro_theme::ColorMode::Truecolor),
        &Failure(message),
    )
    .err()
    .unwrap()
}

/// Native theme state projected from the selected supplied pack.
fn pack_theme(scratch: &Scratch, pack: &maestro_theme::BrandPack) -> maestro_theme::ThemeState {
    let theme_dir = scratch.path("themes");
    std::fs::create_dir(&theme_dir).unwrap();
    for mode in [
        maestro_theme::BrandMode::Dark,
        maestro_theme::BrandMode::Light,
    ] {
        std::fs::write(
            maestro_path::join(&[&theme_dir, &format!("{}.json", mode.name())]),
            pack.theme_json(mode).unwrap(),
        )
        .unwrap();
    }
    maestro_theme::ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: theme_dir,
            custom_themes_dir: scratch.path("custom"),
        },
        std::rc::Rc::new(maestro_theme::NativeThemeOperations),
    )
}
