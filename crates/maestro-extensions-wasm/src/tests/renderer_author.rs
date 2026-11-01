//! Authored renderer and component operations shared by both adapters.
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionAPI, ExtensionResult, Theme, ThemePort,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Selected authored behavior.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    /// Retained mutable authored state.
    Stateful,
    /// Both optional component methods use their defaults.
    Default,
    /// A returned component renders no lines.
    Empty,
    /// No component is returned.
    None,
    /// The renderer returns the selected authored error.
    Error,
    /// Renderer and component methods use their borrowed emitter.
    Emit,
    /// Shared component aliases retain independently supplied themes.
    Alias,
    /// Stateful component using the recorded source output format.
    Oracle,
    /// The author returns a supplied exact line vector.
    Lines,
}
/// Registration control input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Literal registration names, sharing a closure.
    names: Vec<String>,
    /// Authored behavior.
    mode: Mode,
    /// Optional authored error.
    error: Option<String>,
    /// Whether the final view destructor registers a callback.
    reenter: bool,
    /// Optional ordered lines supplied by an author.
    lines: Option<Vec<String>>,
    /// Optional initial live release preference.
    preference: Option<bool>,
    /// Selected registration marker.
    marker: Option<u32>,
}
/// Distinguishable independently supplied theme port; it answers no request.
struct LocalTheme(super::Released);
/// The answer of every request to the local theme.
fn unanswered<T>() -> ExtensionResult<T> {
    Err("local theme".into())
}
impl ThemePort for LocalTheme {
    fn name(&self) -> ExtensionResult<Option<String>> {
        unanswered()
    }
    fn source_path(&self) -> ExtensionResult<Option<String>> {
        unanswered()
    }
    fn source_info(&self) -> ExtensionResult<Option<maestro_extensions_wasm::SourceInfo>> {
        unanswered()
    }
    fn set_source_info(
        &self,
        _: Option<maestro_extensions_wasm::SourceInfo>,
    ) -> ExtensionResult<()> {
        unanswered()
    }
    fn fg(&self, _: &str, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn bg(&self, _: &str, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn bold(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn italic(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn underline(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn inverse(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn strikethrough(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn get_fg_ansi(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn get_bg_ansi(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
    fn get_color_mode(&self) -> ExtensionResult<maestro_extensions_wasm::ColorMode> {
        unanswered()
    }
    fn thinking_border(
        &self,
        _: maestro_extensions_wasm::ModelThinkingLevel,
        _: &str,
    ) -> ExtensionResult<String> {
        unanswered()
    }
    fn bash_mode_border(&self, _: &str) -> ExtensionResult<String> {
        unanswered()
    }
}
impl Drop for LocalTheme {
    fn drop(&mut self) {
        let _ = &self.0;
    }
}
/// Author state retained by shared component aliases.
struct View {
    /// Host facade for observations.
    api: ExtensionAPI,
    /// Current literal input.
    text: RefCell<String>,
    /// Invalidation counter.
    invalidations: Cell<u32>,
    /// Live release preference.
    release: Cell<bool>,
    /// Retained independently supplied themes.
    _themes: Rc<RefCell<Vec<Theme>>>,
    /// Emission behavior.
    mode: Mode,
    /// Selected returned failure.
    error: Option<String>,
    /// Destructor registration switch.
    reenter: bool,
    /// Optional exact authored lines.
    lines: Option<Vec<String>>,
}
impl View {
    /// Performs the immediate effect before the selected returned failure.
    fn effect(&self, operation: &str, emitter: &CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.mode == Mode::Emit {
            emitter.emit(operation, json!({"text":self.text.borrow().as_str(), "invalidations":self.invalidations.get()}))?;
        }
        if self.text.borrow().as_str() == "fail"
            && let Some(error) = &self.error
        {
            return Err(error.clone());
        }
        Ok(())
    }
}
impl Drop for View {
    fn drop(&mut self) {
        let _ = self.api.append_entry("view-drop", Some(json!("owned")));
        if self.reenter {
            let result = self
                .api
                .register_message_renderer("fresh-renderer", Rc::new(|_, _, _, _| Ok(None)));
            let _ = self
                .api
                .append_entry("renderer-reentered", Some(json!(result)));
        }
    }
}
impl ComponentView for View {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.effect("render", &emitter)?;
        if self.mode == Mode::Empty {
            return Ok(vec![]);
        }
        if self.mode == Mode::Lines {
            return Ok(self.lines.clone().unwrap_or_default());
        }
        if self.mode == Mode::Oracle {
            return Ok(vec![
                format!("width={width}"),
                self.text.borrow().clone(),
                format!("invalidations={}", self.invalidations.get()),
                String::new(),
                "repeat".into(),
                "repeat".into(),
            ]);
        }
        Ok(vec![
            format!(
                "{width}:{}:{}",
                self.text.borrow(),
                self.invalidations.get()
            ),
            String::new(),
            "a".into(),
            "z".into(),
            "\u{1b}[31m雪".into(),
        ])
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.text.replace(data.into());
        self.release.set(data == "release");
        self.effect("input", &emitter)
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.effect("release", &emitter)?;
        Ok(self.release.get())
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.invalidations.set(self.invalidations.get() + 1);
        self.effect("invalidate", &emitter)
    }
}
/// View using both optional-operation defaults.
struct Defaults(ExtensionAPI);
impl ComponentView for Defaults {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.append_entry("default-render", Some(json!(width)))?;
        Ok(vec![format!("default:{width}")])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
/// Installs a renderer and explicit alias control without a retained-owner cycle.
pub(super) fn register(api: &ExtensionAPI, text: &str) -> ExtensionResult<()> {
    let request: Request = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let themes = Rc::new(RefCell::new(Vec::<Theme>::new()));
    let view_themes = Rc::new(RefCell::new(vec![Theme::new(Rc::new(LocalTheme(
        super::Released::new(api, "local-theme"),
    )))]));
    let component = Component::new(View {
        api: api.clone(),
        text: RefCell::new(
            if request.mode == Mode::Oracle {
                "before"
            } else {
                "z"
            }
            .into(),
        ),
        invalidations: Cell::new(0),
        release: Cell::new(request.preference.unwrap_or(false)),
        _themes: Rc::clone(&view_themes),
        mode: request.mode,
        error: request.error.clone(),
        reenter: request.reenter,
        lines: request.lines.clone(),
    });
    let alias = Rc::new(RefCell::new(Some(component.clone())));
    let renderer = renderer(api, &request, component, Rc::clone(&themes), view_themes);
    for name in request.names {
        api.register_message_renderer(&name, Rc::clone(&renderer))?;
    }
    control(api, alias, themes)
}
/// Registers during final renderer-capture destruction.
struct Reenter(ExtensionAPI);
impl Drop for Reenter {
    fn drop(&mut self) {
        let result = self
            .0
            .register_message_renderer("capture-reentered", Rc::new(|_, _, _, _| Ok(None)));
        let _ = self
            .0
            .append_entry("capture-reentered", Some(json!(result)));
    }
}
/// Creates the shared authored closure with independently observable destruction.
fn renderer(
    api: &ExtensionAPI,
    request: &Request,
    component: Component,
    themes: Rc<RefCell<Vec<Theme>>>,
    view_themes: Rc<RefCell<Vec<Theme>>>,
) -> maestro_extensions_wasm::MessageRenderer {
    let (log, mode, error, marker) = (
        api.clone(),
        request.mode,
        request.error.clone(),
        request.marker,
    );
    let guard = super::Released::new(api, "renderer");
    let reenter = request.reenter.then(|| Reenter(api.clone()));
    Rc::new(move |message, options, theme, emitter| {
        let _ = (&guard, &reenter);
        log.append_entry("renderer-message", Some(json!({"message":message, "expanded":options.expanded, "negative_zero":message.timestamp == 0.0 && message.timestamp.is_sign_negative()})))?;
        if let Some(marker) = marker {
            log.append_entry("renderer-selected", Some(json!(marker)))?;
        }
        if mode == Mode::Alias {
            themes.borrow_mut().push(theme.clone());
            view_themes.borrow_mut().push(theme);
        }
        if mode == Mode::Emit {
            emitter.emit("renderer", json!(options.expanded))?;
        }
        if mode == Mode::Error || (mode == Mode::Emit && message.custom_type == "fail") {
            return Err(error.clone().unwrap_or_default());
        }
        Ok(match mode {
            Mode::None => None,
            Mode::Default => Some(Component::new(Defaults(log.clone()))),
            _ => Some(component.clone()),
        })
    })
}
/// Explicitly removes authored component and theme aliases before host teardown.
fn control(
    api: &ExtensionAPI,
    alias: Rc<RefCell<Option<Component>>>,
    themes: Rc<RefCell<Vec<Theme>>>,
) -> ExtensionResult<()> {
    api.register_command(
        "renderer-control",
        maestro_extensions_wasm::CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, _| {
                let (alias, themes) = (Rc::clone(&alias), Rc::clone(&themes));
                Box::pin(async move {
                    match args.as_str() {
                        "view" => {
                            drop(alias.take());
                        }
                        "themes" => {
                            drop(themes.take());
                        }
                        _ => return Err("unknown renderer control".into()),
                    }
                    Ok(())
                })
            }),
        },
    )
}

/// Selects only renderer-related capture operations.
pub(super) fn dispatch(api: &ExtensionAPI, args: &str) -> Option<ExtensionResult<()>> {
    args.strip_prefix("renderer-register ")
        .map(|request| register(api, request))
}
