//! Recorded card operands observed through native document and final-clip paint.
#![cfg(test)]
use crate::{
    markdown_support::{Surface, check, js, publish},
    tool_renderer_browser as browser,
};
use maestro_app::JsonObject;
use maestro_web::{
    BashRenderer, CalculateRenderer, GetCurrentTimeRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolRenderState},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub params: Option<JsonObject>,
    pub result: Option<maestro_app::ToolResultMessage>,
    pub is_streaming: bool,
    pub language: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    pub is_custom: bool,
    pub header: String,
    pub spinner: usize,
    pub status: String,
    pub console: Vec<Console>,
    pub errors: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    pub content: String,
    pub variant: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub tool: String,
    pub test: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    pub tool: String,
    pub kind: String,
    pub phase: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Localized {
    pub tool: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Fixture {
    pub rows: Vec<Row>,
    pub domains: Vec<Domain>,
    pub localized: Vec<Localized>,
    pub registration: Registration,
    pub value_routes: Vec<ValueRoute>,
    pub native_detail: NativeDetail,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Registration {
    pub initial_names: Vec<String>,
    pub initial: Expected,
    pub replaced: bool,
    pub forced: Expected,
    pub restored: bool,
    pub removed: Expected,
    pub still_absent: bool,
    pub retained_original: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRoute {
    pub input: ValueInput,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValueInput {
    pub params: serde_json::Value,
    pub result: maestro_app::ToolResultMessage,
    pub is_streaming: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NativeDetail {
    pub input: Input,
    pub expected: Expected,
    pub detail_reads: usize,
}

pub fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/tool_cards.json")).unwrap()
}
pub fn surface() -> Surface {
    Surface::new()
}
pub fn prepare(tool: &str, input: &Input) -> ToolRenderResult {
    assert!(input.language.is_none());
    let renderer: &dyn ToolRenderer = match tool {
        "bash" => &BashRenderer,
        "calculate" => &CalculateRenderer,
        "time" => &GetCurrentTimeRenderer,
        _ => panic!("unknown card"),
    };
    renderer
        .render(
            input.params.as_ref(),
            input.result.as_ref().map(Into::into),
            input.is_streaming,
        )
        .unwrap()
}
pub fn observe(
    surface: &mut Surface,
    recipe: &ToolRenderResult,
    retained: &mut ToolRenderState,
    expected: &Expected,
) {
    assert_eq!(recipe.is_custom, expected.is_custom);
    browser::draw(surface, recipe, retained);
    publish(
        "expectedCard",
        &serde_json::to_string(expected).unwrap().into(),
    );
    check(
        "document.querySelector('.maestro-tool-caption').textContent === JSON.parse(expectedCard).header",
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-console pre')].map(n=>({content:n.textContent,variant:n.classList.contains('maestro-console-error')?'error':'default'}))) === JSON.stringify(JSON.parse(expectedCard).console)",
    );
    check(
        "JSON.stringify([...document.querySelectorAll('.maestro-message-error')].map(n=>n.textContent)) === JSON.stringify(JSON.parse(expectedCard).errors)",
    );
    check(
        "document.querySelectorAll('.maestro-console script,.maestro-tool-caption script,.maestro-message-error script').length === 0",
    );
    let glyphs = browser::glyphs(surface);
    assert_eq!(
        glyphs.len(),
        1 + expected.spinner
            + expected.console.len()
            + usize::try_from(
                js_sys::Array::from(&js("document.querySelectorAll('.maestro-code')")).length()
            )
            .unwrap()
    );
    assert_eq!(
        expected.spinner,
        usize::from(expected.status == "inprogress")
    );
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    browser::has_role(
        surface,
        glyphs[0],
        match expected.status.as_str() {
            "inprogress" => "text",
            "complete" => "success",
            "error" => "error",
            _ => panic!("invalid status"),
        },
        &presentation,
    );
}
pub fn rows(test: &str) {
    let mut surface = surface();
    let rows: Vec<_> = fixture()
        .rows
        .into_iter()
        .filter(|row| row.test == test)
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(!row.id.is_empty());
        let recipe = prepare(&row.tool, &row.input);
        let mut retained = ToolRenderState::default();
        observe(&mut surface, &recipe, &mut retained, &row.expected);
        drop(retained);
        check(
            "document.querySelectorAll('.maestro-console,.maestro-tool-caption,.maestro-message-error').length === 0",
        );
    }
}
pub mod composed;
pub mod disclosure;
pub mod lifetimes;
pub mod styles;
