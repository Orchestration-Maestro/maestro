//! Delegates rendering to a retained native definition through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionAPI, ExtensionResult, ReadToolOptions,
};
use std::rc::Rc;

/// Registers a wrapper that either returns the original component or replaces it.
pub(super) fn register(api: &ExtensionAPI, mode: &str) -> ExtensionResult<()> {
    let own = mode == "own";
    let missing = mode == "missing";
    let expanded = mode == "expanded";
    let mut tool = api.create_read_tool_definition("/".into(), ReadToolOptions::default())?;
    tool.metadata.name = "builtin-render".into();
    let call = tool
        .render_call
        .take()
        .ok_or("missing native call renderer")?;
    tool.render_call = Some(Rc::new(move |args, theme, context, emitter| {
        let count = context.state::<usize>();
        *count.borrow_mut() += 1;
        emitter.emit("builtin-render-count", serde_json::json!(*count.borrow()))?;
        (context.invalidate)()?;
        let mut delegated = context.clone();
        if missing {
            delegated.native_row = None;
        }
        if expanded {
            delegated.expanded = true;
        }
        let component = call(args, theme, &delegated, emitter)?;
        Ok(if own {
            Component::new(Own(component))
        } else {
            component
        })
    }));
    let result = tool
        .render_result
        .take()
        .ok_or("missing native result renderer")?;
    tool.render_result = Some(Rc::new(
        move |result_value, options, theme, context, emitter| {
            let count = context.state::<usize>();
            *count.borrow_mut() += 1;
            emitter.emit("builtin-render-count", serde_json::json!(*count.borrow()))?;
            (context.invalidate)()?;
            result(result_value, options, theme, context, emitter)
        },
    ));
    api.register_tool(tool)
}

/// A wrapper deliberately returning a different object from its native base.
struct Own(Component);
impl ComponentView for Own {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}
