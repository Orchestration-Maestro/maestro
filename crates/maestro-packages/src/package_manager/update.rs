//! Configured update selection, execution and read-only availability.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageFuture, PackageOperations,
    configuration::{entry_source, packages},
    sources::{self, Source},
};
use std::{io, rc::Rc};

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
pub(super) struct ConfiguredUpdateSource {
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
    fn selected_candidates(&self) -> io::Result<Vec<ConfiguredUpdateSource>> {
        let user = self.snapshot(InstalledSourceScope::User)?;
        let project = self.snapshot(InstalledSourceScope::Project)?;
        let mut selected = Vec::new();
        for (scope, entry) in self.selected_package_entries(&user, &project)? {
            selected.push(ConfiguredUpdateSource {
                source: entry_source(entry)?.to_owned(),
                scope,
            });
        }
        Ok(selected)
    }
    /// Applies pin admission before inspecting installed contents.
    pub(super) async fn check_candidate(
        &self,
        candidate: ConfiguredUpdateSource,
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

impl<O: PackageOperations + 'static> DefaultPackageManager<O> {
    /// Admits availability jobs through the shared indexed worker implementation.
    async fn check_candidates(
        self: &Rc<Self>,
        candidates: Vec<ConfiguredUpdateSource>,
    ) -> io::Result<Vec<PackageUpdate>> {
        let tasks = candidates
            .into_iter()
            .map(|candidate| {
                let manager = Rc::clone(self);
                Box::pin(async move { manager.check_candidate(candidate).await })
                    as PackageFuture<'static, _>
            })
            .collect();
        Ok(self
            .run_with_concurrency(tasks)
            .await?
            .into_iter()
            .flatten()
            .collect())
    }
    /// Selects configured rows before applying offline and pin admission.
    pub(super) async fn update_configured(
        self: Rc<Self>,
        source: Option<String>,
    ) -> io::Result<()> {
        let rows = self.selected_update_sources(source.as_deref())?;
        if self.offline() {
            return Ok(());
        }
        let mut npm = Vec::new();
        let mut git = Vec::new();
        for row in rows {
            match sources::parse(&row.source) {
                Source::Npm { name, spec } if name == spec => npm.push(row),
                Source::Git(parsed) if parsed.r#ref.is_none() => git.push((row, parsed)),
                _ => {}
            }
        }
        let tasks: Vec<PackageFuture<'static, Option<ConfiguredUpdateSource>>> = npm
            .into_iter()
            .map(|row| {
                let manager = Rc::clone(&self);
                Box::pin(async move {
                    let needed = manager.should_update_npm(&row).await?;
                    Ok(needed.then_some(row))
                }) as PackageFuture<'static, Option<ConfiguredUpdateSource>>
            })
            .collect();
        let needed = self.run_with_concurrency(tasks).await?;
        let (user, project): (Vec<_>, Vec<_>) = needed
            .into_iter()
            .flatten()
            .partition(|row| row.scope == InstalledSourceScope::User);
        let mut groups = Vec::new();
        for (scope, rows) in [
            (InstalledSourceScope::User, user),
            (InstalledSourceScope::Project, project),
        ] {
            if !rows.is_empty() {
                groups.push(self.npm_batch_job(rows, scope));
            }
        }
        if !git.is_empty() {
            let manager = Rc::clone(&self);
            let tasks = git
                .into_iter()
                .map(|(row, git)| self.git_update_job(row, git))
                .collect();
            groups.push(Box::pin(async move {
                manager.run_with_concurrency(tasks).await.map(|_| ())
            }));
        }
        self.run_with_concurrency(groups).await?;
        Ok(())
    }
    /// Owns the selected npm rows until grouped execution finishes.
    fn npm_batch_job(
        self: &Rc<Self>,
        rows: Vec<ConfiguredUpdateSource>,
        scope: InstalledSourceScope,
    ) -> PackageFuture<'static, ()> {
        let manager = Rc::clone(self);
        Box::pin(async move { manager.update_npm_batch(&rows, scope).await })
    }
    /// Owns the selected Git source through progress and refresh.
    fn git_update_job(
        self: &Rc<Self>,
        row: ConfiguredUpdateSource,
        git: crate::GitSource,
    ) -> PackageFuture<'static, ()> {
        let manager = Rc::clone(self);
        Box::pin(async move {
            manager
                .with_progress(
                    super::ProgressAction::Update,
                    &row.source,
                    format!("Updating {}...", row.source),
                    manager.update_git(&git, row.scope.resource_scope()),
                )
                .await
        })
    }
    /// Captures both settings roots before consuming user then project entries.
    fn selected_update_sources(
        &self,
        source: Option<&str>,
    ) -> io::Result<Vec<ConfiguredUpdateSource>> {
        let user = self.snapshot(InstalledSourceScope::User)?;
        let project = self.snapshot(InstalledSourceScope::Project)?;
        let source = source.filter(|source| !source.is_empty());
        let identity = source
            .map(|source| super::configuration::identity(source, &self.options.cwd))
            .transpose()?;
        let mut selected = Vec::new();
        let mut configured = Vec::new();
        for (scope, snapshot) in [
            (InstalledSourceScope::User, user),
            (InstalledSourceScope::Project, project),
        ] {
            let rows = self.update_scope_sources(snapshot, scope, identity.as_ref())?;
            let (matched, unmatched): (Vec<_>, Vec<_>) =
                rows.into_iter().partition(|(_, matched)| *matched);
            configured.extend(unmatched.into_iter().map(|(source, _)| source));
            selected.extend(
                matched
                    .into_iter()
                    .map(|(source, _)| ConfiguredUpdateSource { source, scope }),
            );
        }
        if selected.is_empty()
            && let Some(source) = source
        {
            return Err(io::Error::other(sources::no_match_message(
                source,
                &configured,
            )));
        }
        Ok(selected)
    }
    /// Consumes one scope in order, resolving stored identities only for named input.
    fn update_scope_sources(
        &self,
        snapshot: maestro_settings::Settings,
        scope: InstalledSourceScope,
        identity: Option<&super::configuration::Identity<'_>>,
    ) -> io::Result<Vec<(String, bool)>> {
        packages(snapshot)?
            .iter()
            .map(|entry| {
                let source = entry_source(entry)?;
                let matched = match identity {
                    Some(identity) => {
                        super::configuration::identity(source, &self.base(scope))? == *identity
                    }
                    None => true,
                };
                Ok((source.to_owned(), matched))
            })
            .collect()
    }
}
