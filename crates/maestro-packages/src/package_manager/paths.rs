//! Scope bases and demand-driven lexical path resolution.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    sources::{self, Source},
};
use maestro_path::{Cwd, Tilde, dirname, expand_tilde_joined, join, parse_tilde, relative};
use maestro_resources::{
    SourceScope,
    paths::{CONFIG_DIR_NAME, home_directory, resolve},
};
use sha2::{Digest, Sha256};
use std::{io, path::Path};
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Finds already-existing contents without acquiring the source.
    /// # Errors
    /// Returns required path, settings or command failures.
    pub(super) fn installed_path(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<Option<String>> {
        let parsed = sources::parse(source);
        let path = if let Source::Local(path) = parsed {
            resolve_local(path, &self.base(scope), home_directory)?
        } else {
            self.remote_path(&parsed, scope.resource_scope())?
        };
        Ok(Path::new(&path).exists().then_some(path))
    }
    /// Selects a remote path before admission or acquisition.
    pub(super) fn remote_path(
        &self,
        source: &Source<'_>,
        scope: SourceScope,
    ) -> io::Result<String> {
        match source {
            Source::Npm { name, .. } => {
                let root = if scope == SourceScope::User {
                    self.global_npm_root()?
                } else {
                    join(&[&self.npm_root(scope), "node_modules"])
                };
                Ok(join(&[&root, name]))
            }
            Source::Git(git) => Ok(self.git_path(git, scope)),
            Source::Local(path) => resolve_local(path, &self.options.cwd, home_directory),
        }
    }
    /// Selects the npm root path for project or temporary installation.
    pub(super) fn npm_root(&self, scope: SourceScope) -> String {
        if scope == SourceScope::Temporary {
            temporary_dir("npm", "")
        } else {
            join(&[&self.options.cwd, CONFIG_DIR_NAME, "npm"])
        }
    }
    /// Selects the Git checkout, excluding refs from cache identity.
    pub(super) fn git_path(&self, git: &crate::GitSource, scope: SourceScope) -> String {
        match scope {
            SourceScope::Temporary => temporary_dir(&format!("git-{}", git.host), &git.path),
            SourceScope::User => join(&[&self.options.agent_dir, "git", &git.host, &git.path]),
            SourceScope::Project => join(&[
                &self.options.cwd,
                CONFIG_DIR_NAME,
                "git",
                &git.host,
                &git.path,
            ]),
        }
    }
    /// The authored settings base for one scope.
    pub(super) fn base(&self, scope: InstalledSourceScope) -> std::borrow::Cow<'_, str> {
        match scope {
            InstalledSourceScope::User => self.options.agent_dir.as_str().into(),
            InstalledSourceScope::Project => join(&[&self.options.cwd, CONFIG_DIR_NAME]).into(),
        }
    }
    /// Normalizes local input for storage, leaving nonlocal spelling intact.
    pub(super) fn normalize_source(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<String> {
        if let Source::Local(path) = sources::parse(source) {
            let resolved = resolve_local(path, &self.options.cwd, home_directory)?;
            let base = resolve(&[&self.base(scope)])?;
            let resolved = resolve(&[&resolved])?;
            let result = relative(
                &base,
                &resolved,
                &Cwd {
                    current: "",
                    drive_directories: &[],
                },
            );
            return Ok(if result.is_empty() {
                ".".into()
            } else {
                result
            });
        }
        Ok(source.to_owned())
    }
}
/// Derives the dependency root from the global bin directory.
pub(super) fn bun_root(bin: &str) -> String {
    join(&[&dirname(bin), "install", "global", "node_modules"])
}

/// Hashes authored UTF-8 identity before lexical joining.
fn temporary_dir(prefix: &str, suffix: &str) -> String {
    let digest = Sha256::digest(format!("{prefix}-{suffix}").as_bytes());
    let hash = format!(
        "{:08x}",
        u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]])
    );
    join(&[
        &std::env::temp_dir().to_string_lossy(),
        "maestro-extensions",
        prefix,
        &hash,
        suffix,
    ])
}

/// Resolves local input with this caller's trim and tilde rules, reading the home directory
/// through `home`.
pub(super) fn resolve_local(
    source: &str,
    base: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    let source = crate::trim(source);
    if parse_tilde(source) == Tilde::Other {
        return resolve(&[base, source]);
    }
    expand_tilde_joined(source, home)
}

#[cfg(test)]
mod tests {
    use super::resolve_local;

    /// Each local source form returns the reference's result for the six supplied homes, spelled
    /// in the host's path flavor.
    #[test]
    fn local_sources_follow_the_reference_table() {
        let (base, rows) = if cfg!(windows) {
            (
                r"C:\b",
                [
                    ("~", r"C:\h", r"C:\h"),
                    ("~/", r"C:\h", r"C:\h"),
                    ("~/x", r"C:\h", r"C:\h\x"),
                    ("~x", r"C:\h", r"C:\b\~x"),
                    ("~", r"C:\h\.", r"C:\h\."),
                    ("~", "", ""),
                    ("~/x", "", "x"),
                ],
            )
        } else {
            (
                "/b",
                [
                    ("~", "/h", "/h"),
                    ("~/", "/h", "/h"),
                    ("~/x", "/h", "/h/x"),
                    ("~x", "/h", "/b/~x"),
                    ("~", "/h/.", "/h/."),
                    ("~", "", ""),
                    ("~/x", "", "x"),
                ],
            )
        };
        for (written, home, expected) in rows {
            assert_eq!(
                resolve_local(written, base, || Ok(home.to_owned())).unwrap(),
                expected,
                "{written:?} with home {home:?}"
            );
        }
    }

    /// A source that does not start with `~` resolves against the supplied base and reads no
    /// home directory.
    #[test]
    fn other_sources_resolve_from_the_base_without_home() {
        let failing = || Err(std::io::Error::other("unknown home"));
        let (base, below, above) = if cfg!(windows) {
            (r"C:\b", r"C:\b\x", r"C:\y")
        } else {
            ("/b", "/b/x", "/y")
        };
        assert_eq!(resolve_local("x", base, failing).unwrap(), below);
        assert_eq!(resolve_local(" ../y ", base, failing).unwrap(), above);
    }
}
