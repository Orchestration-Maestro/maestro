//! Heterogeneous bindings and the independent serialized observations.
use super::{Line, definition, environment};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use maestro_tools::{ToolRenderEnvironment, ToolRenderResult, ToolRendererBinding};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::{collections::HashMap, rc::Rc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case<I> {
    pub test: String,
    pub input: I,
    pub expected: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub mixed: Vec<Case<Mixed>>,
    pub native: Vec<Case<Native>>,
    pub documents: Vec<Case<Document>>,
}
fn present<'de, D: Deserializer<'de>>(decoder: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(decoder).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mixed {
    pub case: String,
    #[serde(default)]
    pub content: Vec<maestro_models::UserBlock>,
    #[serde(default, deserialize_with = "present")]
    pub details: Option<Value>,
    pub outer_details: Option<Value>,
    pub inner_details: Option<Value>,
    pub args: Option<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Native {
    pub name: String,
    pub args: Value,
    pub content: Value,
    pub details: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub entries: Value,
}
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/renderer_bindings.json")).unwrap()
}
pub fn native_environment() -> (Rc<Theme>, Rc<ToolRenderEnvironment>) {
    let theme = Rc::new(
        Theme::new(
            [
                ("toolTitle", 1),
                ("accent", 3),
                ("muted", 6),
                ("dim", 7),
                ("toolOutput", 2),
                ("warning", 5),
                ("error", 4),
                ("toolDiffAdded", 11),
                ("toolDiffRemoved", 12),
                ("toolDiffContext", 10),
            ]
            .into_iter()
            .map(|(name, index)| (ThemeColor::Named(name.into()), ColorValue::Index(index))),
            [
                ("toolPendingBg", 13),
                ("toolSuccessBg", 14),
                ("toolErrorBg", 15),
            ]
            .into_iter()
            .map(|(name, index)| (ThemeBg::Named(name.into()), ColorValue::Index(index))),
            ColorMode::Color256,
            ThemeOptions::default(),
        )
        .unwrap(),
    );
    let state = maestro_theme::ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        },
        Rc::new(maestro_theme::NativeThemeOperations),
    );
    state.set_theme_instance(Rc::clone(&theme)).unwrap();
    let mut environment = environment();
    Rc::get_mut(&mut environment).unwrap().live_theme = state.theme();
    (theme, environment)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    label: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Count {
    value: u32,
}
/// Different parameter, details and state types share one public lookup.
pub fn registry() -> HashMap<String, ToolRendererBinding> {
    let mut bindings = HashMap::new();
    for name in ["grep", "find", "ls"] {
        let tool = maestro_tools::create_tool_definition::<()>(
            name,
            "/work".into(),
            maestro_tools::ToolsOptions::default(),
            "/unused".into(),
            std::sync::Arc::new(maestro_tools::ToolsManager::new("/unused".into(), None)),
        )
        .unwrap();
        bindings.insert(name.into(), tool.renderer_binding().unwrap());
    }
    let mut text = definition::<(), String, Label, Vec<String>>();
    text.render_call = Some(Rc::new(|_, _, _| {
        Ok(Rc::new(Line("custom_text:call".into())))
    }));
    text.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Typed(
        Rc::new(|result, options, _, row| {
            row.state
                .borrow_mut()
                .push(result.details.unwrap().label.clone());
            Ok(Rc::new(Line(format!(
                "custom_text:{}:{}",
                result.details.unwrap().label,
                options.expanded
            ))))
        }),
    ));
    bindings.insert(
        "custom_text".into(),
        ToolRendererBinding::from_definition(&text),
    );
    let mut count = definition::<u32, bool, Count, u32>();
    count.render_call = Some(Rc::new(|_, _, _| {
        Ok(Rc::new(Line("custom_count:call".into())))
    }));
    count.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Typed(
        Rc::new(|result, options, _, row| {
            *row.state.borrow_mut() += result.details.unwrap().value;
            Ok(Rc::new(Line(format!(
                "custom_count:{}:{}",
                result.details.unwrap().value,
                options.expanded
            ))))
        }),
    ));
    bindings.insert(
        "custom_count".into(),
        ToolRendererBinding::from_definition(&count),
    );
    let mut raw = definition::<(), (), NeverDecode, ()>();
    raw.render_result = Some(ToolRenderResult::<_, _, maestro_models::Local>::Raw(
        Rc::new(|_, _, _, _| Ok(Rc::new(Line("raw".into())))),
    ));
    bindings.insert("raw".into(), ToolRendererBinding::from_raw(&raw).unwrap());
    bindings
}

/// A raw slot must never attempt this details decoder.
pub struct NeverDecode;
impl<'de> Deserialize<'de> for NeverDecode {
    fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        panic!("raw binding must never decode execution details")
    }
}
