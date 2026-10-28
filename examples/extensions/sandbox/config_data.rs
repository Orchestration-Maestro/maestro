//! Sandbox permission records and their shallow configuration composition.
use super::network::NetworkConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Configuration with JSON truth semantics for enabled.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SandboxConfig {
    /// Whether startup enables the sandbox.
    pub(crate) enabled: Value,
    /// Domain admission rules.
    pub(crate) network: NetworkConfig,
    /// Filesystem permissions.
    pub(crate) filesystem: FilesystemConfig,
    /// Diagnostic exclusions, never filesystem authorization grants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ignore_violations: Option<std::collections::BTreeMap<String, Vec<String>>>,
    /// Explicitly retains the host process namespace for nested environments.
    #[serde(default)]
    pub(crate) enable_weaker_nested_sandbox: bool,
}

/// Read exclusions and write admission paths.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilesystemConfig {
    /// Paths whose contents must be hidden.
    pub(crate) deny_read: Vec<String>,
    /// Paths writable within the otherwise read-only filesystem.
    pub(crate) allow_write: Vec<String>,
    /// Write exclusions within admitted paths.
    pub(crate) deny_write: Vec<String>,
    /// Allows repository configuration while hooks remain protected.
    #[serde(default)]
    pub(crate) allow_git_config: bool,
}

/// Default permissions before user layers.
pub(crate) fn defaults() -> Value {
    json!({
        "enabled": true,
        "network": NetworkConfig::default(),
        "filesystem": {
            "denyRead": ["~/.ssh", "~/.aws", "~/.gnupg"],
            "allowWrite": [".", "/tmp"],
            "denyWrite": [".env", ".env.*", "*.pem", "*.key"]
        }
    })
}

/// Merges the two shallow permission objects and the explicitly supported top-level keys.
pub(crate) fn merge(base: &mut Value, layer: Value) {
    let Value::Object(layer) = layer else {
        return;
    };
    for (key, value) in layer {
        match key.as_str() {
            "network" | "filesystem" => {
                if let (Some(target), Value::Object(overrides)) =
                    (base.get_mut(&key).and_then(Value::as_object_mut), value)
                {
                    target.extend(overrides);
                }
            }
            "enabled" | "ignoreViolations" | "enableWeakerNestedSandbox" => base[&key] = value,
            _ => {}
        }
    }
}

/// Formats the reloaded configuration without changing the initialized runtime's policy.
pub(crate) fn display(config: &SandboxConfig) -> String {
    let list = |values: &[String]| {
        if values.is_empty() {
            "(none)".into()
        } else {
            values.join(", ")
        }
    };
    format!(
        "Sandbox configuration:\n\nNetwork:\n  Allowed: {}\n  Denied: {}\n\nFilesystem:\n  Deny read: {}\n  Allow write: {}\n  Deny write: {}",
        list(&config.network.allowed_domains),
        list(&config.network.denied_domains),
        list(&config.filesystem.deny_read),
        list(&config.filesystem.allow_write),
        list(&config.filesystem.deny_write)
    )
}

/// Files and directories whose configuration can introduce executable hooks.
pub(crate) const PROTECTED: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".ripgreprc",
    ".mcp.json",
    ".vscode",
    ".idea",
    ".maestro/commands",
    ".maestro/roles",
    ".git/hooks",
    ".git/config",
];
