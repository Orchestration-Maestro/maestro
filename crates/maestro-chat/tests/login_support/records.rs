//! Strict input and observable frame records.
use crate::bindings::Binding;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
/// One attributed corpus query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Public witness owner.
    pub test: String,
    /// Constructor observations and operations.
    pub input: Query,
    /// Independently recorded frames.
    pub expected: Frames,
}
/// Provider metadata observed during construction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// Literal identity.
    pub id: String,
    /// Display name, possibly empty.
    pub name: String,
}
/// Constructor observations with source defaults.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Query {
    /// Registry observations.
    pub providers: Option<Vec<Provider>>,
    /// Literal provider identity.
    pub provider_id: Option<String>,
    /// Nonempty override.
    pub provider_name_override: Option<String>,
    /// Present override.
    pub title_override: Option<String>,
    /// Host platform observation.
    pub platform: Option<String>,
    /// Initial active bindings.
    pub bindings: Option<BTreeMap<String, Binding>>,
    /// Completion reenters progress and cancellation.
    pub reenter: bool,
    /// Viewport cells.
    pub width: Option<usize>,
    /// Ordered actions.
    pub ops: Vec<Operation>,
}
/// One public operation; object alternatives reject unread members.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Operation {
    /// Outer input.
    Key(String),
    /// Authorization presentation.
    Auth {
        auth: String,
        instructions: Option<String>,
    },
    /// Manual entry.
    Manual { manual: String },
    /// Prompt entry.
    Prompt {
        prompt: String,
        placeholder: Option<String>,
    },
    /// Replace information.
    Info { info: Vec<String> },
    /// Append waiting text.
    Waiting { waiting: String },
    /// Append progress text.
    Progress { progress: String },
    /// Change retained input.
    Value {
        #[serde(rename = "inputValue")]
        value: String,
    },
    /// Direct child input.
    Child {
        #[serde(rename = "childInput")]
        input: String,
    },
    /// Outer focus property.
    Focus { focus: bool },
    /// Independent child focus.
    InputFocus {
        #[serde(rename = "inputFocus")]
        focus: bool,
    },
    /// Publish foreground palette.
    Theme { theme: u8 },
    /// Live binding replacement.
    Bindings { bindings: BTreeMap<String, Binding> },
    /// Invalidate caches.
    Invalidate { invalidate: bool },
    /// Clear outer appearance.
    ClearOuter {
        #[serde(rename = "clearOuter")]
        clear: bool,
    },
    /// Restore retained child-array identity.
    RestoreOuter {
        #[serde(rename = "restoreOuter")]
        restore: bool,
    },
    /// Clear retained content.
    ClearContent {
        #[serde(rename = "clearContent")]
        clear: bool,
    },
    /// Caller child appended without padding.
    Append { append: String },
}
/// Every observable boundary, in order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frames {
    /// Initial frame and one frame per action.
    pub frames: Vec<Frame>,
}
/// Exact output including repeated notifications and payloads.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Styled terminal rows.
    pub lines: Vec<String>,
    /// Render requests so far.
    pub renders: usize,
    /// Literal launch operands.
    pub launch_urls: Vec<String>,
    /// Ordered abort and completion observations.
    pub events: Vec<Vec<Value>>,
    /// Settled input origin, category and payload.
    pub outcomes: Vec<(usize, String, String)>,
    /// Shared signal observation.
    pub aborted: bool,
    /// Retained input observation for attributed groups only.
    pub value: Option<String>,
    /// Independent parent and child flags.
    pub focus: Option<[bool; 2]>,
}
