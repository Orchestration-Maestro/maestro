//! Automatic resource roots and scope-specific overrides.
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    configured_resolution::root_entries,
    resolution::Accumulator,
    resource_discovery::{ResourceType, collect_auto_files},
};
use maestro_path::{dirname, join};
use maestro_resources::paths::package_home_directory;
use maestro_resources::{PathMetadata, SourceOrigin};
use maestro_settings::Settings;
use std::{io, path::Path};
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Collects project roots before user roots after required ambient resolution.
    pub(super) fn add_automatic(
        &self,
        user: &Settings,
        project: &Settings,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let home = join(&[&package_home_directory()?, ".agents", "skills"]);
        let resolved_home = maestro_resources::paths::resolve(&[&home])?;
        let mut current = maestro_resources::paths::resolve(&[&self.options.cwd])?;
        let mut ancestors = Vec::new();
        loop {
            let root = join(&[&current, ".agents", "skills"]);
            if root != resolved_home {
                ancestors.push(root);
            }
            let parent = dirname(&current);
            if Path::exists(Path::new(&join(&[&current, ".git"]))) || parent == current {
                break;
            }
            current = parent;
        }
        for (scope, snapshot) in [
            (InstalledSourceScope::Project, project),
            (InstalledSourceScope::User, user),
        ] {
            let roots = if scope == InstalledSourceScope::User {
                std::slice::from_ref(&home)
            } else {
                &ancestors
            };
            self.collect_auto_scope(snapshot, scope, roots, result)?;
        }
        Ok(())
    }
    /// Adds one scope's conventional roots followed by its agents skill roots.
    fn collect_auto_scope(
        &self,
        snapshot: &Settings,
        scope: InstalledSourceScope,
        roots: &[String],
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let base = self.base(scope);
        let metadata = PathMetadata {
            source: "auto".into(),
            scope: scope.resource_scope(),
            origin: SourceOrigin::TopLevel,
            base_dir: Some(base.to_string()),
        };
        for kind in ResourceType::ALL {
            let entries = root_entries(snapshot, kind.name())?;
            let mut files = collect_auto_files(&join(&[&base, kind.name()]), kind, false)?;
            let roots = if matches!(kind, ResourceType::Skills) {
                roots
            } else {
                &[]
            };
            for root in roots {
                files.extend(collect_auto_files(root, kind, true)?);
            }
            let selected = super::resource_patterns::apply_patterns(&files, &entries, &base)?;
            for file in &files {
                result.insert_enabled(kind, file.clone(), &metadata, selected.contains(file));
            }
        }
        Ok(())
    }
}
