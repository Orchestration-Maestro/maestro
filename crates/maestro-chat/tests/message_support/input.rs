//! Strict fixture operands and operations.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub test: String,
    #[serde(rename = "case")]
    pub name: String,
    pub input: Input,
    pub expected: Vec<Vec<String>>,
    /// Complete macOS layout when its key names differ.
    pub expected_macos: Option<Vec<Vec<String>>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub kind: String,
    pub width: usize,
    pub text: Option<String>,
    pub message: Option<AssistantFields>,
    pub tokens: Option<serde_json::Value>,
    pub hide: Option<bool>,
    pub label: Option<String>,
    pub styles_enabled: Option<bool>,
    pub custom_markdown: Option<bool>,
    pub palette: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub ops: Vec<Operation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssistantFields {
    pub content: Option<Vec<Content>>,
    pub stop_reason: Option<String>,
    pub error_message: Option<String>,
}
#[derive(Deserialize, serde::Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "toolCall")]
    Tool {
        id: String,
        name: String,
        arguments: BTreeMap<String, String>,
    },
}
pub struct Operation {
    pub action: Action,
    pub render: Option<bool>,
}
#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub enum Action {
    #[serde(rename = "update")]
    Update { message: AssistantFields },
    #[serde(rename = "mutate")]
    Mutate { message: AssistantFields },
    #[serde(rename = "hide")]
    Hide { value: bool },
    #[serde(rename = "label")]
    Label { value: String },
    #[serde(rename = "expand")]
    Expand { value: bool },
    #[serde(rename = "invalidate")]
    Invalidate {},
    #[serde(rename = "theme")]
    Theme { value: String },
    #[serde(rename = "keys")]
    Keys { value: Vec<String> },
    #[serde(rename = "captureChildren")]
    CaptureChildren {},
    #[serde(rename = "clear")]
    Clear {},
    #[serde(rename = "appendAlias")]
    AppendAlias { text: String },
    #[serde(rename = "append")]
    Append { text: String },
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let mut raw = serde_json::Value::deserialize(decoder)?;
        let object = raw
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("operation must be an object"))?;
        let render = object
            .shift_remove("render")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let action = serde_json::from_value(raw).map_err(serde::de::Error::custom)?;
        Ok(Self { action, render })
    }
}
