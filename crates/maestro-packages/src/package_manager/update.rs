//! Ordered configured-source availability without acquisition.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageFuture, PackageOperations,
    configuration::{entry_source, packages},
    sources::{self, Source},
};
use std::{collections::HashSet, io, rc::Rc};

/// The kind of an available package change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageUpdateType {
    /// An npm package version change.
    Npm,
    /// A Git remote commit change.
    Git,
}
/// An available change for an installed configured source.
#[derive(Debug, PartialEq, Eq)]
pub struct PackageUpdate {
    /// Original configured spelling.
    pub source: String,
    /// Package name or repository host/path.
    pub display_name: String,
    /// The changed source kind.
    pub r#type: PackageUpdateType,
    /// Selected configuration scope.
    pub scope: InstalledSourceScope,
}
/// An owned source selected before workers start.
pub(super) struct Candidate {
    /// Original spelling.
    pub source: String,
    /// Configuration scope.
    pub scope: InstalledSourceScope,
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Checks installed unpinned npm/Git sources with project registration precedence.
    /// Does not acquire contents, persist settings or emit progress.
    /// Failed version/head probes are skipped; consumed settings/path/root failures reject.
    /// Admitted siblings continue after an error while the caller drives its local runtime.
    /// # Errors
    /// Returns consumed settings, path, root lookup or worker-admission failures.
    pub fn check_for_available_updates(
        self: &Rc<Self>,
    ) -> PackageFuture<'static, Vec<PackageUpdate>>
    where
        O: 'static,
    {
        let manager = Rc::clone(self);
        Box::pin(async move {
            if manager.offline() {
                return Ok(Vec::new());
            }
            manager
                .check_candidates(manager.selected_candidates()?)
                .await
        })
    }
    /// Captures both scope roots before resolving ordered source identities.
    fn selected_candidates(&self) -> io::Result<Vec<Candidate>> {
        let user = self.snapshot(InstalledSourceScope::User)?;
        let project = self.snapshot(InstalledSourceScope::Project)?;
        let project = packages(project)?;
        let user = packages(user)?;
        let entries: Vec<_> = project
            .into_iter()
            .map(|v| (InstalledSourceScope::Project, v))
            .chain(user.into_iter().map(|v| (InstalledSourceScope::User, v)))
            .collect();
        let mut identities = HashSet::new();
        let mut selected = Vec::new();
        for (scope, entry) in &entries {
            let source = entry_source(entry)?;
            if identities.insert(self.identity(source, &self.base(*scope))?) {
                selected.push(Candidate {
                    source: source.to_owned(),
                    scope: *scope,
                });
            }
        }
        Ok(selected)
    }
    /// Reads offline mode without trimming the environment value.
    pub(super) fn offline(&self) -> bool {
        self.operations.offline_value().is_some_and(|value| {
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        })
    }
    /// Applies pin admission before inspecting installed contents.
    pub(super) async fn check_candidate(
        &self,
        candidate: Candidate,
    ) -> io::Result<Option<PackageUpdate>> {
        let (display_name, kind) = match sources::parse(&candidate.source) {
            Source::Npm { name, spec } if spec == name => (name.to_owned(), PackageUpdateType::Npm),
            Source::Git(git) if git.r#ref.is_none() => {
                (format!("{}/{}", git.host, git.path), PackageUpdateType::Git)
            }
            _ => return Ok(None),
        };
        let Some(path) = self.installed_path(&candidate.source, candidate.scope)? else {
            return Ok(None);
        };
        let changed = match kind {
            PackageUpdateType::Npm => self.npm_update_available(&display_name, &path).await,
            PackageUpdateType::Git => self.git_update_available(&path).await,
        };
        Ok(changed.then_some(PackageUpdate {
            source: candidate.source,
            display_name,
            r#type: kind,
            scope: candidate.scope,
        }))
    }
}
