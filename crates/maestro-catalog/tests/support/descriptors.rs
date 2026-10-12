//! Strict fixture model records.
use maestro_models::Model;
use serde::Deserialize;
/// Reject unread descriptor and cost fields before reusing the shared model decoder.
pub fn descriptors<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Vec<Model>, D::Error> {
    let values = Vec::<serde_json::Value>::deserialize(decoder)?;
    values
        .into_iter()
        .map(|value| {
            let model = value
                .as_object()
                .ok_or_else(|| serde::de::Error::custom("expected descriptor object"))?;
            if model.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "id" | "name"
                        | "api"
                        | "provider"
                        | "baseUrl"
                        | "reasoning"
                        | "thinkingLevelMap"
                        | "input"
                        | "cost"
                        | "contextWindow"
                        | "maxTokens"
                        | "headers"
                        | "compat"
                )
            }) {
                return Err(serde::de::Error::custom("unread descriptor field"));
            }
            if model
                .get("cost")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|cost| {
                    cost.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "input" | "output" | "cacheRead" | "cacheWrite"
                        )
                    })
                })
            {
                return Err(serde::de::Error::custom("unread cost field"));
            }
            Model::deserialize(value).map_err(serde::de::Error::custom)
        })
        .collect()
}
