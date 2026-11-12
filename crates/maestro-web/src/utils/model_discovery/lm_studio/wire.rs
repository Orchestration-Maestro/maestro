//! System authentication and server envelopes.
use super::super::{field, members};
use super::schema::invalid;

/// Admit the authentication union without losing a refusal's message.
pub(super) fn authentication(raw: &str) -> Result<Result<(), String>, serde_json::Error> {
    let object = members(raw)?;
    if field::<bool>(&object, "success")? {
        Ok(Ok(()))
    } else {
        Ok(Err(field(&object, "error")?))
    }
}

/// Validated messages relevant to this one-shot call.
pub(super) enum Envelope {
    /// Well-formed control traffic belonging to another operation.
    Unowned,
    /// Correlated RPC success with an opaque result.
    Result {
        /// Server's integral correlation identifier.
        id: f64,
        /// Opaque value, absent when the server omitted it.
        result: Option<Box<serde_json::value::RawValue>>,
    },
    /// Correlated RPC failure retaining the complete remote object.
    Error {
        /// Server's integral correlation identifier.
        id: f64,
        /// Full remote error, including unknown metadata.
        error: Box<serde_json::value::RawValue>,
    },
}
/// Validate a remote error root without narrowing optional metadata.
fn remote(object: &super::super::Members) -> Result<(), serde_json::Error> {
    members(object.get("error").map_or("null", |value| value.get()))?;
    Ok(())
}
/// Admit a complete envelope before correlation or result selection.
pub(super) fn envelope(raw: &str) -> Result<Envelope, serde_json::Error> {
    use super::schema::integer;
    let mut object = members(raw)?;
    let kind: String = field(&object, "type")?;
    match kind.as_str() {
        "communicationWarning" => {
            field::<String>(&object, "warning")?;
        }
        "keepAliveAck" => {}
        "rpcResult" => {
            return Ok(Envelope::Result {
                id: integer(&object, "callId")?,
                result: object.remove("result"),
            });
        }
        "rpcError" => {
            let id = integer(&object, "callId")?;
            remote(&object)?;
            return Ok(Envelope::Error {
                id,
                error: object
                    .remove("error")
                    .ok_or_else(|| invalid("Missing error"))?,
            });
        }
        "channelSend" | "channelAck" | "channelClose" | "channelError" => {
            integer(&object, "channelId")?;
            if kind == "channelAck" || object.contains_key("ackId") && kind == "channelSend" {
                integer(&object, "ackId")?;
            }
            if kind == "channelError" {
                remote(&object)?;
            }
        }
        "signalUpdate" | "writableSignalUpdate" | "signalError" | "writableSignalError" => {
            integer(&object, "subscribeId")?;
            if kind.ends_with("Update") {
                field::<Vec<Box<serde_json::value::RawValue>>>(&object, "patches")?;
                field::<Vec<String>>(&object, "tags")?;
            } else {
                remote(&object)?;
            }
        }
        _ => return Err(invalid("Unknown server envelope")),
    }
    Ok(Envelope::Unowned)
}
