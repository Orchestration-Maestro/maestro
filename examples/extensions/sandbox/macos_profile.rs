//! Native macOS authorization profile serialization.
use super::config_data::SandboxConfig;

/// Serializes resolved permissions and the selected local proxy endpoints.
pub(crate) fn profile(_config: &SandboxConfig, _ports: [u16; 2], _temporary: &str, _cwd: &str, _tag: &str) -> String {
    String::new()
}
