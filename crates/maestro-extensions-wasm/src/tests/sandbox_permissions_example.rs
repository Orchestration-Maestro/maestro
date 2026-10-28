//! Complete configuration forwarding and the native macOS authorization profile.
use super::sandbox_network_example::network;
#[path = "../../../../examples/extensions/sandbox/config_data.rs"]
mod config_data;
#[path = "../../../../examples/extensions/sandbox/macos_profile.rs"]
mod macos_profile;
use serde_json::{Value, json};

/// Resolves explicit fixture paths before the platform profile consumes them.
fn configured(network: Value, filesystem: Value) -> config_data::SandboxConfig {
    let mut value = config_data::defaults();
    config_data::merge(
        &mut value,
        Value::Object([("network".into(), network), ("filesystem".into(), filesystem)].into_iter().collect()),
    );
    serde_json::from_value(value).unwrap()
}

/// Uses fixed native observations rather than any network or operating-system probe.
fn render(config: &config_data::SandboxConfig) -> String {
    macos_profile::profile(config, [9001, 9002], "/tmp/maestro", "/project", "case")
}

#[test]
fn sandbox_permissions_keep_every_forwarded_configuration_field() {
    let mut value = config_data::defaults();
    config_data::merge(&mut value, json!({
        "network":{"allowedDomains":["global"],"allowUnixSockets":["/socket"],"allowAllUnixSockets":true,"allowLocalBinding":true,"httpProxyPort":9001,"socksProxyPort":9002},
        "filesystem":{"allowWrite":["/global"],"allowGitConfig":true},
        "ignoreViolations":{"*":["/ignored"]},"enableWeakerNestedSandbox":true
    }));
    config_data::merge(&mut value, json!({"network":{"allowedDomains":["project"]},"filesystem":{"allowWrite":["/project"]}}));
    let config: config_data::SandboxConfig = serde_json::from_value(value.clone()).unwrap();
    let actual = serde_json::to_value(&config).unwrap();
    for field in ["network", "filesystem", "ignoreViolations", "enableWeakerNestedSandbox"] {
        assert_eq!(actual[field], value[field], "{field}");
    }
    assert!(config_data::display(&config).contains("project"));
    assert!(config.network.allows("project"));
    assert!(!config.network.allows("global"));
}

#[test]
fn sandbox_macos_profile_limits_network_and_preserves_filesystem_exclusions() {
    let config = configured(json!({}), json!({"denyRead":["/secrets","/project/**/*.pem"],"allowWrite":["/project"],"denyWrite":["/project/.env"]}));
    let profile = render(&config);
    for rule in ["(deny default", "(allow file-read*)", "(deny file-read*", "(allow file-write*", "(deny file-write*", "file-write-unlink", "/secrets", "/project/.env", "localhost:9001", "localhost:9002", "/tmp/maestro", ".git/hooks", ".git/config"] { assert!(profile.contains(rule), "{rule}: {profile}"); }
    assert!(!profile.contains("(allow network*)"));
    assert!(!profile.contains("localhost:*"));
}

#[test]
fn sandbox_macos_unix_socket_grants_do_not_open_internet_sockets() {
    for (network, expected) in [(json!({"allowUnixSockets":["/agent/socket"]}), "(subpath \"/agent/socket\")"), (json!({"allowAllUnixSockets":true}), "(allow network* (subpath \"/\"))")] {
        let profile = render(&configured(network, json!({})));
        assert!(profile.contains(expected), "{profile}");
        assert!(!profile.contains("(allow network*)"));
    }
}

#[test]
fn sandbox_macos_local_binding_and_git_config_grants_are_explicit() {
    let config = configured(json!({"allowLocalBinding":true}), json!({"allowGitConfig":true}));
    let profile = render(&config);
    assert!(profile.contains("(allow network-bind (local ip \"localhost:*\"))"));
    assert!(!profile.contains(".git/config"));
    assert!(profile.contains(".git/hooks"));
}

#[test]
fn sandbox_violation_ignores_and_linux_nesting_do_not_grant_macos_permissions() {
    let mut value = config_data::defaults();
    let ordinary = render(&serde_json::from_value(value.clone()).unwrap());
    assert!(ordinary.contains("(deny default"));
    config_data::merge(&mut value, json!({"ignoreViolations":{"*":["/secret"]},"enableWeakerNestedSandbox":true}));
    assert_eq!(render(&serde_json::from_value(value).unwrap()), ordinary);
}
