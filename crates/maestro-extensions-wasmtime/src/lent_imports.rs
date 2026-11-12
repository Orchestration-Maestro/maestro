//! Canonical lent style, highlighting, row style, submenu closer and text callback methods.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{
        HostHighlighter, HostRowStyle, HostStyle, HostSubmenuDone, HostTextCallback,
    },
    context_imports::add_resource,
    guest_calls::editors::TextCallbackEndpoint,
    import_route::{Route, push},
    lifecycle::State,
    read_imports::{endpoint, forwards, release},
    widget_lists::{RowStyleEndpoint, SubmenuDoneEndpoint},
    widget_reads::{HighlighterEndpoint, StyleEndpoint},
};
use wasmtime::component::{LinkerInstance, Resource};

impl HostStyle for State {
    fn apply(
        &mut self,
        this: Resource<StyleEndpoint>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.apply(text))
    }

    fn drop(&mut self, this: Resource<StyleEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostHighlighter for State {
    fn highlight(
        &mut self,
        this: Resource<HighlighterEndpoint>,
        code: String,
        lang: Option<String>,
    ) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(endpoint(self, &this)?.highlight(code, lang))
    }

    fn drop(&mut self, this: Resource<HighlighterEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostRowStyle for State {
    fn apply(
        &mut self,
        this: Resource<RowStyleEndpoint>,
        text: String,
        selected: bool,
    ) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.apply(text, selected))
    }

    fn drop(&mut self, this: Resource<RowStyleEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostSubmenuDone for State {
    fn done(
        &mut self,
        this: Resource<SubmenuDoneEndpoint>,
        value: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.done(value);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<SubmenuDoneEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

impl HostTextCallback for State {
    fn call(
        &mut self,
        this: Resource<TextCallbackEndpoint>,
        text: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.call(text);
        push(self, route)
    }

    fn drop(&mut self, this: Resource<TextCallbackEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the methods of every lent style, highlighting and callback to their traits.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, StyleEndpoint>(host, "style")?;
    add_resource::<State, HighlighterEndpoint>(host, "highlighter")?;
    add_resource::<State, RowStyleEndpoint>(host, "row-style")?;
    add_resource::<State, SubmenuDoneEndpoint>(host, "submenu-done")?;
    add_resource::<State, TextCallbackEndpoint>(host, "text-callback")?;
    forwards! { host, "style", HostStyle, StyleEndpoint,
        "apply" => apply(text: String);
    }
    forwards! { host, "highlighter", HostHighlighter, HighlighterEndpoint,
        "highlight" => highlight(code: String, lang: Option<String>);
    }
    forwards! { host, "row-style", HostRowStyle, RowStyleEndpoint,
        "apply" => apply(text: String, selected: bool);
    }
    forwards! { host, "submenu-done", HostSubmenuDone, SubmenuDoneEndpoint,
        "done" => done(value: Option<String>);
    }
    forwards! { host, "text-callback", HostTextCallback, TextCallbackEndpoint,
        "call" => call(text: String);
    }
    Ok(())
}
