//! Scoped npm acquisition and installed-version selection.
use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations};
use maestro_path::join;
use maestro_resources::SourceScope;
use std::{io, io::Write, path::Path};

/// The manifest written into a new project or temporary npm root, without a final newline.
const PACKAGE_JSON: &str = "{\n  \"name\": \"maestro-extensions\",\n  \"private\": true\n}";
/// The ignore file written into a new managed root.
pub(super) const GITIGNORE: &str = "*\n!.gitignore\n";

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Installs globally or into a prepared project or temporary root.
    pub(super) async fn install_npm(&self, spec: &str, scope: SourceScope) -> io::Result<()> {
        if scope == SourceScope::User {
            return self.run_npm(&["install", "-g", spec], None).await;
        }
        let root = self.npm_root(scope);
        ensure_file(&root, ".gitignore", GITIGNORE)?;
        ensure_file(&root, "package.json", PACKAGE_JSON)?;
        self.run_npm(&["install", spec, "--prefix", &root], None)
            .await
    }
    /// Uninstalls a package by name; a project root that does not exist has nothing to remove.
    pub(super) async fn uninstall_npm(
        &self,
        name: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        if scope == InstalledSourceScope::User {
            return self.run_npm(&["uninstall", "-g", name], None).await;
        }
        let root = join(&[&self.base(scope), "npm"]);
        if !Path::new(&root).exists() {
            return Ok(());
        }
        self.run_npm(&["uninstall", name, "--prefix", &root], None)
            .await
    }
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Compares selected installed and latest strings, suppressing probe failures.
    pub(super) async fn npm_update_available(&self, name: &str, path: &str) -> bool {
        if self.offline() {
            return false;
        }
        let installed = installed_npm_version(path);
        let Some(installed) = installed else {
            return false;
        };
        self.latest_npm_version(name)
            .await
            .is_ok_and(|latest| latest != installed)
    }
    /// Captures the live command's latest version without coercing decoded values.
    pub(super) async fn latest_npm_version(&self, name: &str) -> io::Result<String> {
        let (command, mut args) = self.npm_command()?;
        args.extend(["view", name, "version", "--json"].map(str::to_owned));
        let stdout = self
            .operations
            .run_command_capture(
                &command,
                &args,
                super::CommandCaptureOptions {
                    cwd: Some(&self.options.cwd),
                    timeout: Some(std::time::Duration::from_millis(10000)),
                    env: &[],
                },
            )
            .await?;
        let raw = crate::trim(&stdout);
        if raw.is_empty() {
            return Err(io::Error::other("Empty response from npm view"));
        }
        serde_json::from_str(raw).map_err(io::Error::other)
    }
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Unknown versions and failed latest probes require an update.
    pub(super) async fn should_update_npm(
        &self,
        row: &super::update::ConfiguredUpdateSource,
    ) -> io::Result<bool> {
        let installed = self
            .installed_path(&row.source, row.scope)?
            .and_then(|path| installed_npm_version(&path));
        let Some(installed) = installed else {
            return Ok(true);
        };
        let super::sources::Source::Npm { name, .. } = super::sources::parse(&row.source) else {
            return Ok(false);
        };
        Ok(self
            .latest_npm_version(name)
            .await
            .map_or(true, |latest| latest != installed))
    }
    /// Reports one grouped update and installs ordered latest specs in its scope.
    pub(super) async fn update_npm_batch(
        &self,
        rows: &[super::update::ConfiguredUpdateSource],
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let label: std::borrow::Cow<str> = if let [row] = rows {
            (&row.source).into()
        } else {
            format!(
                "{} npm packages",
                if scope == InstalledSourceScope::User {
                    "user"
                } else {
                    "project"
                }
            )
            .into()
        };
        let specs: Vec<_> = rows
            .iter()
            .filter_map(|row| match super::sources::parse(&row.source) {
                super::sources::Source::Npm { name, .. } => Some(format!("{name}@latest")),
                _ => None,
            })
            .collect();
        self.with_progress(
            super::ProgressAction::Update,
            &label,
            format!("Updating {label}..."),
            self.install_npm_batch(&specs, scope),
        )
        .await
    }
    /// Shares project preparation and live command selection with acquisition.
    async fn install_npm_batch(
        &self,
        specs: &[String],
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let mut args = vec!["install"];
        let root = join(&[&self.base(scope), "npm"]);
        if scope == InstalledSourceScope::User {
            args.push("-g");
        }
        args.extend(specs.iter().map(String::as_str));
        if scope == InstalledSourceScope::Project {
            ensure_file(&root, ".gitignore", GITIGNORE)?;
            ensure_file(&root, "package.json", PACKAGE_JSON)?;
            args.extend(["--prefix", &root]);
        }
        self.run_npm(&args, None).await
    }
}

/// Creates parents and a missing bootstrap file, accepting a competing creation collision.
/// Other creation failures propagate without rolling back earlier effects.
pub(super) fn ensure_file(dir: &str, name: &str, text: &str) -> io::Result<()> {
    if !Path::new(dir).exists() {
        std::fs::create_dir_all(dir)?;
    }
    let path = join(&[dir, name]);
    if Path::new(&path).exists() {
        return Ok(());
    }
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    match created {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        result => result,
    }
}

/// Reads a nonempty string version, suppressing file and manifest failures.
pub(super) fn installed_npm_version(path: &str) -> Option<String> {
    let manifest = join(&[path, "package.json"]);
    if !Path::new(&manifest).exists() {
        return None;
    }
    maestro_resources::read_text(Path::new(&manifest))
        .ok()
        .and_then(|text| {
            serde_json::from_str::<
                std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
            >(&text)
            .ok()
        })
        .and_then(|mut fields| serde_json::from_str::<String>(fields.remove("version")?.get()).ok())
        .filter(|version| !version.is_empty())
}
