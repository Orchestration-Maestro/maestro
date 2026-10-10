//! Scope bases and demand-driven lexical path resolution.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    sources::{self, Source},
};
use maestro_path::{Cwd, dirname, join, relative, resolve, try_resolve};
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Finds already-existing contents without acquiring the source.
    /// # Errors
    /// Returns required path, settings or command failures.
    pub(super) fn installed_path(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<Option<String>> {
        let path = match sources::parse(source) {
            Source::Npm { name, .. } => match scope {
                InstalledSourceScope::User => join(&[&self.global_npm_root()?, name]),
                InstalledSourceScope::Project => {
                    join(&[&self.base(scope), "npm", "node_modules", name])
                }
            },
            Source::Git(git) => join(&[&self.base(scope), "git", &git.host, &git.path]),
            Source::Local(path) => self.resolve_local(path, &self.base(scope))?,
        };
        Ok(self.operations.exists(&path).then_some(path))
    }
    /// The authored settings base for one scope.
    pub(super) fn base(&self, scope: InstalledSourceScope) -> std::borrow::Cow<'_, str> {
        match scope {
            InstalledSourceScope::User => self.options.agent_dir.as_str().into(),
            InstalledSourceScope::Project => join(&[&self.options.cwd, ".maestro"]).into(),
        }
    }
    /// Resolves local input with this caller's trim and tilde rules.
    pub(super) fn resolve_local(&self, source: &str, base: &str) -> io::Result<String> {
        let source = crate::trim(source);
        if let Some(after) = source.strip_prefix('~') {
            let home = self.operations.home_dir()?;
            return Ok(if after.is_empty() {
                home
            } else {
                join(&[&home, after.strip_prefix('/').unwrap_or(after)])
            });
        }
        self.resolve_operands(&[base, source])
    }
    /// Consults ambient directories only after authored operands fail to resolve.
    pub(super) fn resolve_operands(&self, operands: &[&str]) -> io::Result<String> {
        Ok(self.resolve_together(&[operands])?.concat())
    }
    /// Resolves each operand group against one ambient context: the working directory is captured at most once, and
    /// only when required; per-drive directory lookups are separate.
    pub(super) fn resolve_together(&self, groups: &[&[&str]]) -> io::Result<Vec<String>> {
        let plain: Result<Vec<_>, _> = groups.iter().map(|g| try_resolve(g, &[])).collect();
        if let Ok(paths) = plain {
            return Ok(paths);
        }
        #[cfg(windows)]
        let directories: Vec<(char, String)> = {
            let mut found: Vec<(char, String)> = Vec::new();
            for group in groups {
                let letter = group.iter().rev().find_map(|path| {
                    let bytes = path.as_bytes();
                    bytes
                        .first()
                        .filter(|l| l.is_ascii_alphabetic() && bytes.get(1) == Some(&b':'))
                        .map(|l| char::from(*l))
                });
                if let Some(drive) = letter.filter(|d| found.iter().all(|(known, _)| known != d)) {
                    if let Some(directory) = self.operations.drive_directory(drive) {
                        found.push((drive, directory));
                    }
                }
            }
            found
        };
        #[cfg(windows)]
        let drives: Vec<_> = directories.iter().map(|(d, p)| (*d, p.as_str())).collect();
        #[cfg(not(windows))]
        let drives = [];
        let with_drives: Result<Vec<_>, _> =
            groups.iter().map(|g| try_resolve(g, &drives)).collect();
        if let Ok(paths) = with_drives {
            return Ok(paths);
        }
        let current = self.operations.current_dir()?;
        let cwd = Cwd {
            current: &current,
            drive_directories: &drives,
        };
        Ok(groups.iter().map(|g| resolve(g, &cwd)).collect())
    }
    /// Normalizes local input for storage, leaving nonlocal spelling intact.
    pub(super) fn normalize_source(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<String> {
        if let Source::Local(path) = sources::parse(source) {
            let resolved = self.resolve_local(path, &self.options.cwd)?;
            let base = self.resolve_operands(&[&self.base(scope)])?;
            let resolved = self.resolve_operands(&[&resolved])?;
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
