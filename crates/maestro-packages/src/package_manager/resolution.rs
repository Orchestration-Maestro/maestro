//! Explicit source resolution with resource provenance.
use maestro_resources::{PathMetadata, paths::CONFIG_DIR_NAME};
/// One discovered resource with its original path and provenance.
pub struct ResolvedResource {
    /// The authored resource path.
    pub path: String,
    /// Whether the resource is enabled.
    pub enabled: bool,
    /// The retained source owner after resource precedence selection.
    pub metadata: PathMetadata,
}
/// One selected package, even when none of its resources are enabled.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedPackage {
    /// Original source spelling, including its pin.
    pub source: String,
    /// Kind-prefixed identity independent of the pin.
    pub identity: String,
    /// Scope used to resolve the contents.
    pub scope: SourceScope,
    /// Existing package contents.
    pub installed_path: String,
}
/// Resources separated by their discovery kind.
#[derive(Default)]
pub struct ResolvedPaths {
    /// Selected packages in resolution order, independent of resource filters.
    pub packages: Vec<ResolvedPackage>,
    /// Extension entries.
    pub extensions: Vec<ResolvedResource>,
    /// Skill entries.
    pub skills: Vec<ResolvedResource>,
    /// Prompt templates.
    pub prompts: Vec<ResolvedResource>,
    /// Theme files.
    pub themes: Vec<ResolvedResource>,
    /// Browser contribution files.
    pub contributions: Vec<ResolvedResource>,
    /// Manifest fields that were ignored while resolving.
    pub diagnostics: Vec<maestro_resources::ResourceDiagnostic>,
}
use super::{
    DefaultPackageManager, PackageOperations,
    resource_discovery::{ResourceType, collect_resource_files},
    sources::{self, Source},
};
use maestro_path::{dirname, join};
use maestro_resources::{SourceOrigin, SourceScope, canonicalize_path, skills::ResourceFileType};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};
/// Source spelling, configuration scope and optional raw object filters.
pub(super) struct ResolutionSource<'a> {
    /// Original source spelling.
    pub(super) source: &'a str,
    /// Lookup scope.
    pub(super) scope: SourceScope,
    /// Deferred resource filters.
    pub(super) filter: Option<&'a serde_json::Map<String, serde_json::Value>>,
}
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
            self.resolve_source(
                ResolutionSource {
                    source,
                    scope,
                    filter: None,
                },
                None,
                &mut result,
            )
            .await?;
        }
        Ok(result.finish())
    }
    /// Admits local sources and acquires or refreshes remote contents before collection.
    pub(super) async fn resolve_source<'a>(
        &'a self,
        input: ResolutionSource<'_>,
        on_missing: Option<&'a super::MissingCallback<'a>>,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let ResolutionSource {
            source,
            scope,
            filter,
        } = input;
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
                SourceScope::Project => join(&[&self.options.cwd, CONFIG_DIR_NAME]).into(),
                SourceScope::Temporary => self.options.cwd.as_str().into(),
            };
            let path = super::paths::resolve_local(path, &base, super::operations::home_dir)?;
            if Path::new(&path).exists() {
                result.package(source, scope, &path, &base)?;
            }
            Self::collect_local(path, &mut metadata, filter, result);
            return Ok(());
        }
        let path = self.remote_path(&parsed, scope)?;
        let exists = Path::new(&path).exists();
        let mismatched = exists
            && matches!(&parsed, Source::Npm { spec, name } if spec != name && super::npm::installed_npm_version(&path).as_deref() != Some(&spec[name.len()+1..]));
        if !exists || mismatched {
            if !self.admit_missing(source, on_missing).await? {
                return Ok(());
            }
            match &parsed {
                Source::Npm { spec, .. } => self.install_npm(spec, scope).await?,
                Source::Git(git) => {
                    self.install_git(git, scope).await?;
                }
                Source::Local(_) => {}
            }
        } else if let Source::Git(git) = &parsed
            && scope == SourceScope::Temporary
            && git.r#ref.is_none()
            && !self.offline()
        {
            self.refresh_temporary_git(git, source).await;
        }
        result.package(source, scope, &path, "")?;
        metadata.base_dir = Some(path.clone());
        Self::collect_package(&path, &metadata, filter, result)?;
        Ok(())
    }
    /// Applies offline refusal before invoking the operation-local missing callback.
    async fn admit_missing<'a>(
        &'a self,
        source: &str,
        on_missing: Option<&'a super::MissingCallback<'a>>,
    ) -> io::Result<bool> {
        if self.offline() {
            return Ok(false);
        }
        let action = match on_missing {
            Some(callback) => callback(source).await?,
            None => super::MissingSourceAction::Install,
        };
        match action {
            super::MissingSourceAction::Install => Ok(true),
            super::MissingSourceAction::Skip => Ok(false),
            super::MissingSourceAction::Error => {
                Err(io::Error::other(format!("Missing source: {source}")))
            }
        }
    }
    /// Suppresses local stat and discovery failures after path resolution.
    fn collect_local(
        path: String,
        metadata: &mut PathMetadata,
        filter: Option<&serde_json::Map<String, serde_json::Value>>,
        result: &mut Accumulator,
    ) {
        if !Path::new(&path).exists() {
            return;
        }
        match maestro_resources::metadata(Path::new(&path)) {
            Ok(ResourceFileType::File) => {
                metadata.base_dir = Some(dirname(&path));
                result.insert(ResourceType::Extensions, path, metadata);
            }
            Ok(ResourceFileType::Directory) => {
                metadata.base_dir = Some(path.clone());
                if let Ok(false) = Self::collect_package(&path, metadata, filter, result) {
                    result.insert(ResourceType::Extensions, path, metadata);
                }
            }
            _ => {}
        }
    }
    /// Inserts each selected kind only after its complete candidate selection succeeds.
    fn collect_manifest(
        manifest: &super::manifest::Manifest,
        path: &str,
        metadata: &PathMetadata,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        for kind in ResourceType::ALL {
            let entries = manifest.entries(kind.name())?;
            let files = super::manifest::collect_manifest_entries(&entries, path, kind)?;
            let enabled = super::resource_patterns::apply_patterns(&files, &entries, path)?;
            for file in files.iter().filter(|file| enabled.contains(file.as_str())) {
                result.insert(kind, file.clone(), metadata);
            }
        }
        Ok(())
    }
    /// Collects conventional paths, retaining results produced by earlier kinds.
    pub(super) fn collect_conventional(
        path: &str,
        metadata: &PathMetadata,
        result: &mut Accumulator,
    ) -> io::Result<bool> {
        if let Some(manifest) =
            super::manifest::read_maestro_manifest_file(&join(&[path, "package.json"]))
        {
            Self::collect_manifest(&manifest, path, metadata, result)?;
            return Ok(true);
        }
        let mut found = false;
        for kind in ResourceType::ALL {
            let dir = join(&[path, kind.name()]);
            if !Path::new(&dir).exists() {
                continue;
            }
            found = true;
            for path in collect_resource_files(&dir, kind)? {
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
            ResourceType::Contributions => &mut self.contributions,
        }
    }
}

/// Authored-path ownership before ranked canonical selection.
#[derive(Default)]
pub(super) struct Accumulator {
    /// Ordered resources.
    paths: ResolvedPaths,
    /// Authored identities already admitted for each kind.
    seen: [HashMap<String, usize>; 5],
}
impl Accumulator {
    /// Records an admitted package without deriving it from resource files.
    fn package(
        &mut self,
        source: &str,
        scope: SourceScope,
        path: &str,
        base: &str,
    ) -> io::Result<()> {
        self.paths.packages.push(ResolvedPackage {
            source: source.to_owned(),
            identity: super::configuration::identity_text(source, base)?,
            scope,
            installed_path: path.to_owned(),
        });
        Ok(())
    }
    /// Stably ranks resources before retaining the first canonical identity.
    pub(super) fn finish(mut self) -> ResolvedPaths {
        for kind in ResourceType::ALL {
            let items = self.paths.items(kind);
            items.sort_by_key(|resource| precedence(&resource.metadata));
            let mut seen = HashSet::new();
            items.retain(|resource| seen.insert(canonicalize_path(&resource.path)));
        }
        self.paths
    }
    /// Inserts an enabled candidate with precedence-aware authored ownership.
    pub(super) fn insert(&mut self, kind: ResourceType, path: String, metadata: &PathMetadata) {
        self.insert_enabled(kind, path, metadata, true);
    }
    /// Replaces a worse-ranked owner in its existing authored-key position.
    pub(super) fn insert_enabled(
        &mut self,
        kind: ResourceType,
        path: String,
        metadata: &PathMetadata,
        enabled: bool,
    ) {
        let items = self.paths.items(kind);
        let seen = &mut self.seen[kind as usize];
        let resource = ResolvedResource {
            path,
            enabled,
            metadata: metadata.clone(),
        };
        if let Some(&index) = seen.get(&resource.path) {
            if precedence(metadata) < precedence(&items[index].metadata) {
                items[index] = resource;
            }
        } else {
            seen.insert(resource.path.clone(), items.len());
            items.push(resource);
        }
    }
}
/// Ranks top-level scope ownership ahead of package ownership.
fn precedence(metadata: &PathMetadata) -> u8 {
    if metadata.origin == SourceOrigin::Package {
        return 4;
    }
    let scope = if metadata.scope == SourceScope::Project {
        0
    } else {
        2
    };
    scope + u8::from(metadata.source != "local")
}
