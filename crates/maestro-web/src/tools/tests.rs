//! Shared card preparation observed through a recording representation.
use super::{
    extract_document::ExtractDocumentRenderer,
    javascript_repl::JavaScriptReplRenderer,
    renderer_registry::ToolHeaderState,
    renderers::{
        bash_renderer::BashRenderer,
        card_content::{Body, Caption, CardContent, CardOutput},
        collapsible_card::CollapsibleCard,
    },
    types::{ToolRenderer, ToolUiError},
};
use crate::components::console_block::ConsoleVariant;
use maestro_app::{
    JsonObject, TextContent, ToolResultMessage, UserBlock,
    browser::tools::{
        extract_document::{ExtractDocumentParams, ExtractDocumentResult},
        javascript_repl::{JavaScriptReplFile, JavaScriptReplParams, JavaScriptReplToolResult},
    },
};
use serde_json::{Value, json};
use std::{any::TypeId, cell::Cell, rc::Rc};

/// The selected recipe exactly as a representation receives it.
enum Recorded {
    /// Status card recipe.
    Card(CardContent),
    /// Disclosure recipe and the renderer that prepared it.
    Collapsible(Box<CollapsibleCard<Recorded>>, TypeId),
    /// Selected line difference operands.
    Diff(crate::components::diff::DiffProps),
}
impl CardOutput for Recorded {
    fn card(card: CardContent) -> Self {
        Self::Card(card)
    }
    fn collapsible<R: 'static>(card: CollapsibleCard<Self>) -> Self {
        Self::Collapsible(Box::new(card), TypeId::of::<R>())
    }
    fn diff(props: crate::components::diff::DiffProps) -> Self {
        Self::Diff(props)
    }
}
/// Prepare through the renderer's shared selection for the recording representation.
fn render<P, D, T: ToolRenderer<P, D, Recorded>>(
    renderer: &T,
    params: Option<&P>,
    result: Option<&ToolResultMessage<D>>,
) -> Result<Recorded, ToolUiError> {
    let shown = renderer.render(params, result.map(Into::into), false)?;
    assert!(!shown.is_custom);
    Ok(shown.content)
}
/// One argument object holding `key`, or none when `value` is absent.
fn object(key: &str, value: Option<Value>) -> JsonObject {
    value
        .map(|value| (key.to_owned(), value))
        .into_iter()
        .collect()
}
/// A result envelope with one text block.
fn result<D>(output: &str, is_error: bool, details: Option<D>) -> ToolResultMessage<D> {
    ToolResultMessage {
        tool_call_id: "call".into(),
        tool_name: "tool".into(),
        timestamp: 1.,
        content: vec![UserBlock::Text(TextContent::new(output.into()))],
        details,
        is_error,
        usage: None,
    }
}
/// The untranslated caption the selection chose.
fn caption(caption: &Caption) -> String {
    match caption {
        Caption::Literal(text) => text.clone(),
        Caption::Localized(key, suffix) => format!("{key}{suffix}"),
        Caption::Artifact(key, _) => (*key).to_owned(),
    }
}
/// The prepared disclosure recipe.
fn disclosure<R: 'static>(recorded: Recorded) -> CollapsibleCard<Recorded> {
    match recorded {
        Recorded::Collapsible(card, renderer) => {
            assert_eq!(renderer, TypeId::of::<R>());
            *card
        }
        Recorded::Card(_) | Recorded::Diff(_) => panic!("status card selected"),
    }
}
/// Selected values of every reached JSON class keep their fallback, literal or compact text.
const DOMAINS: [(Option<&str>, Option<&str>); 10] = [
    (None, None),
    (Some("null"), None),
    (Some(r#""""#), None),
    (Some("false"), None),
    (Some("0"), None),
    (Some(r#""  a <b> ""#), Some("  a <b> ")),
    (Some("1.5"), Some("1.5")),
    (Some("true"), Some("true")),
    (Some(r#"{"b":1,"a":[1]}"#), Some(r#"{"b":1,"a":[1]}"#)),
    (Some(r#"[1,"x"]"#), Some(r#"[1,"x"]"#)),
];

#[test]
fn native_card_preparation_retains_selected_field_domains() {
    bash_domains_keep_native_details();
    javascript_domains_and_files();
    extraction_domains_and_metadata();
    artifact_replacements_and_pill();
}

/// Command branches select the same text for every field class; native details stay native.
fn bash_domains_keep_native_details() {
    for (input, selected) in DOMAINS {
        let params = object(
            "command",
            input.map(|text| serde_json::from_str(text).unwrap()),
        );
        for is_error in [false, true] {
            let envelope = result("out", is_error, Some(Rc::new(Cell::new(1_u8))));
            let Recorded::Card(card) =
                render(&BashRenderer, Some(&params), Some(&envelope)).unwrap()
            else {
                panic!("disclosure selected for a command")
            };
            let state = if is_error {
                ToolHeaderState::Error
            } else {
                ToolHeaderState::Complete
            };
            assert_eq!(card.state, state);
            match (selected, &card.body) {
                (None, Body::Empty) => {
                    assert_eq!(caption(&card.caption), "Waiting for command...");
                }
                (Some(command), Body::Console(text, variant)) => {
                    assert_eq!(caption(&card.caption), "Running command...");
                    assert_eq!(text, &format!("> {command}\n\nout"));
                    assert_eq!(*variant == ConsoleVariant::Error, is_error);
                }
                _ => panic!("wrong command branch for {input:?}"),
            }
        }
    }
}

/// Code and title follow the field classes; returned files keep their records and decode text.
fn javascript_domains_and_files() {
    for (input, selected) in DOMAINS {
        let value = input.map(|text| serde_json::from_str::<Value>(text).unwrap());
        let params = object("code", value.clone());
        let pending = disclosure::<JavaScriptReplRenderer>(
            render::<_, Value, _>(&JavaScriptReplRenderer, Some(&params), None).unwrap(),
        );
        let body = pending.body.unwrap();
        assert_eq!(
            body.code.map(|(code, language)| {
                assert_eq!(language, "javascript");
                code
            }),
            selected.map(str::to_owned)
        );
        let envelope = result("", false, None::<Value>);
        let finished = disclosure::<JavaScriptReplRenderer>(
            render(&JavaScriptReplRenderer, Some(&params), Some(&envelope)).unwrap(),
        );
        assert_eq!(
            finished.body.unwrap().code.unwrap().0,
            selected.unwrap_or_default(),
            "a result always shows its listing"
        );
        let titled = disclosure::<JavaScriptReplRenderer>(
            render::<_, Value, _>(&JavaScriptReplRenderer, Some(&object("title", value)), None)
                .unwrap(),
        );
        assert_eq!(
            caption(&titled.caption),
            selected.unwrap_or("Executing JavaScript")
        );
    }
    returned_files();
    let malformed = result("", false, Some(json!({"files": "x"})));
    assert!(matches!(
        render(
            &JavaScriptReplRenderer,
            Some(&JsonObject::new()),
            Some(&malformed)
        ),
        Err(ToolUiError::Presentation(_))
    ));
    let unread = disclosure::<JavaScriptReplRenderer>(
        render(&JavaScriptReplRenderer, None, Some(&malformed)).unwrap(),
    );
    assert!(unread.body.is_none() && unread.attachments.is_empty());
    assert_eq!(caption(&unread.caption), "Preparing JavaScript...");
}

/// Returned files: name, MIME type, base64 content and byte length.
const FILES: [(&str, &str, &str, usize); 5] = [
    ("", "", "AAEC", 3),
    ("a.png", "image/png", "iVBO", 4),
    ("t.txt", "text/plain", "w6k=", 2),
    ("bad.json", "application/json", "%%%", 1),
    ("t.txt", "text/plain", "w6k=", 2),
];
/// Returned files keep order, duplicates, fallbacks, previews and UTF-8 text through both carriers.
fn returned_files() {
    let raw = result(
        "",
        false,
        Some(
            json!({"files": FILES.map(|(name, mime, content, size)| json!({
                "fileName": name, "mimeType": mime, "contentBase64": content, "size": size
            }))}),
        ),
    );
    let typed = result(
        "",
        false,
        Some(JavaScriptReplToolResult {
            files: Some(
                FILES
                    .map(|(name, mime, content, size)| JavaScriptReplFile {
                        file_name: name.into(),
                        mime_type: mime.into(),
                        size,
                        content_base64: content.into(),
                    })
                    .into(),
            ),
        }),
    );
    let typed_params = JavaScriptReplParams {
        title: String::new(),
        code: "c".into(),
    };
    recorded_files();
    for card in [
        render(
            &JavaScriptReplRenderer,
            Some(&JsonObject::new()),
            Some(&raw),
        )
        .unwrap(),
        render(&JavaScriptReplRenderer, Some(&typed_params), Some(&typed)).unwrap(),
    ] {
        assert_records(&disclosure::<JavaScriptReplRenderer>(card));
    }
}
/// The recorded execution files reach each original record field, without the generated identity.
fn recorded_files() {
    let cases = [
        include_str!("../../tests/fixtures/native_document_cards.json"),
        include_str!("../../tests/fixtures/document_tool_cards.json"),
    ]
    .map(|fixture| serde_json::from_str::<Vec<Value>>(fixture).unwrap());
    let mut seen = 0;
    for case in cases.iter().flatten().filter(|case| case["tool"] == "repl") {
        let expected = &case["expected"]["attachments"];
        if expected.as_array().is_none_or(Vec::is_empty) {
            continue;
        }
        let params: JsonObject = serde_json::from_value(case["input"]["params"].clone()).unwrap();
        let result: ToolResultMessage<Value> =
            serde_json::from_value(case["input"]["result"].clone()).unwrap();
        let card = disclosure::<JavaScriptReplRenderer>(
            render(&JavaScriptReplRenderer, Some(&params), Some(&result)).unwrap(),
        );
        assert_eq!(&Value::Array(records(&card)), expected);
        seen += 1;
    }
    assert_eq!(seen, 6);
}
/// Each prepared record with its generated identity removed.
fn records(card: &CollapsibleCard<Recorded>) -> Vec<Value> {
    card.attachments
        .iter()
        .map(|download| {
            let mut record = serde_json::to_value(download.record()).unwrap();
            let id = record.as_object_mut().unwrap().shift_remove("id").unwrap();
            assert!(id.as_str().unwrap().starts_with("repl-"));
            record
        })
        .collect()
}
/// Each record keeps its fallbacks, preview and decoded text under its own identity.
fn assert_records(card: &CollapsibleCard<Recorded>) {
    assert_eq!(
        Value::Array(records(card)),
        json!([
            {"type": "document", "fileName": "file-0", "mimeType": "application/octet-stream", "size": 3, "content": "AAEC"},
            {"type": "image", "fileName": "a.png", "mimeType": "image/png", "size": 4, "content": "iVBO", "preview": "iVBO"},
            {"type": "document", "fileName": "t.txt", "mimeType": "text/plain", "size": 2, "content": "w6k=", "extractedText": "é"},
            {"type": "document", "fileName": "bad.json", "mimeType": "application/json", "size": 1, "content": "%%%"},
            {"type": "document", "fileName": "t.txt", "mimeType": "text/plain", "size": 2, "content": "w6k=", "extractedText": "é"}
        ])
    );
    let ids: std::collections::HashSet<_> = card
        .attachments
        .iter()
        .map(|d| d.attachment.id.clone())
        .collect();
    assert_eq!(
        ids.len(),
        FILES.len(),
        "each emitted file has its own identity"
    );
}

/// Pending and completed URLs select independently; metadata reads only the reached fields.
fn extraction_domains_and_metadata() {
    for (input, selected) in DOMAINS {
        let params = object("url", input.map(|text| serde_json::from_str(text).unwrap()));
        let pending = disclosure::<ExtractDocumentRenderer>(
            render::<_, Value, _>(&ExtractDocumentRenderer, Some(&params), None).unwrap(),
        );
        let shown = match input {
            None | Some("null" | r#""""#) => Some(""),
            Some("false") => Some("false"),
            Some("0") => Some("0"),
            _ => selected,
        };
        assert_eq!(pending.body.unwrap().url.as_deref(), shown, "{input:?}");
        let envelope = result("", false, None::<Value>);
        let finished = disclosure::<ExtractDocumentRenderer>(
            render(&ExtractDocumentRenderer, Some(&params), Some(&envelope)).unwrap(),
        );
        assert_eq!(finished.body.unwrap().url.as_deref(), selected, "{input:?}");
    }
    extraction_metadata();
}
/// Captions read only the metadata their result branch reaches, through both carriers.
fn extraction_metadata() {
    let captions = [
        (json!(null), false, Ok("Extracted text from document")),
        (
            json!({"fileName": "", "format": 5}),
            true,
            Ok("Failed to extract document"),
        ),
        (
            json!({"fileName": "ü.pdf", "format": "straße", "size": 256}),
            false,
            Ok("Extracted text from ü.pdf (STRASSE, 0.3KB)"),
        ),
        (json!({}), false, Err(())),
    ];
    for (details, is_error, expected) in captions {
        let envelope = result("text", is_error, Some(details));
        let prepared = render(
            &ExtractDocumentRenderer,
            Some(&JsonObject::new()),
            Some(&envelope),
        )
        .map(|card| caption(&disclosure::<ExtractDocumentRenderer>(card).caption));
        assert_eq!(prepared.as_deref().map_err(|_| ()), expected);
    }
    let typed = result(
        "text",
        false,
        Some(ExtractDocumentResult {
            extracted_text: String::new(),
            format: "pdf".into(),
            file_name: "a.pdf".into(),
            size: 1536,
        }),
    );
    let typed_params = ExtractDocumentParams { url: String::new() };
    let card = disclosure::<ExtractDocumentRenderer>(
        render(&ExtractDocumentRenderer, Some(&typed_params), Some(&typed)).unwrap(),
    );
    assert_eq!(
        caption(&card.caption),
        "Extracted text from a.pdf (PDF, 1.5KB)"
    );
    let body = card.body.unwrap();
    assert!(body.url.is_none());
    assert_eq!(body.code, Some(("text".into(), "plaintext")));
    let unread = disclosure::<ExtractDocumentRenderer>(
        render(
            &ExtractDocumentRenderer,
            None,
            Some(&result("", false, Some(json!({})))),
        )
        .unwrap(),
    );
    assert!(unread.body.is_none());
    assert_eq!(caption(&unread.caption), "Preparing extraction...");
}

/// Update replacements are read only when reached, and the filename pill keeps its captured panel
/// and authored name, for raw and validated arguments alike.
fn artifact_replacements_and_pill() {
    use crate::tools::artifacts::artifacts_tool_renderer::ArtifactsToolRenderer;
    use maestro_app::browser::tools::artifacts::{ArtifactCommand, ArtifactsParams};
    let replacements = |recorded: Recorded| {
        let card = disclosure::<ArtifactsToolRenderer>(recorded);
        let Caption::Artifact(_, Some(pill)) = &card.caption else {
            panic!("missing filename pill")
        };
        assert_eq!(pill.filename.as_deref(), Some("a.rs"));
        assert!(pill.panel.is_none());
        card.body.and_then(|body| body.diff).map(|diff| match diff {
            Recorded::Diff(props) => (props.old_text, props.new_text, props.title),
            _ => panic!("difference expected"),
        })
    };
    let renderer = ArtifactsToolRenderer::new(None);
    let complete = result("ok", false, None::<Value>);
    for (old, new, phase, expected) in [
        (json!("o"), json!("n"), None, Some(("o", "n"))),
        (Value::Null, json!("n"), None, Some(("", "n"))),
        (json!(""), json!(0), Some(&complete), Some(("", ""))),
        (Value::Null, Value::Null, Some(&complete), Some(("", ""))),
    ] {
        let params =
            json!({"command": "update", "filename": "a.rs", "old_str": old, "new_str": new});
        let shown = render(&renderer, params.as_object(), phase).unwrap();
        let expected = expected.map(|(old, new)| (old.to_owned(), new.to_owned(), None));
        assert_eq!(replacements(shown), expected);
    }
    let params = json!({"command": "update", "filename": "a.rs", "old_str": "o"});
    let pending = render(&renderer, params.as_object(), None::<&ToolResultMessage>).unwrap();
    assert_eq!(
        replacements(pending),
        None,
        "an absent replacement selects no difference"
    );
    let typed = ArtifactsParams {
        command: ArtifactCommand::Update,
        filename: "a.rs".into(),
        content: None,
        old_str: Some("o".into()),
        new_str: Some("n".into()),
    };
    let shown = render(&renderer, Some(&typed), None::<&ToolResultMessage>).unwrap();
    assert_eq!(replacements(shown), Some(("o".into(), "n".into(), None)));
}

/// The real initializer's built-ins prepare their owned records through the shared dispatcher on
/// native targets, for map, JSON and typed operands and for details no transport trait describes.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn builtin_initializer_prepares_native_records() {
    use super::{
        render_tool,
        renderer_registry::{TOOL_RENDERERS, get_tool_renderer},
        renderers::{collapsible_card::Content, default_renderer::DefaultContent},
        types::{ToolParams, ToolRenderResult},
    };
    use std::any::Any;
    /// Details with no transport, cloning or default trait.
    struct Opaque;
    /// The record each built-in prepares, independent of the operand route.
    fn record(name: &str, shown: &ToolRenderResult<Box<dyn Any>>) {
        assert!(!shown.is_custom, "{name}");
        let content = shown.content.as_ref();
        let prepared = match name {
            "bash" => content.is::<CardContent>(),
            "javascript_repl" => content.is::<Content<JavaScriptReplRenderer, Box<dyn Any>>>(),
            "extract_document" => content.is::<Content<ExtractDocumentRenderer, Box<dyn Any>>>(),
            _ => content.is::<DefaultContent>(),
        };
        assert!(prepared, "{name}");
    }
    TOOL_RENDERERS.with(|all| {
        let names: Vec<_> = all.borrow().keys().cloned().collect();
        assert_eq!(names, ["javascript_repl", "extract_document", "bash"]);
    });
    let params = json!({"command": "ls", "code": "1 + 1", "url": "https://a.test/d"});
    let members = params.as_object().unwrap();
    for name in ["bash", "javascript_repl", "extract_document", "unknown"] {
        if let Some(renderer) = get_tool_renderer::<JsonObject, Value>(name) {
            record(name, &renderer.render(Some(members), None, false).unwrap());
        }
        let object = ToolParams::object(members);
        record(
            name,
            &render_tool::<JsonObject, Value>(name, Some(object), None, false).unwrap(),
        );
        let json = Some(ToolParams::json(&params));
        record(
            name,
            &render_tool::<Value, Value>(name, json, None, false).unwrap(),
        );
    }
    let repl: JavaScriptReplParams =
        serde_json::from_value(json!({"title": "t", "code": "1"})).unwrap();
    let typed = Some(ToolParams::typed(&repl, |_| Ok(None)));
    let shown = render_tool::<_, JavaScriptReplToolResult>("javascript_repl", typed, None, false);
    record("javascript_repl", &shown.unwrap());
    let extract: ExtractDocumentParams = serde_json::from_value(json!({"url": "u"})).unwrap();
    let typed = Some(ToolParams::typed(&extract, |_| Ok(None)));
    let shown = render_tool::<_, ExtractDocumentResult>("extract_document", typed, None, false);
    record("extract_document", &shown.unwrap());
    let opaque = result("out", false, Some(Opaque));
    let object = Some(ToolParams::object(members));
    record(
        "bash",
        &render_tool("bash", object, Some(&opaque), false).unwrap(),
    );
}
