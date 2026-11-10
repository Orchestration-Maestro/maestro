//! Server protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default)]
/// Ping params record.
#[schemars(description = "")]
pub struct PingParams {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Server live handoff params record.
#[schemars(description = "")]
pub struct ServerLiveHandoffParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Import exe.
    #[schemars(description = "")]
    pub import_exe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Expected protocol.
    #[schemars(description = "")]
    pub expected_protocol: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Expected version.
    #[schemars(description = "")]
    pub expected_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Server capabilities record.
#[schemars(description = "")]
pub struct ServerCapabilities {
    /// Live handoff.
    #[schemars(description = "")]
    pub live_handoff: bool,
    #[serde(default)]
    /// Detached server daemon.
    #[schemars(description = "")]
    pub detached_server_daemon: bool,
}
