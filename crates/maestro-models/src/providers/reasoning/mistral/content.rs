//! Selection of incoming text and thinking content.
use super::wire::{Nullable, SafeInteger, nullable};
use crate::providers::json_text::{member, object_record};
use serde::{Deserialize, Deserializer, de::Error as _};
use serde_json::value::RawValue;

/// Only content that contributes to the output.
pub(super) enum Content {
    /// Visible text, including an empty string.
    Text(String),
    /// Joined nonempty thinking text.
    Thinking(String),
}
/// Decoded content contributions in source order.
#[derive(Default)]
pub(super) struct Contents(pub Vec<Content>);
impl<'de> Deserialize<'de> for Contents {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(decoder)?;
        if raw.get() == "null" {
            return Ok(Self::default());
        }
        if let Ok(text) = serde_json::from_str::<String>(raw.get()) {
            return Ok(Self(vec![Content::Text(text)]));
        }
        let items: Vec<&RawValue> = serde_json::from_str(raw.get()).map_err(D::Error::custom)?;
        Ok(Self(items.into_iter().filter_map(select).collect()))
    }
}
/// Select a known item, or discard its invalid or unused shape.
fn select(raw: &RawValue) -> Option<Content> {
    let kind: String = serde_json::from_str(member(raw, "type")?.get()).ok()?;
    match kind.as_str() {
        "text" => object_record::<Text>(raw).map(|item| Content::Text(item.text)),
        "thinking" => {
            let item = object_record::<Thinking>(raw)?;
            let parts: Option<Vec<String>> =
                item.parts.iter().map(|part| part_text(part)).collect();
            let text = parts?.concat();
            (!text.is_empty()).then_some(Content::Thinking(text))
        }
        _ => None,
    }
}
/// A known text contribution.
#[derive(Deserialize)]
struct Text {
    /// Required text string.
    text: String,
}
/// A whole thinking contribution whose parts must all be selectable.
#[derive(Deserialize)]
struct Thinking {
    /// Parts kept raw until union selection.
    #[serde(rename = "thinking")]
    parts: Vec<Box<RawValue>>,
    /// Optional nullable signature, validated but not emitted.
    #[serde(default, rename = "signature", deserialize_with = "nullable")]
    _signature: Option<Nullable<String>>,
    /// Optional nonnullable closed flag.
    #[serde(default, rename = "closed", deserialize_with = "super::wire::optional")]
    _closed: Option<bool>,
}
/// Tool-reference shape and its presence-bearing fields.
#[derive(Deserialize)]
struct ToolReference {
    /// Required tool name.
    #[serde(rename = "tool")]
    _tool: String,
    /// Required title.
    #[serde(rename = "title")]
    _title: String,
    /// Optional URL.
    #[serde(default, deserialize_with = "nullable")]
    url: Option<Nullable<String>>,
    /// Optional icon.
    #[serde(default, deserialize_with = "nullable")]
    favicon: Option<Nullable<String>>,
    /// Optional description.
    #[serde(default, deserialize_with = "nullable")]
    description: Option<Nullable<String>>,
}
/// Reference shape whose retained IDs contribute union weight.
#[derive(Deserialize)]
struct Reference {
    /// String or integer reference identities.
    reference_ids: Vec<ReferenceId>,
}
/// Admitted reference identity; its value is not emitted.
struct ReferenceId;
impl<'de> Deserialize<'de> for ReferenceId {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(decoder)?;
        if serde_json::from_str::<String>(raw.get()).is_err() {
            serde_json::from_str::<SafeInteger>(raw.get()).map_err(D::Error::custom)?;
        }
        Ok(Self)
    }
}
/// Whether an absent/defaulted or explicit discriminator admits this candidate.
fn tag(raw: &RawValue, expected: &str) -> bool {
    member(raw, "type").is_none_or(|value| {
        serde_json::from_str::<String>(value.get()).is_ok_and(|tag| tag == expected)
    })
}
/// Choose the richest successful part, retaining source tie precedence.
fn part_text(raw: &RawValue) -> Option<String> {
    let tool_weight = tag(raw, "tool_reference")
        .then(|| object_record::<ToolReference>(raw))
        .flatten()
        .map(|tool| {
            3 + usize::from(tool.url.is_some())
                + usize::from(tool.favicon.is_some())
                + usize::from(tool.description.is_some())
        });
    let text = tag(raw, "text")
        .then(|| object_record::<Text>(raw))
        .flatten();
    let reference_weight = tag(raw, "reference")
        .then(|| object_record::<Reference>(raw))
        .flatten()
        .map(|reference| 1 + reference.reference_ids.len());
    let best_nontext = tool_weight.into_iter().chain(reference_weight).max();
    if let Some(text) = text
        && tool_weight.is_none_or(|weight| weight < 2)
        && reference_weight.is_none_or(|weight| weight <= 2)
    {
        return Some(text.text);
    }
    best_nontext.map(|_| String::new())
}
