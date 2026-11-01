//! Correlated workspace responses with preserved machine-readable failures.
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::schema::{ErrorBody, ResponseResult};

/// A correlated command result in the shared RPC envelope.
#[derive(Debug, Clone, PartialEq, schemars::JsonSchema)]
#[schemars(with = "ResponseWire")]
pub struct RpcResponse {
    /// Caller-supplied correlation identifier, when present.
    pub id: Option<String>,
    /// Command type from the request.
    pub command: String,
    /// Typed success data or the original workspace error fields.
    pub result: Result<ResponseResult, ErrorBody>,
}

impl Serialize for RpcResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let result = match &self.result {
            Ok(data) => Ok(Some(data)),
            Err(error) => Err(RpcErrorRef {
                message: &error.message,
                code: Some(&error.code),
            }),
        };
        serialize_rpc_response(serializer, self.id.as_deref(), Some(&self.command), result)
    }
}

/// A borrowed failure of the shared response envelope.
#[derive(Clone, Copy, Debug)]
pub struct RpcErrorRef<'a> {
    /// The human-readable failure.
    pub message: &'a str,
    /// The open machine-readable failure kind, written only when present.
    pub code: Option<&'a str>,
}

/// Write the shared response envelope: the optional `id`, `type: "response"`, the optional
/// `command`, `success`, then `data` when a success carries one, or `error` and an optional
/// `code` for a failure.
///
/// # Errors
/// Returns the serializer's failure.
pub fn serialize_rpc_response<S: Serializer, T: Serialize>(
    serializer: S,
    id: Option<&str>,
    command: Option<&str>,
    result: Result<Option<&T>, RpcErrorRef<'_>>,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(None)?;
    if let Some(id) = id {
        map.serialize_entry("id", id)?;
    }
    map.serialize_entry("type", "response")?;
    if let Some(command) = command {
        map.serialize_entry("command", command)?;
    }
    map.serialize_entry("success", &result.is_ok())?;
    match result {
        Ok(Some(data)) => map.serialize_entry("data", data)?,
        Ok(None) => {}
        Err(error) => {
            map.serialize_entry("error", error.message)?;
            if let Some(code) = error.code {
                map.serialize_entry("code", code)?;
            }
        }
    }
    map.end()
}

/// The single response discriminator.
#[derive(Deserialize, schemars::JsonSchema)]
enum ResponseTag {
    /// A command reply.
    #[serde(rename = "response")]
    Response,
}

/// Boolean-tagged response alternatives used by decoding and schema generation.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
enum ResponseWire {
    /// Successful workspace operation.
    Success {
        /// Optional request correlation.
        id: Option<String>,
        /// Response discriminator.
        #[serde(rename = "type")]
        _kind: ResponseTag,
        /// Original request command.
        command: String,
        /// Successful operation marker.
        #[serde(rename = "success", deserialize_with = "status::<true, _>")]
        #[schemars(extend("const" = true))]
        _success: bool,
        /// Typed workspace result.
        data: Box<ResponseResult>,
    },
    /// Failed workspace operation.
    Failure {
        /// Optional request correlation.
        id: Option<String>,
        /// Response discriminator.
        #[serde(rename = "type")]
        _kind: ResponseTag,
        /// Original request command.
        command: String,
        /// Failed operation marker.
        #[serde(rename = "success", deserialize_with = "status::<false, _>")]
        #[schemars(extend("const" = false))]
        _success: bool,
        /// Original human-readable failure.
        error: String,
        /// Open machine-readable failure kind.
        code: String,
    },
}

/// Decode the expected boolean discriminator rather than accepting another branch.
fn status<'de, const EXPECTED: bool, D: Deserializer<'de>>(decoder: D) -> Result<bool, D::Error> {
    let value = bool::deserialize(decoder)?;
    if value != EXPECTED {
        return Err(de::Error::custom("unexpected response success marker"));
    }
    Ok(value)
}

impl<'de> Deserialize<'de> for RpcResponse {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        Ok(match ResponseWire::deserialize(decoder)? {
            ResponseWire::Success {
                id, command, data, ..
            } => Self {
                id,
                command,
                result: Ok(*data),
            },
            ResponseWire::Failure {
                id,
                command,
                error,
                code,
                ..
            } => Self {
                id,
                command,
                result: Err(ErrorBody {
                    code,
                    message: error,
                }),
            },
        })
    }
}
