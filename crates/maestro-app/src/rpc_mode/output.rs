//! The protocol's output: the records a server writes and the framed lines that carry them.
use super::RpcOutput;
use crate::agent_session::AgentSessionEvent;
use crate::output_guard::write_raw_stdout;
use crate::rpc_types::{RpcExtensionUIRequest, RpcResponse, workspace_schema};
use maestro_extensions::ExtensionError;
use maestro_json::jsonl::serialize_json_line;
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::{io, rc::Rc};

/// One record a server writes.
///
/// Only these records are written; a JSON value is not one:
///
/// ```compile_fail
/// use maestro_app::rpc_mode::RpcLine;
/// let line = RpcLine::new(&serde_json::json!({"type": "response"}));
/// ```
#[derive(Clone, Copy)]
pub enum RpcRecord<'a> {
    /// A command's response.
    Response(&'a RpcResponse),
    /// A session event.
    Event(&'a AgentSessionEvent),
    /// A request of the extension interface.
    ExtensionUiRequest(&'a RpcExtensionUIRequest),
    /// An extension's failure, written without its stack.
    ExtensionError(&'a ExtensionError),
    /// A workspace event the connection opted into.
    WorkspaceEvent(&'a workspace_schema::EventData),
}

impl Serialize for RpcRecord<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Response(response) => response.serialize(serializer),
            Self::Event(event) => event.serialize(serializer),
            Self::ExtensionUiRequest(request) => request.serialize(serializer),
            Self::WorkspaceEvent(event) => event.serialize(serializer),
            Self::ExtensionError(error) => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "extension_error")?;
                map.serialize_entry("extensionPath", &error.extension_path)?;
                map.serialize_entry("event", &error.event)?;
                map.serialize_entry("error", &error.error)?;
                map.end()
            }
        }
    }
}

/// One record as a compact JSON line, LF included.
///
/// A line holds a framed record and nothing else:
///
/// ```compile_fail
/// let line = maestro_app::rpc_mode::RpcLine(String::from("{}\n"));
/// ```
pub struct RpcLine(String);

impl RpcLine {
    /// Frames `record` through the shared JSONL framing.
    ///
    /// ```
    /// use maestro_app::rpc_mode::{RpcLine, RpcRecord};
    /// use maestro_app::RpcResponse;
    ///
    /// let response = RpcResponse::Error {
    ///     id: None,
    ///     command: None,
    ///     error: "failed".into(),
    /// };
    /// let line = RpcLine::new(&RpcRecord::Response(&response)).unwrap();
    /// assert!(line.as_str().ends_with('\n'));
    /// let output: maestro_app::rpc_mode::RpcOutput = std::rc::Rc::new(|_: &RpcLine| Ok(()));
    /// output(&line).unwrap();
    /// ```
    ///
    /// # Errors
    /// Returns the serializer's failure.
    pub fn new(record: &RpcRecord<'_>) -> serde_json::Result<Self> {
        serialize_json_line(&serde_json::to_value(record)?).map(Self)
    }

    /// The line's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Writes `record` as one line through `output`.
pub(super) fn write_record(output: &RpcOutput, record: RpcRecord<'_>) -> io::Result<()> {
    output(&RpcLine::new(&record)?)
}

/// An output writing each line to standard output, which carries protocol lines only once
/// taken over.
#[must_use]
pub fn stdout_output() -> RpcOutput {
    Rc::new(|line: &RpcLine| write_raw_stdout(line.as_str()))
}
