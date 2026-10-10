//! npm command selection, captured errors and keyed root reuse.
use super::{DefaultPackageManager, PackageOperations, paths::bun_root};
use maestro_settings::SettingsListEntry;
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Reads a command vector with native errors for consumed non-string members.
    fn npm_command(&self) -> io::Result<(String, Vec<String>)> {
        let settings = self
            .options
            .settings_manager
            .try_borrow()
            .map_err(io::Error::other)?;
        let mut entries = settings.get_npm_command().unwrap_or_default().into_iter();
        drop(settings);
        let executable = entries
            .next()
            .map(command_member)
            .transpose()?
            .unwrap_or_else(|| "npm".into());
        if executable.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid npmCommand: first array entry must be a non-empty command",
            ));
        }
        let args = entries.map(command_member).collect::<io::Result<_>>()?;
        Ok((executable, args))
    }
    /// Queries the selected command, reusing only the matching nonempty root.
    pub(super) fn global_npm_root(&mut self) -> io::Result<&str> {
        let (executable, mut args) = self.npm_command()?;
        let key = std::iter::once(executable.as_str())
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("\0");
        if self.root_cache.0 != key || self.root_cache.1.is_empty() {
            let bun = executable == "bun";
            args.extend(if bun {
                vec!["pm".into(), "bin".into(), "-g".into()]
            } else {
                vec!["root".into(), "-g".into()]
            });
            let output = capture(&self.operations, &executable, &args)?;
            let root = if bun {
                bun_root(crate::trim(&output))
            } else {
                output
            };
            self.root_cache = (key, root);
        }
        Ok(&self.root_cache.1)
    }
}
/// Captures a completed command, retaining stream choice and authored wrapping.
fn capture(
    operations: &impl PackageOperations,
    command: &str,
    args: &[String],
) -> io::Result<String> {
    let result = operations.run_command_sync(command, args);
    let error = match result {
        Err(error) => error.to_string(),
        Ok(output) if output.status == Some(0) => {
            let selected = if output.stdout.is_empty() {
                &output.stderr
            } else {
                &output.stdout
            };
            return Ok(crate::trim(selected).to_owned());
        }
        Ok(output) => {
            if output.stderr.is_empty() {
                output.stdout
            } else {
                output.stderr
            }
        }
    };
    Err(io::Error::other(format!(
        "Failed to run {command} {}: {error}",
        args.join(" ")
    )))
}

/// Consumes a command member without coercing or discarding its value.
fn command_member(entry: SettingsListEntry) -> io::Result<String> {
    match entry {
        SettingsListEntry::String(text) => Ok(text),
        SettingsListEntry::Unknown(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "npmCommand member is not a string",
        )),
    }
}
