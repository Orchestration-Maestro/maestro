//! Canonical editor imports delegate to the host's editors.
use crate::{
    callback_imports::{CallbackView, retain},
    guest_calls::Callback,
    guest_calls::editors::TextCallbackEndpoint,
    import_route::push,
    keys_reads::KeysEndpoint,
    local_resources::effects::TuiEndpoint,
    widget_editors::{EditorBuild, EditorRequest, Read},
    widget_imports::{StyleSource, build, paint, widget},
    widget_list_imports::SelectListStyles,
    widget_reads::{Build, Request, WidgetEndpoint},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};

/// Editor styles as the guest sends them.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct EditorStyles {
    /// The border.
    #[component(name = "border-color")]
    border_color: StyleSource,
    /// The completion list.
    #[component(name = "select-list")]
    select_list: SelectListStyles,
}

/// Editor options as the guest sends them.
#[derive(ComponentType, Lift)]
#[component(record)]
struct EditorOptions {
    /// Side padding.
    #[component(name = "padding-x")]
    padding_x: Option<f64>,
    /// Completion rows.
    #[component(name = "autocomplete-max-visible")]
    autocomplete_max_visible: Option<f64>,
}

/// An editor request as the guest sends it.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum WireRequest {
    /// The text.
    #[component(name = "get-text")]
    GetText,
    /// The expanded text.
    #[component(name = "get-expanded-text")]
    GetExpandedText,
    /// The lines.
    #[component(name = "get-lines")]
    GetLines,
    /// The cursor.
    #[component(name = "get-cursor")]
    GetCursor,
    /// Adds text to the history.
    #[component(name = "add-to-history")]
    AddToHistory(String),
    /// The side padding.
    #[component(name = "get-padding-x")]
    GetPaddingX,
    /// Replaces the side padding.
    #[component(name = "set-padding-x")]
    SetPaddingX(f64),
    /// The completion rows.
    #[component(name = "get-autocomplete-max-visible")]
    GetAutocompleteMaxVisible,
    /// Replaces the completion rows.
    #[component(name = "set-autocomplete-max-visible")]
    SetAutocompleteMaxVisible(f64),
    /// Whether submission is disabled.
    #[component(name = "get-disable-submit")]
    GetDisableSubmit,
    /// Disables or enables submission.
    #[component(name = "set-disable-submit")]
    SetDisableSubmit(bool),
    /// Whether the completion list shows.
    #[component(name = "is-showing-autocomplete")]
    IsShowingAutocomplete,
    /// Whether the editor has the focus.
    #[component(name = "get-focused")]
    GetFocused,
    /// Gives or takes the focus.
    #[component(name = "set-focused")]
    SetFocused(bool),
    /// Replaces the border style.
    #[component(name = "set-border-color")]
    SetBorderColor(StyleSource),
}

/// The request a wire request names, retaining an authored border style.
fn read<T: CallbackView>(store: &mut T, request: WireRequest) -> wasmtime::Result<Read> {
    Ok(match request {
        WireRequest::GetText => Read::GetText,
        WireRequest::GetExpandedText => Read::GetExpandedText,
        WireRequest::GetLines => Read::GetLines,
        WireRequest::GetCursor => Read::GetCursor,
        WireRequest::AddToHistory(text) => Read::AddToHistory(text),
        WireRequest::GetPaddingX => Read::GetPaddingX,
        WireRequest::SetPaddingX(padding) => Read::SetPaddingX(padding),
        WireRequest::GetAutocompleteMaxVisible => Read::GetAutocompleteMaxVisible,
        WireRequest::SetAutocompleteMaxVisible(max) => Read::SetAutocompleteMaxVisible(max),
        WireRequest::GetDisableSubmit => Read::GetDisableSubmit,
        WireRequest::SetDisableSubmit(disabled) => Read::SetDisableSubmit(disabled),
        WireRequest::IsShowingAutocomplete => Read::IsShowingAutocomplete,
        WireRequest::GetFocused => Read::GetFocused,
        WireRequest::SetFocused(focused) => Read::SetFocused(focused),
        WireRequest::SetBorderColor(style) => Read::SetBorderColor(paint(store, &style)?),
    })
}

/// Builds an editor, a custom one with `keybindings`.
fn editor<T: CallbackView>(
    store: &mut T,
    (tui, theme): (&Resource<TuiEndpoint>, &EditorStyles),
    keybindings: Option<&Resource<KeysEndpoint>>,
    options: &EditorOptions,
) -> wasmtime::Result<Result<Resource<WidgetEndpoint>, String>> {
    let options = (options.padding_x, options.autocomplete_max_visible);
    let editor = editor_build(store, (tui, theme), keybindings, options)?;
    build(store, Build::Editor(editor))
}

/// What an editor is built from: the terminal, its styles, a manager and options.
pub(crate) fn editor_build<T: CallbackView>(
    store: &mut T,
    (tui, theme): (&Resource<TuiEndpoint>, &EditorStyles),
    keybindings: Option<&Resource<KeysEndpoint>>,
    options: (Option<f64>, Option<f64>),
) -> wasmtime::Result<EditorBuild> {
    let tui = store.ctx().table.get(tui)?.clone();
    let keybindings = keybindings
        .map(|keys| store.ctx().table.get(keys).cloned())
        .transpose()?;
    Ok(EditorBuild {
        tui,
        border: paint(store, &theme.border_color)?,
        list: theme.select_list.paints(store)?,
        keybindings,
        options,
    })
}

/// Installs the editor constructors and methods.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    authored(host)?;
    host.func_wrap(
        "new-editor",
        |mut store: StoreContextMut<'_, T>,
         (tui, theme, options): (Resource<TuiEndpoint>, EditorStyles, EditorOptions)| {
            Ok((editor(store.data_mut(), (&tui, &theme), None, &options)?,))
        },
    )?;
    host.func_wrap(
        "new-custom-editor",
        |mut store: StoreContextMut<'_, T>,
         (tui, theme, keys, options): (
            Resource<TuiEndpoint>,
            EditorStyles,
            Resource<KeysEndpoint>,
            EditorOptions,
        )| {
            Ok((editor(
                store.data_mut(),
                (&tui, &theme),
                Some(&keys),
                &options,
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.editor",
        |mut store: StoreContextMut<'_, T>,
         (this, request): (Resource<WidgetEndpoint>, WireRequest)| {
            let read = read(store.data_mut(), request)?;
            let request = |reply| Request::Editor(EditorRequest::Read(read, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    for (method, insert) in [("set-editor-text", false), ("insert-editor-text", true)] {
        host.func_wrap(
            &format!("[method]widget.{method}"),
            move |mut store: StoreContextMut<'_, T>,
                  (this, text): (Resource<WidgetEndpoint>, String)| {
                let request = |reply| Request::Editor(EditorRequest::Edit(text, insert, reply));
                let route: crate::import_route::Route =
                    widget(store.data_mut(), &this)?.route(request);
                Ok((push(store.data_mut(), route)?,))
            },
        )?;
    }
    host.func_wrap(
        "[method]widget.on-action",
        |mut store: StoreContextMut<'_, T>,
         (this, action, handler): (Resource<WidgetEndpoint>, String, Resource<Callback>)| {
            let handler = retain(store.data_mut(), &handler)?;
            let request = |reply| Request::Editor(EditorRequest::OnAction(action, handler, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    Ok(())
}

/// Installs the lent text callbacks, border style lending and widget sharing an authored editor
/// uses.
fn authored<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, TextCallbackEndpoint>(host, "text-callback")?;
    host.func_wrap(
        "[method]text-callback.call",
        |mut store: StoreContextMut<'_, T>,
         (this, text): (Resource<TextCallbackEndpoint>, String)| {
            let callback = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((push(store.data_mut(), callback.call(text))?,))
        },
    )?;
    host.func_wrap(
        "[method]widget.border-color",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let request = |reply| Request::Editor(EditorRequest::BorderColor(reply));
            let style = match widget(store.data_mut(), &this)?.read(request) {
                Ok(style) => Ok(store.data_mut().ctx().table.push(style)?),
                Err(error) => Err(error),
            };
            Ok((style,))
        },
    )?;
    host.func_wrap(
        "[method]widget.set-autocomplete-provider",
        |mut store: StoreContextMut<'_, T>,
         (this, provider): (Resource<WidgetEndpoint>, Resource<Callback>)| {
            let provider = retain(store.data_mut(), &provider)?;
            let request = |reply| Request::Editor(EditorRequest::Provider(provider, reply));
            Ok((widget(store.data_mut(), &this)?.read(request),))
        },
    )?;
    host.func_wrap(
        "[method]widget.share",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<WidgetEndpoint>,)| {
            let shared = widget(store.data_mut(), &this)?;
            Ok((store.data_mut().ctx().table.push(shared)?,))
        },
    )
}
