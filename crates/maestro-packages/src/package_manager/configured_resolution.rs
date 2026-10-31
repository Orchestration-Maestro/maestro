//! Configured resource entries over captured settings.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    resolution::Accumulator,
    resource_discovery::{ResourceType, collect_files_from_paths},
};
use maestro_resources::paths::home_directory;
use maestro_resources::{PathMetadata, SourceOrigin};
use maestro_settings::Settings;
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Captures each scope before resolving resource entries.
    pub(super) async fn resolve_configured<'a>(
        &'a self,
        on_missing: Option<&'a super::MissingCallback<'a>>,
    ) -> io::Result<super::ResolvedPaths> {
        let user = self.snapshot(InstalledSourceScope::User)?;
        let project = self.snapshot(InstalledSourceScope::Project)?;
        let mut result = Accumulator::default();
        for (scope, entry) in self.selected_package_entries(&user, &project)? {
            self.resolve_source(
                super::resolution::ResolutionSource {
                    source: super::configuration::entry_source(entry)?,
                    scope: scope.resource_scope(),
                    filter: entry.as_object(),
                },
                on_missing,
                &mut result,
            )
            .await?;
        }
        for kind in ResourceType::ALL {
            let project_entries = root_entries(&project, kind.name())?;
            self.resolve_local_entries(
                project_entries,
                kind,
                InstalledSourceScope::Project,
                &mut result,
            )?;
            let user_entries = root_entries(&user, kind.name())?;
            self.resolve_local_entries(
                user_entries,
                kind,
                InstalledSourceScope::User,
                &mut result,
            )?;
        }
        self.add_automatic(&user, &project, &mut result)?;
        Ok(result.finish())
    }
    /// Collects literal top-level entries before selecting their enabled membership.
    fn resolve_local_entries(
        &self,
        entries: Vec<&str>,
        kind: ResourceType,
        scope: InstalledSourceScope,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let base = self.base(scope);
        let (patterns, plain): (Vec<_>, Vec<_>) =
            entries.into_iter().partition(|entry| is_pattern(entry));
        let paths = plain
            .iter()
            .map(|entry| super::paths::resolve_local(entry, &base, home_directory))
            .collect::<io::Result<Vec<_>>>()?;
        let metadata = PathMetadata {
            source: "local".into(),
            scope: scope.resource_scope(),
            origin: SourceOrigin::TopLevel,
            base_dir: None,
        };
        let files = collect_files_from_paths(paths, kind);
        let enabled = super::resource_patterns::select_patterns(&files, &patterns, &base)?;
        for path in &files {
            result.insert_enabled(kind, path.clone(), &metadata, enabled.contains(path));
        }
        Ok(())
    }
}
/// Consumes one optional root list without decoding other members.
pub(super) fn root_entries<'a>(settings: &'a Settings, name: &str) -> io::Result<Vec<&'a str>> {
    match settings.0.get(name) {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(serde_json::Value::Array(entries)) => entries
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "resource entry is not a string")
                })
            })
            .collect(),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "resource list is not an array",
        )),
    }
}

/// Recognizes configured top-level patterns without treating brackets or braces as discovery.
fn is_pattern(entry: &str) -> bool {
    entry.starts_with(['!', '+', '-']) || entry.contains(['*', '?'])
}
