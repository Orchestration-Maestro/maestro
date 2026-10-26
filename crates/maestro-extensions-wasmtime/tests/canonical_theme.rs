//! Native renderer themes stay live through independently retained component handles.
#![cfg(test)]
use maestro_extensions::{Local, MessageRenderOptions, create_event_bus_for};
use maestro_theme::{
    ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeDirectories, ThemeOptions, ThemeState,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn canonical_theme_name() {
    theme("name", &json!(" literal theme "));
}
#[test]
fn canonical_theme_source_path() {
    theme("source-path", &json!("./literal/../theme"));
}
#[test]
fn canonical_theme_source_info() {
    theme("source-info", &source());
}
#[test]
fn canonical_theme_set_source_info() {
    theme("set-source-info", &edited());
}
#[test]
fn canonical_theme_fg() {
    theme("fg", &json!("\u{1b}[38;5;7m literal text\r\n雪 \u{1b}[39m"));
}
#[test]
fn canonical_theme_bg() {
    theme("bg", &json!("\u{1b}[48;5;8m literal text\r\n雪 \u{1b}[49m"));
}
#[test]
fn canonical_theme_bold() {
    theme(
        "bold",
        &json!("\u{1b}[1m literal text\u{1b}[22m\r\n\u{1b}[1m雪 \u{1b}[22m"),
    );
}
#[test]
fn canonical_theme_italic() {
    theme(
        "italic",
        &json!("\u{1b}[3m literal text\u{1b}[23m\r\n\u{1b}[3m雪 \u{1b}[23m"),
    );
}
#[test]
fn canonical_theme_underline() {
    theme(
        "underline",
        &json!("\u{1b}[4m literal text\u{1b}[24m\r\n\u{1b}[4m雪 \u{1b}[24m"),
    );
}
#[test]
fn canonical_theme_inverse() {
    theme(
        "inverse",
        &json!("\u{1b}[7m literal text\u{1b}[27m\r\n\u{1b}[7m雪 \u{1b}[27m"),
    );
}
#[test]
fn canonical_theme_strikethrough() {
    theme(
        "strikethrough",
        &json!("\u{1b}[9m literal text\u{1b}[29m\r\n\u{1b}[9m雪 \u{1b}[29m"),
    );
}
#[test]
fn canonical_theme_get_fg_ansi() {
    theme("get-fg-ansi", &json!("\u{1b}[38;5;7m"));
}
#[test]
fn canonical_theme_get_bg_ansi() {
    theme("get-bg-ansi", &json!("\u{1b}[48;5;8m"));
}
#[test]
fn canonical_theme_get_color_mode() {
    theme("get-color-mode", &json!("color256"));
}

/// Original provenance shared with a caller outside the component.
fn source() -> Value {
    json!({"path":"./original", "source":" literal original ", "scope":"user", "origin":"top-level"})
}
/// Provenance the component writes without changing authored paths.
fn edited() -> Value {
    json!({"path":"./edited/../theme", "source":" literal source ", "scope":"project", "origin":"package", "baseDir":"./base"})
}
/// Prepares colors without filesystem access or terminal environment policy.
fn prepared() -> Rc<Theme> {
    Rc::new(
        Theme::new(
            [(
                ThemeColor::Named(" literal color ".into()),
                ColorValue::Index(7),
            )],
            [(
                ThemeBg::Named(" literal color ".into()),
                ColorValue::Index(8),
            )],
            ColorMode::Color256,
            ThemeOptions {
                name: Some(" literal theme ".into()),
                source_path: Some("./literal/../theme".into()),
                source_info: Some(Rc::new(RefCell::new(
                    serde_json::from_value(source()).unwrap(),
                ))),
            },
        )
        .unwrap(),
    )
}
/// Loads a real authored renderer, then invokes its retained theme through a native component.
fn theme(method: &str, expected: &Value) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(Rc::new(|_| panic!("no tails")), Rc::new(|_| panic!("no diagnostics")));
        let (_, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let state = ThemeState::new(ThemeDirectories { themes_dir: String::new(), custom_themes_dir: String::new() });
        let original = prepared();
        let provenance = original.source_info().unwrap();
        state.set_theme_instance(original.clone()).unwrap();
        let renderer = runner.get_message_renderer::<Value>("bridge-view").unwrap();
        let message = serde_json::from_value(json!({"role":"custom", "customType":format!("theme {method}"), "content":"", "display":true, "timestamp":0})).unwrap();
        let component = bus.invoke_callback(|emitter| renderer(Rc::new(message), MessageRenderOptions { expanded: false }, state.theme(), emitter)).unwrap().unwrap();
        if ["fg", "bg", "get-fg-ansi", "get-bg-ansi"].contains(&method) {
            let plane = if method.contains("bg") { "background color" } else { "color" };
            assert_eq!(component.try_render(0).unwrap_err().to_string(), format!("Unknown theme {plane}:  missing color "));
        }
        assert_eq!(component.try_render(80).unwrap(), [expected.to_string()]);
        if method == "set-source-info" {
            assert_eq!(serde_json::to_value(&*original.source_info().unwrap().borrow()).unwrap(), edited());
            assert_eq!(serde_json::to_value(&*provenance.borrow()).unwrap(), source());
            component.input_handler().unwrap().try_handle_input("clear").unwrap();
            assert!(original.source_info().is_none());
        }
        if ["name", "source-path", "source-info", "get-color-mode"].contains(&method) {
            state.set_theme_instance(Rc::new(Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap())).unwrap();
            let expected = if method == "get-color-mode" { json!("truecolor") } else { Value::Null };
            assert_eq!(component.try_render(80).unwrap(), [expected.to_string()]);
        }
        drop(component);
        bus.clear();
    });
}
