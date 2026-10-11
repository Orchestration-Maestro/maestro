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
    DefaultPackageManager, PackageOperations,
    resource_discovery::{ResourceType, collect_resource_files},
    sources::{self, Source},
};
use maestro_path::{dirname, join};
use maestro_resources::{SourceOrigin, SourceScope, canonicalize_path, skills::ResourceFileType};
use std::{collections::HashSet, io, path::Path};
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Resolves each explicit source sequentially before final canonical deduplication.
    pub(super) async fn resolve_sources(
        &self,
        sources: &[String],
        scope: Option<SourceScope>,
    ) -> io::Result<ResolvedPaths> {
        let scope = scope.unwrap_or(SourceScope::User);
        let mut result = Accumulator::default();
        for source in sources {
            self.resolve_source(source, scope, &mut result).await?;
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
    /// Admits local sources and acquires or refreshes remote contents before collection.
    async fn resolve_source(
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
        let path = self.remote_path(&parsed, scope)?;
        let exists = self.operations.exists(&path);
        let mismatched = exists
            && matches!(&parsed, Source::Npm { spec, name } if spec != name && self.installed_npm_version(&path).as_deref() != Some(&spec[name.len()+1..]));
        if !exists || mismatched {
            if self.offline() {
                return Ok(());
            }
            match &parsed {
                Source::Npm { spec, .. } => self.install_npm(spec, scope).await?,
                Source::Git(git) => self.install_git(git, scope).await?,
                Source::Local(_) => {}
            }
        } else if let Source::Git(git) = &parsed
            && scope == SourceScope::Temporary
            && git.r#ref.is_none()
            && !self.offline()
        {
            self.refresh_temporary_git(git, source).await;
        }
        metadata.base_dir = Some(path.clone());
        self.collect_conventional(&path, &metadata, result)?;
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
    /// Inserts each selected kind only after its complete candidate selection succeeds.
    fn collect_manifest(
        &self,
        manifest: &super::manifest::Manifest,
        path: &str,
        metadata: &PathMetadata,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let operations = self.operations.resource_operations();
        for kind in ResourceType::ALL {
            let entries = manifest.entries(kind.name())?;
            let files =
                super::manifest::collect_manifest_entries(&entries, path, kind, operations)?;
            let enabled =
                super::resource_patterns::apply_patterns(&files, &entries, path, operations)?;
            for file in files.iter().filter(|file| enabled.contains(file.as_str())) {
                result.insert(kind, file.clone(), metadata);
            }
        }
        Ok(())
    }
    /// Collects conventional paths, retaining results produced by earlier kinds.
    fn collect_conventional(
        &self,
        path: &str,
        metadata: &PathMetadata,
        result: &mut Accumulator,
    ) -> io::Result<bool> {
        let operations = self.operations.resource_operations();
        if let Some(manifest) =
            super::manifest::read_maestro_manifest_file(&join(&[path, "package.json"]), operations)
        {
            self.collect_manifest(&manifest, path, metadata, result)?;
            return Ok(true);
        }
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
