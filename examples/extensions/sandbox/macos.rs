//! Native macOS command preparation over the shared bash execution backend.
use super::{linux::argv, macos_profile, runtime::PlatformContext};
use maestro_extensions_wasm::tools::path_utils::guest_path;

/// Writes an owned native profile and wraps operands without executing the payload.
pub(crate) fn wrap(
    context: &PlatformContext,
    command: &str,
    temporary: &str,
    authored_temporary: &str,
) -> Result<String, String> {
    let mut config = super::config_data::SandboxConfig {
        enabled: serde_json::Value::Bool(true),
        network: context.network.clone(),
        filesystem: context.filesystem.clone(),
        ignore_violations: None,
        enable_weaker_nested_sandbox: context.weaker,
    };
    for paths in [
        &mut config.filesystem.deny_read,
        &mut config.filesystem.allow_write,
        &mut config.filesystem.deny_write,
        &mut config.network.allow_unix_sockets,
    ] {
        for path in paths {
            if !path.starts_with("**/") {
                *path = context.normalize(path)?;
            }
        }
    }
    config
        .filesystem
        .deny_write
        .push(context.control_directory.clone());
    let id = context.profile_id.get();
    context.profile_id.set(
        id.checked_add(1)
            .ok_or("Sandbox profile identities exhausted")?,
    );
    let path = format!("{}/profile-{id}.sb", context.control_directory);
    let profile = macos_profile::profile(
        &config,
        context.ports,
        temporary,
        &context.cwd,
        "maestro-sandbox",
    );
    std::fs::write(guest_path(&context.api, &path)?, profile).map_err(|error| error.to_string())?;
    let mut arguments = vec!["env".into()];
    arguments.extend(
        context
            .environment(authored_temporary, context.ports)
            .into_iter()
            .map(|(key, value)| format!("{key}={value}")),
    );
    arguments.extend([
        "sandbox-exec".into(),
        "-f".into(),
        path,
        context.shell.clone(),
        "-c".into(),
        command.into(),
    ]);
    Ok(argv(&arguments))
}
