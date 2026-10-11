//! Explicit source resolution with resource provenance.
use maestro_resources::PathMetadata;
/// One enabled resource with its original path and provenance.
pub struct ResolvedResource {
    /// The authored resource path.
    pub path: String,
    /// Whether the resource is enabled.
    pub enabled: bool,
    /// The source that first supplied this path.
    pub metadata: PathMetadata,
}
/// Resources separated by their discovery kind.
#[derive(Default)]
pub struct ResolvedPaths {
    /// Extension entries.
    pub extensions: Vec<ResolvedResource>,
    /// Skill entries.
    pub skills: Vec<ResolvedResource>,
    /// Prompt templates.
    pub prompts: Vec<ResolvedResource>,
    /// Theme files.
    pub themes: Vec<ResolvedResource>,
}
use super::{
    DefaultPackageManager, InstalledSourceScope, PackageOperations,
    resource_discovery::{ResourceType, collect_resource_files},
    sources::{self, Source},
};
use maestro_path::{dirname, join};
use maestro_resources::{SourceOrigin, SourceScope, canonicalize_path, skills::ResourceFileType};
use std::{collections::HashSet, io, path::Path};
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Resolves local sources before suppressing their filesystem failures.
    pub(super) fn resolve_sources(
        &self,
        sources: &[String],
        scope: Option<SourceScope>,
    ) -> io::Result<ResolvedPaths> {
        let scope = scope.unwrap_or(SourceScope::User);
        let mut result = Accumulator::default();
        for source in sources {
            self.resolve_source(source, scope, &mut result)?;
        }
        for kind in ResourceType::ALL {
            let mut seen = HashSet::new();
            result.paths.items(kind).retain(|resource| {
                seen.insert(canonicalize_path(
                    &resource.path,
                    self.operations.resource_operations(),
                ))
            });
        }
        Ok(result.paths)
    }
    /// Admits local or already-installed unpinned remote sources.
    fn resolve_source(
        &self,
        source: &str,
        scope: SourceScope,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let parsed = sources::parse(source);
        let mut metadata = PathMetadata {
            source: source.to_owned(),
            scope,
            origin: SourceOrigin::Package,
            base_dir: None,
        };
        if let Source::Local(path) = parsed {
            let base = match scope {
                SourceScope::User => std::borrow::Cow::Borrowed(self.options.agent_dir.as_str()),
                SourceScope::Project => join(&[&self.options.cwd, ".maestro"]).into(),
                SourceScope::Temporary => self.options.cwd.as_str().into(),
            };
            let path = self.resolve_local(path, &base)?;
            self.collect_local(path, &mut metadata, result);
            return Ok(());
        }
        if scope == SourceScope::Temporary || matches!(parsed,Source::Npm {spec,name} if spec!=name)
        {
            return Ok(());
        }
        let installed_scope = if scope == SourceScope::Project {
            InstalledSourceScope::Project
        } else {
            InstalledSourceScope::User
        };
        if let Some(path) = self.installed_path(source, installed_scope)? {
            metadata.base_dir = Some(path.clone());
            self.collect_conventional(&path, &metadata, result)?;
        }
        Ok(())
    }
    /// Suppresses local stat and discovery failures after path resolution.
    fn collect_local(&self, path: String, metadata: &mut PathMetadata, result: &mut Accumulator) {
        let operations = self.operations.resource_operations();
        if !operations.exists(Path::new(&path)) {
            return;
        }
        match operations.metadata(Path::new(&path)) {
            Ok(ResourceFileType::File) => {
                metadata.base_dir = Some(dirname(&path));
                result.insert(ResourceType::Extensions, path, metadata);
            }
            Ok(ResourceFileType::Directory) => {
                metadata.base_dir = Some(path.clone());
                if let Ok(false) = self.collect_conventional(&path, metadata, result) {
                    result.insert(ResourceType::Extensions, path, metadata);
                }
            }
            _ => {}
        }
    }
    /// Collects conventional paths, retaining results produced by earlier kinds.
    fn collect_conventional(
        &self,
        path: &str,
        metadata: &PathMetadata,
        result: &mut Accumulator,
    ) -> io::Result<bool> {
        let operations = self.operations.resource_operations();
        let mut found = false;
        for kind in ResourceType::ALL {
            let dir = join(&[path, kind.name()]);
            if !operations.exists(Path::new(&dir)) {
                continue;
            }
            found = true;
            for path in collect_resource_files(&dir, kind, operations)? {
                result.insert(kind, path, metadata);
            }
        }
        Ok(found)
    }
}
impl ResolvedPaths {
    /// Selects the collection for a resource kind.
    fn items(&mut self, kind: ResourceType) -> &mut Vec<ResolvedResource> {
        match kind {
            ResourceType::Extensions => &mut self.extensions,
            ResourceType::Skills => &mut self.skills,
            ResourceType::Prompts => &mut self.prompts,
            ResourceType::Themes => &mut self.themes,
        }
    }
}

/// First-authored-path ownership before canonical conversion.
#[derive(Default)]
struct Accumulator {
    /// Ordered resources.
    paths: ResolvedPaths,
    /// Authored identities already admitted for each kind.
    seen: [HashSet<String>; 4],
}
impl Accumulator {
    /// Retains the first metadata for each authored identity.
    fn insert(&mut self, kind: ResourceType, path: String, metadata: &PathMetadata) {
        if self.seen[kind as usize].insert(path.clone()) {
            self.paths.items(kind).push(ResolvedResource {
                path,
                enabled: true,
                metadata: metadata.clone(),
            });
        }
    }
}
