//! npm command selection, command execution and keyed root reuse.
use super::{DefaultPackageManager, PackageOperations, paths::bun_root};
use maestro_settings::SettingsListEntry;
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Reads a command vector with native errors for consumed non-string members.
    pub(super) fn npm_command(&self) -> io::Result<(String, Vec<String>)> {
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
    pub(super) fn global_npm_root(&self) -> io::Result<String> {
        let (executable, mut args) = self.npm_command()?;
        let key = std::iter::once(executable.as_str())
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("\0");
        let cached = {
            let cache = self.root_cache.borrow();
            (cache.0 == key && !cache.1.is_empty()).then(|| cache.1.clone())
        };
        if let Some(root) = cached {
            return Ok(root);
        }
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
        *self.root_cache.borrow_mut() = (key, root.clone());
        Ok(root)
    }
    /// Runs the configured npm command with the given trailing arguments.
    pub(super) async fn run_npm(&self, args: &[&str], cwd: Option<&str>) -> io::Result<()> {
        let (executable, mut all) = self.npm_command()?;
        all.extend(args.iter().map(|arg| (*arg).to_owned()));
        self.run_command(&executable, &all, cwd).await
    }
    /// Whether settings configure a nonempty command vector.
    pub(super) fn npm_command_configured(&self) -> io::Result<bool> {
        let settings = self
            .options
            .settings_manager
            .try_borrow()
            .map_err(io::Error::other)?;
        Ok(settings
            .get_npm_command()
            .is_some_and(|command| !command.is_empty()))
    }
    /// Waits for a child to exit, failing on a nonzero or missing exit code.
    pub(super) async fn run_command(
        &self,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> io::Result<()> {
        match self.operations.run_command(command, args, cwd).await? {
            Some(0) => Ok(()),
            code => Err(io::Error::other(format!(
                "{command} {} failed with code {}",
                args.join(" "),
                code.map_or_else(|| "null".to_owned(), |code| code.to_string())
            ))),
        }
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
