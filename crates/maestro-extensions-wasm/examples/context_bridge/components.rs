//! Authored renderers and views whose operands, state and lifetimes native witnesses observe.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionAPI, ExtensionResult, FlagOptions,
    FlagType, FlagValue,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Registers the echoing, scoped and replaceable renderers.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let echo_api = api.clone();
    api.register_message_renderer(
        "echo-view",
        Rc::new(move |message, options, _, emitter| {
            let label = super::ordering::label(&echo_api)?;
            let message_json = serde_json::to_value(&message).map_err(|error| error.to_string())?;
            let forwarded = json!({"message": message_json, "expanded": options.expanded});
            emitter.emit(&format!("payload:{label}"), forwarded)?;
            match message.custom_type.as_str() {
                "none" => Ok(None),
                "error" => Err("renderer refused".into()),
                "plain" => Ok(Some(Component::new(Plain::default()))),
                "preference" => Ok(Some(Component::new(Preference {
                    api: echo_api.clone(),
                    release: Cell::new(false),
                }))),
                _ => Ok(Some(Component::new(State::default()))),
            }
        }),
    )?;
    let scoped_api = api.clone();
    api.register_message_renderer(
        "scoped-view",
        Rc::new(move |_, _, _, emitter| {
            scope(&scoped_api, &emitter, "create")?;
            Ok(Some(Component::new(Scoped {
                api: scoped_api.clone(),
            })))
        }),
    )?;
    swap(api, "v1".into())
}

/// Registers, or replaces, renderer `swap-view` with views named after `version`.
pub(super) fn swap(api: &ExtensionAPI, version: String) -> ExtensionResult<()> {
    let view_api = api.clone();
    api.register_message_renderer(
        "swap-view",
        Rc::new(move |message, _, _, _| {
            Ok(Some(Component::new(super::View {
                api: view_api.clone(),
                name: format!("{}:{version}", message.custom_type),
            })))
        }),
    )
}

/// Traces the start and end of `method` around one emission to other listeners.
fn scope(api: &ExtensionAPI, emitter: &CallbackEmitter<'_>, method: &str) -> ExtensionResult<()> {
    let label = super::ordering::label(api)?;
    let trace = format!("trace:{label}");
    emitter.emit(&trace, json!(format!("{label}:{method}:start")))?;
    emitter.emit("component-foreign", Value::Null)?;
    emitter.emit(&trace, json!(format!("{label}:{method}:end")))
}

/// A view whose lines show every input and invalidation it retained.
#[derive(Default)]
struct State {
    /// Literal inputs in arrival order.
    inputs: RefCell<Vec<String>>,
    /// Invalidations so far.
    invalidated: Cell<u32>,
}

impl ComponentView for State {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![
            format!("width:{width}"),
            String::new(),
            "dup".into(),
            "dup".into(),
            "\u{1b}[1m\u{96ea}\u{1b}[22m".into(),
            format!("inputs:{}", self.inputs.borrow().join("|")),
            format!("invalidated:{}", self.invalidated.get()),
        ])
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.inputs.borrow_mut().push(data.into());
        Ok(())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.invalidated.set(self.invalidated.get() + 1);
        Ok(())
    }
}

/// A view that keeps the default input and key-release behaviour.
#[derive(Default)]
struct Plain {
    /// Renders so far.
    renders: Cell<u32>,
}

impl ComponentView for Plain {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.renders.set(self.renders.get() + 1);
        Ok(vec![format!("renders:{}", self.renders.get())])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A view whose key-release preference input switches on and off.
struct Preference {
    /// Original API for trace labels.
    api: ExtensionAPI,
    /// Current preference.
    release: Cell<bool>,
}

impl ComponentView for Preference {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("release:{}", self.release.get())])
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.release.set(data == "on");
        Ok(())
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        let label = super::ordering::label(&self.api)?;
        emitter.emit(
            &format!("trace:{label}"),
            json!(format!("{label}:preference")),
        )?;
        emitter.emit("component-foreign", Value::Null)?;
        Ok(self.release.get())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A view whose every method emits to other listeners; its release registers a flag.
struct Scoped {
    /// Original API, including during the destructor.
    api: ExtensionAPI,
}

impl ComponentView for Scoped {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        scope(&self.api, &emitter, "render")?;
        Ok(vec![format!("scoped:{width}")])
    }
    fn handle_input(&self, _: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        scope(&self.api, &emitter, "input")
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        scope(&self.api, &emitter, "preference")?;
        Ok(true)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        scope(&self.api, &emitter, "invalidate")
    }
}

impl Drop for Scoped {
    fn drop(&mut self) {
        let _ = self.api.register_flag(
            "dropped-scoped",
            FlagOptions {
                description: None,
                type_: FlagType::Boolean,
                default: Some(FlagValue::Boolean(true)),
            },
        );
    }
}
