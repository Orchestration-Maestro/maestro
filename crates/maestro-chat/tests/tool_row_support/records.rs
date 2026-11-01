//! Recorded row inputs; explicit null is retained at raw JSON boundaries.
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::BTreeMap;

/// Decode a present value without collapsing null into absence.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
/// Full source recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub colors: BTreeMap<String, u8>,
    pub backgrounds: BTreeMap<String, u8>,
    pub cases: Vec<Case>,
}
/// One complete invocation and its independent observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub test: String,
    pub name: String,
    pub input: Input,
    pub expected: Expected,
    /// Complete macOS layout when its key names differ.
    pub expected_macos: Option<Expected>,
}
/// Exact output with ordered callbacks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub frames: Vec<Vec<String>>,
    pub events: Vec<Value>,
    pub renders: usize,
}
/// Constructor and later operations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub tool: Option<String>,
    pub id: Option<String>,
    pub cwd: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub args: Option<Value>,
    pub args_json: Option<String>,
    pub definition: Option<Definition>,
    pub options: Option<Options>,
    pub protocol: Option<String>,
    pub width: Option<usize>,
    #[serde(default)]
    pub ops: Vec<Op>,
}
/// Renderer callbacks and independently optional shell.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub call: Option<String>,
    pub result: Option<String>,
    pub shell: Option<String>,
}
/// Supplied image policies, preserving omitted defaults.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub show_images: Option<bool>,
    pub image_width_cells: Option<f64>,
}
/// One operation with an optional viewport observation.
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Op {
    pub action: Operation,
}
/// Public row operations.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Args {
        #[serde(default, deserialize_with = "present")]
        value: Option<Value>,
    },
    Started {},
    Complete {},
    Result {
        value: ResultRecord,
        partial: Option<bool>,
    },
    Expand {
        value: bool,
    },
    Show {
        value: bool,
    },
    ImageWidth {
        value: f64,
    },
    Invalidate {},
    Theme {},
    Append {
        text: String,
    },
    Capture {},
    AliasAppend {
        text: String,
    },
    Clear {},
    Callback {
        slot: String,
    },
    Keys {
        value: Vec<String>,
    },
    Flush {},
}
/// Raw result payload consumed by the public update method.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultRecord {
    pub content: Vec<maestro_app::UserBlock>,
    #[serde(default, deserialize_with = "present")]
    pub details: Option<Value>,
    pub is_error: bool,
}
impl ResultRecord {
    /// Construct the row record without decoding native details.
    pub fn convert(&self) -> maestro_chat::ToolExecutionResult {
        maestro_chat::ToolExecutionResult {
            content: self.content.clone(),
            details: self.details.clone(),
            is_error: self.is_error,
        }
    }
}
