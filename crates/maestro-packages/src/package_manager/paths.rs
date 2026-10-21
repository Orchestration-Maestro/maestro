//! Scope bases and demand-driven lexical path resolution.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    sources::{self, Source},
};
use maestro_path::{Cwd, dirname, join, relative, resolve_with};
use maestro_resources::SourceScope;
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
            resolve_local(path, &self.base(scope))?
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
            Source::Local(path) => resolve_local(path, &self.options.cwd),
        }
    }
    /// Selects the npm root path for project or temporary installation.
    pub(super) fn npm_root(&self, scope: SourceScope) -> String {
        if scope == SourceScope::Temporary {
            temporary_dir("npm", "")
        } else {
            join(&[&self.options.cwd, ".maestro", "npm"])
        }
    }
    /// Selects the Git checkout, excluding refs from cache identity.
    pub(super) fn git_path(&self, git: &crate::GitSource, scope: SourceScope) -> String {
        match scope {
            SourceScope::Temporary => temporary_dir(&format!("git-{}", git.host), &git.path),
            SourceScope::User => join(&[&self.options.agent_dir, "git", &git.host, &git.path]),
            SourceScope::Project => {
                join(&[&self.options.cwd, ".maestro", "git", &git.host, &git.path])
            }
        }
    }
    /// The authored settings base for one scope.
    pub(super) fn base(&self, scope: InstalledSourceScope) -> std::borrow::Cow<'_, str> {
        match scope {
            InstalledSourceScope::User => self.options.agent_dir.as_str().into(),
            InstalledSourceScope::Project => join(&[&self.options.cwd, ".maestro"]).into(),
        }
    }
    /// Normalizes local input for storage, leaving nonlocal spelling intact.
    pub(super) fn normalize_source(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<String> {
        if let Source::Local(path) = sources::parse(source) {
            let resolved = resolve_local(path, &self.options.cwd)?;
            let base = resolve_operands(&[&self.base(scope)])?;
            let resolved = resolve_operands(&[&resolved])?;
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

/// Resolves each operand group against one ambient context: the drive directories and
/// the working directory are read at most once, and only when an operand needs them.
pub(super) fn resolve_together(groups: &[&[&str]]) -> io::Result<Vec<String>> {
    resolve_with(
        groups,
        maestro_resources::drive_directories,
        maestro_resources::current_directory,
    )
}

/// Resolves local input with this caller's trim and tilde rules.
pub(super) fn resolve_local(source: &str, base: &str) -> io::Result<String> {
    let source = crate::trim(source);
    if let Some(after) = source.strip_prefix('~') {
        let home = super::operations::home_dir()?;
        return Ok(if after.is_empty() {
            home
        } else {
            join(&[&home, after.strip_prefix('/').unwrap_or(after)])
        });
    }
    resolve_operands(&[base, source])
}

/// Consults ambient directories only after authored operands fail to resolve.
pub(super) fn resolve_operands(operands: &[&str]) -> io::Result<String> {
    Ok(resolve_together(&[operands])?.concat())
}
