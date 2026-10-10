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
        &mut self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<Option<String>> {
        let path = match sources::parse(source) {
            Source::Npm(name) => match scope {
                InstalledSourceScope::User => join(&[self.global_npm_root()?, name]),
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
    fn resolve_operands(&self, operands: &[&str]) -> io::Result<String> {
        if let Ok(path) = try_resolve(operands, &[]) {
            return Ok(path);
        }
        #[cfg(windows)]
        let directory = operands
            .iter()
            .rev()
            .find_map(|path| {
                let bytes = path.as_bytes();
                bytes
                    .first()
                    .filter(|letter| letter.is_ascii_alphabetic() && bytes.get(1) == Some(&b':'))
                    .map(|letter| char::from(*letter))
            })
            .and_then(|drive| {
                self.operations
                    .drive_directory(drive)
                    .map(|directory| (drive, directory))
            });
        #[cfg(windows)]
        let drives: Vec<_> = directory
            .iter()
            .map(|(drive, path)| (*drive, path.as_str()))
            .collect();
        #[cfg(not(windows))]
        let drives = [];
        #[cfg(windows)]
        if let Ok(path) = try_resolve(operands, &drives) {
            return Ok(path);
        }
        let current = self.operations.current_dir()?;
        Ok(resolve(
            operands,
            &Cwd {
                current: &current,
                drive_directories: &drives,
            },
        ))
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
