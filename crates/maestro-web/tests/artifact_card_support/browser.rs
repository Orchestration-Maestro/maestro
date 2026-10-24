//! Real native text, status, selected children and disposal observations.
#![cfg(all(test, target_arch = "wasm32"))]
use super::browser_records::{self as records, Expected, Row};
use crate::{
    document_card_support::browser::open,
    markdown_support::{Surface, check, js, publish},
    tool_renderer_browser as drawing,
};
use maestro_web::tools::types::{ToolRenderResult, ToolRenderState};

/// Observe every card in a recorded command group.
/// # Panics
/// Panics if a native observation differs from its recorded value.
pub async fn rows(name: &str, count: usize) {
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards_browser.json")).unwrap();
    crate::support::strict_record::<Row>(&valid[name][0], &["input", "expected"]);
    let mut surface = Surface::new();
    let mut groups: std::collections::BTreeMap<String, Vec<Row>> =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards_browser.json")).unwrap();
    let rows = groups.remove(name).unwrap();
    assert_eq!(rows.len(), count);
    for (index, row) in rows.into_iter().enumerate() {
        publish("artifactIndex", &index.to_string().into());
        observe(
            &mut surface,
            &super::prepare(&row.input).unwrap(),
            &row.expected,
        )
        .await;
    }
}
/// Draw one card through its closed and opened states.
/// # Panics
/// Panics if a native observation differs from its recorded value.
pub async fn observe(surface: &mut Surface, recipe: &ToolRenderResult, expected: &Expected) {
    let mut state = header(surface, recipe, expected);
    body();
    if let Some(code) = expected.code.first() {
        drawing::click(
            surface,
            crate::markdown_support::first_slot_center(),
            recipe,
            &mut state,
        );
        crate::markdown_support::settle().await;
        assert_eq!(surface.effects.copies.borrow().last(), Some(&code.code));
        assert_eq!(surface.effects.copies.borrow().last(), Some(&code.decoded));
    }
    drop(state);
    check(
        "document.querySelectorAll('.maestro-tool-caption,.maestro-artifact-pill,.maestro-code,.maestro-console,.maestro-diff,.maestro-message-error').length===0",
    );
}
fn body() {
    assert_eq!(
        js("JSON.stringify([...document.querySelectorAll('.maestro-code code')].map(n=>({code:n.textContent,language:[...n.classList].find(c=>c.startsWith('language-')).slice(9)})))").as_string(),
        js("JSON.stringify(JSON.parse(artifactExpected).code.map(n=>({code:n.code.replaceAll('\\r\\n','\\n').replaceAll('\\r','\\n'),language:n.language})))").as_string(), "row {:?}", js("globalThis.artifactIndex"),
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-console pre')].map(n=>({content:n.textContent,variant:n.classList.contains('maestro-console-error')?'error':'default'})))===JSON.stringify(JSON.parse(artifactExpected).console)",
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-message-error')].map(n=>n.textContent))===JSON.stringify(JSON.parse(artifactExpected).errors)",
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-diff-scroll pre')].map(n=>({text:n.textContent,kind:n.classList.contains('maestro-diff-added')?'added':n.classList.contains('maestro-diff-removed')?'removed':'unchanged'})))===JSON.stringify(JSON.parse(artifactExpected).diff)",
    );
    check(
        "document.querySelectorAll('.maestro-tool-caption script,.maestro-artifact-pill script,.maestro-code script,.maestro-diff script').length===0",
    );
}
/// Observe the selected syntax label for every literal suffix.
/// # Panics
/// Panics if a native observation differs from its recorded value.
pub fn languages() {
    let mut surface = Surface::new();
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/artifact_languages_browser.json")).unwrap();
    crate::support::strict_record::<records::Language>(&valid[0], &["input", "expected"]);
    crate::support::strict_record::<super::Filename>(&valid[0]["input"], &[]);
    let cases: Vec<records::Language> =
        serde_json::from_str(include_str!("../fixtures/artifact_languages_browser.json")).unwrap();
    assert_eq!(cases.len(), 77);
    for row in cases {
        let mut params = serde_json::json!({"command":"create", "content":"literal"});
        if let Some(filename) = row.input.filename {
            params["filename"] = filename.into();
        }
        let recipe = super::value(Some(&params), "pending").unwrap();
        let mut state = ToolRenderState::default();
        drawing::draw(&mut surface, &recipe, &mut state);
        drawing::draw(&mut surface, &recipe, &mut state);
        open(&mut surface, &recipe, &mut state);
        assert_eq!(js("[...document.querySelector('.maestro-code code').classList].find(c=>c.startsWith('language-'))").as_string().unwrap(), format!("language-{}", row.expected));
        assert_eq!(
            js("document.querySelector('.maestro-code code').textContent")
                .as_string()
                .unwrap(),
            "literal"
        );
    }
}
/// Observe whole-value and selected-code inputs through native bodies.
/// # Panics
/// Panics if a native observation differs from its recorded value.
pub async fn fields() {
    let mut surface = Surface::new();
    let valid: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/artifact_value_routes_browser.json"
    ))
    .unwrap();
    crate::support::strict_record::<records::ValueRoute>(&valid[0], &["input", "expected"]);
    let routes: Vec<records::ValueRoute> = serde_json::from_str(include_str!(
        "../fixtures/artifact_value_routes_browser.json"
    ))
    .unwrap();
    assert_eq!(routes.len(), 39);
    for row in routes {
        let recipe = super::value(row.input.params.as_ref(), &row.input.phase).unwrap();
        observe(&mut surface, &recipe, &row.expected).await;
    }
    let unused: Vec<records::UnusedDiff> = serde_json::from_str(include_str!(
        "../fixtures/artifact_unused_diff_browser.json"
    ))
    .unwrap();
    for row in unused {
        let card = match row.expected {
            records::Branch::Rendered { card } => card,
            records::Branch::Error { kind } => {
                assert_eq!(kind, "TypeError");
                continue;
            }
        };
        let row = Row {
            input: row.input,
            expected: card,
        };
        observe(
            &mut surface,
            &super::prepare(&row.input).unwrap(),
            &row.expected,
        )
        .await;
    }
    let domains: Vec<records::CodeDomain> = serde_json::from_str(include_str!(
        "../fixtures/artifact_code_domains_browser.json"
    ))
    .unwrap();
    assert_eq!(domains.len(), 42);
    for row in domains {
        let recipe = super::value(Some(&row.input.params), &row.input.phase).unwrap();
        let mut state = ToolRenderState::default();
        drawing::draw(&mut surface, &recipe, &mut state);
        drawing::draw(&mut surface, &recipe, &mut state);
        open(&mut surface, &recipe, &mut state);
        let actual =
            js("document.querySelector('.maestro-code code')?.textContent??null").as_string();
        let expected = match row.expected {
            records::CodeSelection::Omitted {} => None,
            records::CodeSelection::Text { text } => Some(text),
        };
        assert_eq!(actual, expected, "{}", row.input.phase);
    }
}

/// Observe ordered native rows and direct diff properties.
/// # Panics
/// Panics if a native observation differs from its recorded value.
pub fn diffs() {
    use maestro_web::components::changes::{DiffProps, diff};
    let mut surface = Surface::new();
    let cases = super::diff_cases();
    assert_eq!(cases.len(), 48);
    for (index, row) in cases.into_iter().enumerate() {
        let recipe = ToolRenderResult {
            content: diff(DiffProps {
                old_text: row.input.old_text,
                new_text: row.input.new_text,
                title: row.input.title,
                class_name: row.input.class_name,
            }),
            is_custom: true,
        };
        let mut state = ToolRenderState::default();
        drawing::draw(&mut surface, &recipe, &mut state);
        drawing::draw(&mut surface, &recipe, &mut state);
        let observed: Vec<records::DiffRow> = serde_json::from_str(&js("JSON.stringify([...document.querySelectorAll('.maestro-diff-scroll pre')].map(n=>({text:n.textContent,kind:n.classList.contains('maestro-diff-added')?'added':n.classList.contains('maestro-diff-removed')?'removed':'unchanged'})))").as_string().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(observed).unwrap(),
            serde_json::to_value(row.expected.rows).unwrap(),
            "row {index}"
        );
        assert_eq!(
            js("document.querySelector('.maestro-diff-header span')?.textContent??null")
                .as_string(),
            row.expected.title,
            "row {index}"
        );
        assert_eq!(
            js("document.querySelector('.maestro-diff-counts')?.textContent??null").as_string(),
            row.expected.counts,
            "row {index}"
        );
        assert_eq!(
            js("document.querySelector('.maestro-diff').classList.contains('custom-token')")
                .as_bool(),
            Some(row.expected.class_present)
        );
        drop(state);
        check("document.querySelector('.maestro-diff')===null");
    }
}

/// Observe translated captions while retaining literal filenames and body data.
/// # Panics
/// Panics if a recorded translated branch differs.
pub async fn localized() {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/artifact_localized.json")).unwrap();
    assert_eq!(rows.len(), 21);
    let mut surface = Surface::new();
    for row in rows {
        surface
            .i18n
            .set_language(row.input.language.as_deref().unwrap())
            .unwrap();
        observe(
            &mut surface,
            &super::prepare(&row.input).unwrap(),
            &row.expected,
        )
        .await;
    }
}

/// Draw and observe the caption before exposing its existing body.
fn header(
    surface: &mut Surface,
    recipe: &ToolRenderResult,
    expected: &Expected,
) -> ToolRenderState {
    surface.time.get_or_insert(0.);
    assert_eq!(recipe.is_custom, expected.is_custom);
    let mut state = ToolRenderState::default();
    drawing::draw(surface, recipe, &mut state);
    drawing::draw(surface, recipe, &mut state);
    publish(
        "artifactExpected",
        &serde_json::to_string(expected).unwrap().into(),
    );
    assert_eq!(
        js("document.querySelector('.maestro-tool-caption').textContent")
            .as_string()
            .unwrap(),
        expected.caption
    );
    check(
        "(document.querySelector('.maestro-artifact-pill')?.textContent??null)===JSON.parse(artifactExpected).pill",
    );
    let glyphs = drawing::glyphs(surface);
    assert_eq!(
        glyphs.len(),
        1 + usize::from(expected.collapsible) + usize::from(expected.status == "inprogress")
    );
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    drawing::has_role(
        surface,
        glyphs[usize::from(expected.status == "inprogress" && expected.collapsible)],
        match expected.status.as_str() {
            "complete" => "success",
            "error" => "error",
            "inprogress" => "text",
            _ => panic!("unknown status"),
        },
        &presentation,
    );
    if expected.collapsible {
        assert_eq!(expected.initially_collapsed, Some(true));
        check(
            "[...document.querySelectorAll('.maestro-code,.maestro-console,.maestro-diff')].every(n=>{const r=n.getBoundingClientRect();return !n.contains(document.elementFromPoint(r.left+r.width/2,r.top+r.height/2))})",
        );
        open(surface, recipe, &mut state);
    } else {
        assert!(expected.initially_collapsed.is_none());
    }

    state
}
