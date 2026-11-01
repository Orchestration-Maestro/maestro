//! Typed nested callback results returned to their original native caller.
use super::{Callback, Item, Resource, Returned};
use wasmtime::component::{ComponentType, Lift};

/// The nested call's own result, returned to the native caller that asked for it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(in crate::import_route) enum LocalResult {
    /// Original OAuth interaction result.
    #[component(name = "oauth")]
    OAuth(Result<Option<String>, String>),
    /// Exact optional fallback value or authored error.
    #[component(name = "auth-fallback")]
    AuthFallback(Result<Option<String>, String>),
    /// Optional retained component identity, or the authored error.
    #[component(name = "renderer")]
    Renderer(Result<Option<Resource<Callback>>, String>),
    /// Exact rendering lines or the authored error.
    #[component(name = "render")]
    Render(Result<Vec<String>, String>),
    /// Unit result or the authored error.
    #[component(name = "unit")]
    Unit(Result<(), String>),
    /// Key-release preference or the authored error.
    #[component(name = "key")]
    Key(Result<bool, String>),
    /// Optional focus state or an ordinary error.
    #[component(name = "focused")]
    Focused(Result<Option<bool>, String>),
    /// Prepared argument document or the authored error.
    #[component(name = "prepared")]
    Prepared(Result<crate::guest_exports::tools::PreparedArguments, String>),
    /// The terminal input listener's decision or its error.
    #[component(name = "terminal-input")]
    TerminalInput(Result<Option<crate::guest_calls::terminal::Decision>, String>),
    /// Unit result of a synchronous prefix, or the authored error.
    #[component(name = "synchronous")]
    Synchronous(Result<(), String>),
    /// The schema result with every authored diagnostic field.
    #[component(name = "schema")]
    Schema(Result<String, crate::schema_wire::Error>),
    /// Selected terminal classification or retained result.
    #[component(name = "stream")]
    Stream(Result<crate::stream_calls::WireSelection, String>),
    /// Original returned authored stream.
    #[component(name = "provider")]
    Provider(Result<Resource<Callback>, String>),
    /// Ordinary returned-stream operation result.
    #[component(name = "assistant")]
    Assistant(Result<Option<String>, String>),
    /// Event outcome, or the message rejecting the event before its handler ran.
    #[component(name = "event")]
    Event(Result<Returned, String>),
    /// Optional ordered completion candidates, or the authored error.
    #[component(name = "completion")]
    Completion(Result<Option<Vec<Item>>, String>),
    /// Tool result document, or the authored error.
    #[component(name = "tool")]
    Tool(Result<String, String>),
    /// Optional exit status, or the authored error.
    #[component(name = "bash")]
    Bash(Result<Option<f64>, String>),
    /// The built component identities, or the authored error.
    #[component(name = "factory")]
    Factory(Result<crate::guest_calls::factories::Built, String>),
    /// A completed custom prefix or its owned continuation.
    #[component(name = "custom-factory")]
    CustomFactory(Result<crate::guest_calls::custom::Built, String>),
    /// The rendered component and new row state identities, or the authored error.
    #[component(name = "tool-render")]
    ToolRender(Result<crate::guest_calls::tool_renders::Rendered, String>),
    /// Original read observation or structured diagnostic.
    #[component(name = "read")]
    Read(Result<crate::guest_calls::read::Output, crate::schema_wire::Error>),
    /// Original write completion or structured diagnostic.
    #[component(name = "write")]
    Write(Result<(), crate::schema_wire::Error>),
    /// Original content search observation or structured diagnostic.
    #[component(name = "grep")]
    Grep(Result<crate::guest_calls::grep::Output, crate::schema_wire::Error>),
    /// Original file search observation or structured diagnostic.
    #[component(name = "find")]
    Find(Result<crate::guest_calls::find::Output, crate::schema_wire::Error>),
    /// Original directory observation or structured diagnostic.
    #[component(name = "ls")]
    Ls(Result<crate::guest_calls::ls::Output, crate::schema_wire::Error>),
    /// Original edit observation or structured diagnostic.
    #[component(name = "edit")]
    Edit(Result<crate::guest_calls::read::Output, crate::schema_wire::Error>),
    /// The styled text, or the authored error.
    #[component(name = "style")]
    Style(Result<String, String>),
    /// The highlighted rows, or the authored error.
    #[component(name = "highlight")]
    Highlight(Result<Vec<String>, String>),
    /// The widget callback's answer, or the authored error.
    #[component(name = "widget")]
    Widget(Result<crate::guest_calls::widget_calls::Answer, String>),
    /// The built editor's identities, or the authored error.
    #[component(name = "editor-factory")]
    EditorFactory(Result<crate::guest_calls::editors::Built, String>),
    /// The authored editor's answer, or its error.
    #[component(name = "editor")]
    Editor(Result<crate::guest_calls::editors::Answer, String>),
    /// The authored completion provider's answer, or its error.
    #[component(name = "autocomplete")]
    Autocomplete(Result<crate::guest_calls::providers::Answer, String>),
    /// The returned completion provider's identity, or the authored error.
    #[component(name = "autocomplete-factory")]
    AutocompleteFactory(Result<Resource<Callback>, String>),
}
