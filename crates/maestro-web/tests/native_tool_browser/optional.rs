//! Calculation, current-time and artifact cards and direct differences through page roots.
use crate::{
    artifact_card_support::{self as cards, browser_records as records},
    contributions_support::{body, localized_view, select, text},
};
use maestro_app::ToolResultMessage;
use maestro_web::{
    ArtifactsToolRenderer, BashRenderer, CalculateRenderer, GetCurrentTimeRenderer,
    dom::{DomContent, DomView, MountedContent},
    tools::types::{ToolRenderResult, ToolRenderer, ToolUiError},
};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, HtmlElement};

/// Page-content preparation.
type Dom = ToolRenderResult<Box<dyn DomContent>>;

/// A mounted page root, removed and released on drop.
struct Shown(MountedContent);
impl Shown {
    fn new(content: &dyn DomContent, view: &DomView) -> Self {
        let mut slot = MountedContent::default();
        slot.show(content, &body(), view).unwrap();
        Self(slot)
    }
    fn root(&self) -> Element {
        self.0.root().unwrap().clone().unchecked_into()
    }
}
impl Drop for Shown {
    fn drop(&mut self) {
        self.0.clear();
    }
}

/// Details without serialization, cloning or equality.
struct Opaque;
/// The same result envelope carrying opaque native details.
fn opaque(result: &ToolResultMessage) -> ToolResultMessage<Rc<Opaque>> {
    ToolResultMessage {
        tool_call_id: result.tool_call_id.clone(),
        tool_name: result.tool_name.clone(),
        content: result.content.clone(),
        details: Some(Rc::new(Opaque)),
        is_error: result.is_error,
        timestamp: result.timestamp,
    }
}
/// Prepare a recorded command, calculation or current-time card for page content.
fn optional<D: 'static>(
    tool: &str,
    input: &crate::support::tool_input::Input,
    result: Option<&ToolResultMessage<D>>,
) -> Dom {
    let (params, streaming) = (input.params.as_ref(), input.is_streaming);
    let result = result.map(Into::into);
    match tool {
        "bash" => BashRenderer.render(params, result, streaming),
        "calculate" => CalculateRenderer.render(params, result, streaming),
        "time" => GetCurrentTimeRenderer.render(params, result, streaming),
        other => panic!("unknown optional tool {other}"),
    }
    .unwrap()
}
/// Every recorded command, calculation and current-time branch, field domain and locale, with
/// the same outcome for opaque native details.
pub(super) fn optional_tools(view: &Rc<DomView>) {
    let fixture = crate::tool_cards_support::fixture();
    let rows = (fixture.rows.into_iter()).map(|row| (row.tool, row.input, row.expected));
    let domains = (fixture.domains.into_iter()).map(|row| (row.tool, row.input, row.expected));
    let localized = (fixture.localized.into_iter()).map(|row| (row.tool, row.input, row.expected));
    let cases: Vec<_> = rows.chain(domains).chain(localized).collect();
    assert_eq!(cases.len(), 210);
    for (index, (tool, input, expected)) in cases.iter().enumerate() {
        let view = input.language.as_deref().map_or_else(
            || Rc::clone(view),
            |language| localized_view(view, language),
        );
        let shown = optional(tool, input, input.result.as_ref());
        assert_eq!(shown.is_custom, expected.is_custom, "{index}");
        let card = Shown::new(shown.content.as_ref(), &view);
        super::observe_card(&card.root(), expected, index);
        if let Some(result) = &input.result {
            let detail = optional(tool, input, Some(&opaque(result)));
            let other = Shown::new(detail.content.as_ref(), &view);
            assert_eq!(
                other.root().outer_html(),
                card.root().outer_html(),
                "{index}"
            );
        }
    }
}

/// Compare a mounted artifact card with its recorded visible fields.
fn observe_artifact(card: &Element, expected: &records::Expected, index: &str) {
    use crate::contributions_support::select;
    let header = card.first_element_child().unwrap();
    assert_eq!(
        crate::contributions_support::disclosure(&header).is_some(),
        expected.collapsible,
        "{index}"
    );
    let count = |selector: &str| card.query_selector_all(selector).unwrap().length() as usize;
    assert_eq!(
        count(".maestro-tool-spinner"),
        usize::from(expected.status == "inprogress"),
        "{index}"
    );
    for status in ["complete", "error"] {
        assert_eq!(
            count(&format!(".maestro-tool-icon.maestro-tool-{status}")),
            usize::from(expected.status == status),
            "{index}"
        );
    }
    let caption = select(&header, ".maestro-tool-caption");
    let pill = caption.query_selector(".maestro-artifact-pill").unwrap();
    assert_eq!(
        pill.as_ref().map(|pill| text(pill)),
        expected.pill,
        "{index}"
    );
    let label = match &pill {
        Some(_) => text(
            &caption
                .first_element_child()
                .unwrap()
                .first_child()
                .unwrap(),
        ),
        None => text(&caption),
    };
    assert_eq!(label, expected.caption, "{index}");
    let body = card.query_selector(".maestro-tool-body").unwrap();
    assert_eq!(body.is_some(), expected.collapsible, "{index}");
    assert_eq!(
        body.map(|body| body.class_list().contains("max-h-0")),
        expected.initially_collapsed,
        "{index}"
    );
    assert_eq!(count("script"), 0, "{index}");
    observe_artifact_children(card, expected, index);
}
/// Listings, consoles, destructive text and difference rows, each in order.
fn observe_artifact_children(card: &Element, expected: &records::Expected, index: &str) {
    let listings: Vec<_> = super::all(card, ".maestro-code")
        .iter()
        .map(|shown| {
            let language = shown.get_attribute("language").unwrap();
            (listing_text(shown), language)
        })
        .collect();
    let blocks: Vec<_> = (expected.code.iter())
        .map(|block| {
            assert_eq!(block.decoded, block.code, "{index}");
            (block.code.clone(), block.language.clone())
        })
        .collect();
    assert_eq!(listings, blocks, "{index}");
    let consoles: Vec<_> = super::all(card, ".maestro-console")
        .iter()
        .map(|shown| {
            let variant = if shown
                .query_selector(".maestro-console-error")
                .unwrap()
                .is_some()
            {
                "error"
            } else {
                "default"
            };
            (text(&select(shown, "pre")), variant.to_owned())
        })
        .collect();
    let outputs: Vec<_> = (expected.console.iter())
        .map(|console| (console.content.clone(), console.variant.clone()))
        .collect();
    assert_eq!(consoles, outputs, "{index}");
    let errors: Vec<_> = super::all(card, "div.maestro-tool-error")
        .iter()
        .map(|error| text(error))
        .collect();
    assert_eq!(errors, expected.errors, "{index}");
    assert_eq!(diff_rows(card), rows(&expected.diff), "{index}");
}
/// Observe the literal operand separately from HTML parsing's newline conversion.
fn listing_text(block: &Element) -> String {
    let code = select(block, "copy-button").get_attribute("text").unwrap();
    assert_eq!(
        text(&select(block, "pre code")),
        maestro_app::normalize_to_lf(&code)
    );
    code
}
/// Ordered difference rows as text and change kind.
fn diff_rows(root: &Element) -> Vec<(String, String)> {
    super::all(root, ".maestro-diff-scroll pre")
        .iter()
        .map(|row| {
            let classes = row.class_list();
            let kind = if classes.contains("maestro-diff-added") {
                "added"
            } else if classes.contains("maestro-diff-removed") {
                "removed"
            } else {
                "unchanged"
            };
            (text(row), kind.to_owned())
        })
        .collect()
}
/// Recorded difference rows in the observed shape.
fn rows(expected: &[records::DiffRow]) -> Vec<(String, String)> {
    (expected.iter())
        .map(|row| (row.text.clone(), row.kind.clone()))
        .collect()
}
/// Prepare a recorded artifact card through the object adapter.
fn artifact(input: &crate::support::tool_input::Input) -> Result<Dom, ToolUiError> {
    ArtifactsToolRenderer::new(None).render(
        input.params.as_ref(),
        input.result.as_ref().map(Into::into),
        input.is_streaming,
    )
}
/// Prepare an artifact card through the whole-value adapter at a recorded phase.
fn value(params: Option<&Value>, phase: &str) -> Result<Dom, ToolUiError> {
    let result = cards::phase(phase);
    ArtifactsToolRenderer::new(None).render(params, result.as_ref().map(Into::into), false)
}
/// Observe one prepared artifact card.
fn observe(shown: &Dom, view: &DomView, expected: &records::Expected, index: &str) {
    assert_eq!(shown.is_custom, expected.is_custom, "{index}");
    let card = Shown::new(shown.content.as_ref(), view);
    observe_artifact(&card.root(), expected, index);
}

/// The four command groups, localized captions, value routes and discarded replacements.
pub(super) fn artifacts(view: &Rc<DomView>) {
    let groups: std::collections::BTreeMap<String, Vec<records::Row>> =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards_browser.json")).unwrap();
    let counts: Vec<_> = groups
        .iter()
        .map(|(name, rows)| (name.as_str(), rows.len()))
        .collect();
    assert_eq!(
        counts,
        [
            (
                "artifact_cards_cover_command_status_and_parameter_branches",
                54
            ),
            (
                "artifact_cards_keep_literal_output_and_html_console_rules",
                288
            ),
            (
                "artifact_cards_place_pills_only_on_the_selected_header_branches",
                21
            ),
            (
                "artifact_update_distinguishes_absent_and_empty_replacements",
                27
            ),
        ]
    );
    for (name, rows) in &groups {
        for (index, row) in rows.iter().enumerate() {
            let shown = artifact(&row.input).unwrap();
            observe(&shown, view, &row.expected, &format!("{name} {index}"));
        }
    }
    let localized: Vec<records::Row> =
        serde_json::from_str(include_str!("../fixtures/artifact_localized.json")).unwrap();
    assert_eq!(localized.len(), 21);
    for (index, row) in localized.iter().enumerate() {
        let view = localized_view(view, row.input.language.as_deref().unwrap());
        let shown = artifact(&row.input).unwrap();
        observe(&shown, &view, &row.expected, &format!("localized {index}"));
    }
    let routes: Vec<records::ValueRoute> = serde_json::from_str(include_str!(
        "../fixtures/artifact_value_routes_browser.json"
    ))
    .unwrap();
    assert_eq!(routes.len(), 39);
    for (index, row) in routes.iter().enumerate() {
        let shown = value(row.input.params.as_ref(), &row.input.phase).unwrap();
        observe(&shown, view, &row.expected, &format!("route {index}"));
    }
    unused_replacements(view);
    fields(view);
    typed(view);
}
/// Discarded replacements preserve their recorded error boundary.
fn unused_replacements(view: &Rc<DomView>) {
    let unused: Vec<records::UnusedDiff> = serde_json::from_str(include_str!(
        "../fixtures/artifact_unused_diff_browser.json"
    ))
    .unwrap();
    assert_eq!(unused.len(), 4);
    for (index, row) in unused.iter().enumerate() {
        let shown = artifact(&row.input);
        match &row.expected {
            records::Branch::Rendered { card } => {
                observe(&shown.unwrap(), view, card, &format!("unused {index}"));
            }
            records::Branch::Error { kind } => {
                assert_eq!(kind, "TypeError");
                assert!(
                    matches!(shown, Err(ToolUiError::Presentation(_))),
                    "unused {index}"
                );
            }
        }
    }
}
/// Reached-field admission, selected code text and literal suffix languages.
fn fields(view: &Rc<DomView>) {
    let admission = cards::admission();
    assert_eq!(admission.len(), 390);
    for row in &admission {
        let context = format!("{} {} {} {}", row.context, row.key, row.kind, row.phase);
        let shown = value(Some(&row.input), &row.phase);
        match &row.expected {
            cards::Branch::Prepared { is_custom } => {
                let shown = shown.unwrap();
                assert_eq!(shown.is_custom, *is_custom, "{context}");
                drop(Shown::new(shown.content.as_ref(), view));
            }
            cards::Branch::Error { kind } => {
                assert_eq!(kind, "TypeError");
                assert!(shown.is_err(), "{context}");
            }
            cards::Branch::Rendered { .. } => panic!("unexpected admission branch"),
        }
    }
    let domains: Vec<records::CodeDomain> = serde_json::from_str(include_str!(
        "../fixtures/artifact_code_domains_browser.json"
    ))
    .unwrap();
    assert_eq!(domains.len(), 42);
    for (index, row) in domains.iter().enumerate() {
        let shown = value(Some(&row.input.params), &row.input.phase).unwrap();
        let card = Shown::new(shown.content.as_ref(), view);
        let code = card.root().query_selector(".maestro-code").unwrap();
        let expected = match &row.expected {
            records::CodeSelection::Omitted {} => None,
            records::CodeSelection::Text { text } => Some(text.clone()),
        };
        assert_eq!(
            code.map(|code| listing_text(&code)),
            expected,
            "domain {index}"
        );
    }
    languages(view);
}
/// Filename suffixes select literal code languages.
fn languages(view: &Rc<DomView>) {
    let languages: Vec<records::Language> =
        serde_json::from_str(include_str!("../fixtures/artifact_languages_browser.json")).unwrap();
    assert_eq!(languages.len(), 77);
    for row in languages {
        let mut params = json!({"command": "create", "content": "literal"});
        if let Some(filename) = &row.input.filename {
            params["filename"] = filename.as_str().into();
        }
        let shown = value(Some(&params), "pending").unwrap();
        let card = Shown::new(shown.content.as_ref(), view);
        let block = select(&card.root(), ".maestro-code");
        assert_eq!(block.get_attribute("language").unwrap(), row.expected);
        assert_eq!(text(&select(&block, "pre code")), "literal");
    }
}
/// Validated native arguments mount the same card as their raw object.
fn typed(view: &Rc<DomView>) {
    use maestro_app::browser::tools::artifacts::{ArtifactCommand, ArtifactsParams};
    let params = ArtifactsParams {
        command: ArtifactCommand::Update,
        filename: "page.html".into(),
        content: None,
        old_str: Some("a\nb\n".into()),
        new_str: Some("a\nc\n".into()),
    };
    let raw = json!({"command": "update", "filename": "page.html", "old_str": "a\nb\n", "new_str": "a\nc\n"});
    let result = crate::contributions_support::result("log", false);
    let native: Dom = <ArtifactsToolRenderer as ToolRenderer<ArtifactsParams, Value, _>>::render(
        &ArtifactsToolRenderer::new(None),
        Some(&params),
        Some((&result).into()),
        false,
    )
    .unwrap();
    let object: Dom = ArtifactsToolRenderer::new(None)
        .render(raw.as_object(), Some((&result).into()), false)
        .unwrap();
    let (native, object) = (
        Shown::new(native.content.as_ref(), view),
        Shown::new(object.content.as_ref(), view),
    );
    let native_id = select(&native.root(), ".maestro-tool-caption").id();
    let object_id = select(&object.root(), ".maestro-tool-caption").id();
    assert!(!native_id.is_empty());
    assert_ne!(
        native_id, object_id,
        "independent headers have distinct labels"
    );
    assert_eq!(
        native.root().outer_html().replace(&native_id, "caption"),
        object.root().outer_html().replace(&object_id, "caption")
    );
    assert_eq!(
        diff_rows(&native.root()),
        [("  a", "unchanged"), ("- b", "removed"), ("+ c", "added")]
            .map(|(text, kind)| (text.to_owned(), kind.to_owned()))
    );
    assert_eq!(text(&select(&native.root(), ".maestro-console pre")), "log");
}

/// One recorded template change and the children it keeps.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    input: Pair,
    expected: Kept,
}
/// The preceding and following template.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    from: Template,
    to: Template,
}
/// Operands selecting one artifact template.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    command: Option<String>,
    phase: String,
    present: Option<bool>,
    streaming: Option<bool>,
    content: Option<String>,
}
/// Which children survive the change.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Kept {
    header_retained: bool,
    body_retained: Option<bool>,
    collapsed: Option<bool>,
    code_retained: Option<bool>,
}
/// Prepare one template's page content.
fn template(template: &Template) -> Dom {
    let params = template.command.as_ref().map_or_else(
        || template.present.unwrap_or(false).then(|| json!({})),
        |command| Some(json!({"command": command, "filename": "a.rs", "content": template.content.as_deref().unwrap_or("literal"), "old_str": "old", "new_str": "new"})),
    );
    let mut result = cards::phase(&template.phase);
    if let Some(result) = &mut result {
        result
            .content
            .push(maestro_app::UserBlock::Text(maestro_app::TextContent::new(
                "R".into(),
            )));
    }
    <ArtifactsToolRenderer as ToolRenderer<Value, Value, _>>::render(
        &ArtifactsToolRenderer::new(None),
        params.as_ref(),
        result.as_ref().map(Into::into),
        template.streaming.unwrap_or(false),
    )
    .unwrap()
}
/// Every recorded template change keeps exactly the children the reference keeps, with an
/// expanded body staying open while its template stays selected.
pub(super) fn transitions(view: &Rc<DomView>) {
    let rows: Vec<Transition> =
        serde_json::from_str(include_str!("../fixtures/artifact_transitions.json")).unwrap();
    assert_eq!(rows.len(), 26);
    for (index, row) in rows.iter().enumerate() {
        let before = template(&row.input.from);
        let mut card = Shown::new(before.content.as_ref(), view);
        let root = card.root();
        let find = |selector: &str| root.query_selector(selector).unwrap();
        let (caption, body, code) = (
            find(".maestro-tool-caption").unwrap(),
            find(".maestro-tool-body"),
            find(".maestro-code"),
        );
        if body.is_some() {
            super::header_of(&root).click();
        }
        let after = template(&row.input.to);
        card.0
            .show(
                after.content.as_ref(),
                &crate::contributions_support::body(),
                view,
            )
            .unwrap();
        let kept = |old: &Element, selector: &str| {
            find(selector).is_some_and(|new| new.is_same_node(Some(old)))
        };
        assert_eq!(
            kept(&caption, ".maestro-tool-caption"),
            row.expected.header_retained,
            "{index}"
        );
        if let Some(expected) = row.expected.code_retained {
            assert_eq!(
                code.as_ref()
                    .is_some_and(|code| kept(code, ".maestro-code")),
                expected,
                "{index}"
            );
        }
        if let Some(expected) = row.expected.body_retained {
            assert_eq!(
                body.as_ref()
                    .is_some_and(|body| kept(body, ".maestro-tool-body")),
                expected,
                "{index}"
            );
        }
        assert_eq!(
            find(".maestro-tool-body").map(|body| body.class_list().contains("max-h-0")),
            row.expected.collapsed,
            "{index}"
        );
    }
}

/// Direct differences keep their title, counts, class and ordered rows when mounted and when
/// one retained root is updated through every case.
pub(super) fn diffs(view: &Rc<DomView>) {
    use maestro_web::components::diff::{DiffProps, diff};
    crate::diff_factory::diff_factory_identity_uses_normalized_semantics(view);
    let cases = cards::diff_cases();
    assert_eq!(cases.len(), 48);
    let mut retained = Shown(MountedContent::default());
    for (index, row) in cases.into_iter().enumerate() {
        let props = || DiffProps {
            old_text: row.input.old_text.clone(),
            new_text: row.input.new_text.clone(),
            title: row.input.title.clone(),
            class_name: row.input.class_name.clone(),
        };
        let fresh = Shown::new(diff(props()).as_ref(), view);
        retained
            .0
            .show(diff(props()).as_ref(), &body(), view)
            .unwrap();
        for root in [fresh.root(), retained.root()] {
            assert_eq!(diff_rows(&root), rows(&row.expected.rows), "{index}");
            let title = root.query_selector(".maestro-diff-header > span").unwrap();
            assert_eq!(
                title.map(|title| text(&title)),
                row.expected.title,
                "{index}"
            );
            let counts = root.query_selector(".maestro-diff-counts").unwrap();
            assert_eq!(
                counts.map(|counts| text(&counts)),
                row.expected.counts,
                "{index}"
            );
            assert_eq!(
                root.class_list().contains("custom-token"),
                row.expected.class_present,
                "{index}"
            );
        }
    }
}

/// A real panel, with its presentation counting open notifications.
pub(super) struct Panel {
    /// Application panel the pills capture.
    pub(super) panel: Rc<maestro_app::browser::tools::artifacts::LocalArtifactsPanel>,
    /// Test presentation behind the panel.
    pub(super) presentation: Rc<crate::presentation::Presentation>,
}
impl Panel {
    /// A panel holding one stored artifact, with a different one shown.
    pub(super) fn new() -> Self {
        let host = Rc::new(crate::attachment_support::Host::default());
        let presentation = Rc::new(crate::presentation::Presentation::new(host));
        let panel = Rc::new(
            maestro_app::browser::tools::artifacts::LocalArtifactsPanel::new_local(
                presentation.clone(),
            ),
        );
        panel.show_artifact("sentinel");
        presentation.opened.set(0);
        Self {
            panel,
            presentation,
        }
    }
    /// Create `filename` through the panel's own tool.
    pub(super) fn create(&self, filename: &str) -> ToolResultMessage {
        let result = crate::presentation::ready((self.panel.tool().execute)(
            "create".into(),
            json!({"command": "create", "filename": filename, "content": "stored"}),
            None,
            None,
        ))
        .unwrap();
        self.presentation.opened.set(0);
        ToolResultMessage {
            tool_call_id: "create".into(),
            tool_name: "artifacts".into(),
            content: result.content,
            details: None,
            is_error: false,
            timestamp: 0.,
        }
    }
}
/// A pending update card whose filename pill captures `panel`.
fn pill_card(filename: &Value, panel: Option<&Panel>) -> Dom {
    let renderer = ArtifactsToolRenderer::new(panel.map(|panel| Rc::clone(&panel.panel)));
    let params =
        json!({"command": "update", "filename": filename, "old_str": "old", "new_str": "new"});
    <ArtifactsToolRenderer as ToolRenderer<Value, Value, _>>::render(
        &renderer,
        Some(&params),
        None,
        false,
    )
    .unwrap()
}
/// Observe the pill's handling before its containing header, then the complete dispatch.
fn click_pill(card: &Element) -> (u32, bool, bool) {
    let pill = select(card, ".maestro-artifact-pill");
    let bubbled = Rc::new(Cell::new(0));
    let prevented = Rc::new(Cell::new(None));
    let (count, before_header) = (Rc::clone(&bubbled), Rc::clone(&prevented));
    let listener = Closure::<dyn Fn(web_sys::Event)>::new(move |event: web_sys::Event| {
        count.set(count.get() + 1);
        before_header.set(Some(event.default_prevented()));
    });
    let parent = pill.parent_element().unwrap();
    parent
        .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    let init = web_sys::CustomEventInit::new();
    init.set_bubbles(true);
    init.set_cancelable(true);
    let event = web_sys::CustomEvent::new_with_event_init_dict("click", &init).unwrap();
    pill.dispatch_event(&event).unwrap();
    parent
        .remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    let final_prevented = event.default_prevented();
    (
        bubbled.get(),
        prevented.get().unwrap_or(final_prevented),
        final_prevented,
    )
}
/// Both recorded pill cases, literal filenames and a display-only filename value.
pub(super) fn pills(view: &Rc<DomView>) {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../fixtures/artifact_pills.json")).unwrap();
    assert_eq!(cases.len(), 2);
    for case in &cases {
        let expected = &case["expected"];
        let filename = expected["filename"].as_str().unwrap();
        let panel = Panel::new();
        crate::presentation::store(&panel.panel, filename);
        let supplied = case["supplied"].as_bool().unwrap();
        let shown = pill_card(&filename.into(), supplied.then_some(&panel));
        let card = Shown::new(shown.content.as_ref(), view);
        let root = card.root();
        let pill = select(&root, ".maestro-artifact-pill");
        assert_eq!(text(&pill), expected["filename"].as_str().unwrap());
        assert!(pill.query_selector("svg").unwrap().is_some());
        assert_eq!(
            pill.class_list().contains("maestro-text-action"),
            expected["pointer"]
        );
        let body = select(&root, ".maestro-tool-body");
        let nested = &expected["nested"];
        assert_eq!(
            body.class_list().contains("max-h-0"),
            nested["initiallyCollapsed"]
        );
        let (bubbled, prevented, after_header) = click_pill(&root);
        assert_eq!(Value::from(bubbled), expected["bubbled"]);
        assert_eq!(Value::from(prevented), expected["prevented"]);
        assert!(after_header);
        assert_eq!(
            body.class_list().contains("max-h-0"),
            nested["collapsedAfterPill"]
        );
        let calls = expected["calls"].as_array().unwrap();
        assert_eq!(panel.presentation.opened.get(), calls.len());
        for call in calls {
            assert_eq!(call.as_str(), panel.panel.active_filename().as_deref());
        }
    }
    literal_pills(view);
}
/// Literal filename text stays inert, and display-only values never become identities.
fn literal_pills(view: &Rc<DomView>) {
    let panel = Panel::new();
    let literal = "<script>no()</script>  雪\t<&>.rs";
    let shown = pill_card(&literal.into(), Some(&panel));
    let card = Shown::new(shown.content.as_ref(), view);
    assert_eq!(
        text(&select(&card.root(), ".maestro-artifact-pill")),
        literal
    );
    assert!(card.root().query_selector("script").unwrap().is_none());
    let shown = pill_card(&json!({"name": "display only"}), Some(&panel));
    let card = Shown::new(shown.content.as_ref(), view);
    let body = select(&card.root(), ".maestro-tool-body");
    select(&card.root(), ".maestro-artifact-pill")
        .unchecked_into::<HtmlElement>()
        .click();
    assert!(
        body.class_list().contains("max-h-0"),
        "a panel pill never toggles"
    );
    assert_eq!(panel.panel.active_filename().as_deref(), Some("sentinel"));
    assert_eq!(panel.presentation.opened.get(), 0);
}

/// A really created artifact's card keeps the panel it captured: its pill, or any descendant of
/// it, opens that file once without toggling the body, while the header still toggles it.
pub(super) fn created_artifact(view: &Rc<DomView>) {
    let panel = Panel::new();
    let weak = Rc::downgrade(&panel.panel);
    let result = panel.create("made.rs");
    let mut renderer = ArtifactsToolRenderer::new(Some(Rc::clone(&panel.panel)));
    let params = json!({"command": "create", "filename": "made.rs", "content": "stored"});
    let shown: Dom = renderer
        .render(params.as_object(), Some((&result).into()), false)
        .unwrap();
    renderer.artifacts_panel = None;
    drop(renderer);
    panel.panel.show_artifact("sentinel");
    panel.presentation.opened.set(0);
    let card = Shown::new(shown.content.as_ref(), view);
    let root = card.root();
    let body = select(&root, ".maestro-tool-body");
    let pill: HtmlElement = select(&root, ".maestro-artifact-pill").unchecked_into();
    pill.click();
    assert_eq!(panel.panel.active_filename().as_deref(), Some("made.rs"));
    assert_eq!(panel.presentation.opened.get(), 1);
    assert!(
        body.class_list().contains("max-h-0"),
        "the pill does not toggle"
    );
    super::header_of(&root).click();
    assert!(
        !body.class_list().contains("max-h-0"),
        "the header still toggles"
    );
    panel.panel.show_artifact("sentinel");
    select(&pill, "span")
        .unchecked_into::<HtmlElement>()
        .click();
    assert_eq!(panel.panel.active_filename().as_deref(), Some("made.rs"));
    assert_eq!(panel.presentation.opened.get(), 2);
    assert!(!body.class_list().contains("max-h-0"));
    drop(shown);
    drop(panel);
    assert!(
        weak.upgrade().is_some(),
        "the mounted action retains its panel"
    );
    drop(card);
    assert!(
        weak.upgrade().is_none(),
        "released actions free the captured panel"
    );
}

/// A panel pill is its own button beside the disclosure, never inside it: Tab reaches the pill
/// and then the disclosure, Enter and Space on the pill open the file without toggling, and Enter
/// on the disclosure toggles. Without a panel the pill is text the keyboard passes over.
pub(super) async fn keyboard_pills(view: &Rc<DomView>) {
    let panel = Panel::new();
    crate::presentation::store(&panel.panel, "made.rs");
    let before: HtmlElement = view
        .document
        .create_element("button")
        .unwrap()
        .unchecked_into();
    body().append_child(&before).unwrap();
    for supplied in [Some(&panel), None] {
        keyboard_pill(view, supplied, &before).await;
    }
    before.remove();
}
/// Walk one pill card's controls by keyboard, starting after `before`.
async fn keyboard_pill(view: &DomView, panel: Option<&Panel>, before: &HtmlElement) {
    use crate::contributions_support::{disclosure, driver, focused};
    let shown = pill_card(&"made.rs".into(), panel);
    let card = Shown::new(shown.content.as_ref(), view);
    let root = card.root();
    let pill = select(&root, ".maestro-artifact-pill");
    let content = select(&root, ".maestro-tool-body");
    let toggle = disclosure(&super::header_of(&root)).unwrap();
    assert_eq!(pill.tag_name() == "BUTTON", panel.is_some());
    let parent = pill.parent_element().unwrap();
    assert!(
        parent.closest("button").unwrap().is_none(),
        "no nested control"
    );
    before.focus().unwrap();
    driver("driverPress", "Tab").await;
    if let Some(panel) = panel {
        pill_keys(&pill, panel).await;
        assert!(
            content.class_list().contains("max-h-0"),
            "the pill does not toggle"
        );
        driver("driverPress", "Tab").await;
    }
    assert!(focused(&toggle));
    driver("driverPress", "Enter").await;
    assert!(
        !content.class_list().contains("max-h-0"),
        "the disclosure toggles"
    );
}
/// The focused panel pill and the disclosure carry their names, and Enter and Space on the pill
/// each open its file.
async fn pill_keys(pill: &Element, panel: &Panel) {
    use crate::contributions_support::{driver, focused};
    assert!(focused(pill));
    let names = driver("driverAria", ".maestro-tool-toggle")
        .await
        .as_string()
        .unwrap();
    for name in ["made.rs", "Updating artifact made.rs"] {
        assert!(names.contains(&format!("button \"{name}\"")), "{names}");
    }
    for key in ["Enter", "Space"] {
        driver("driverPress", key).await;
    }
    assert_eq!(panel.presentation.opened.get(), 2);
}
