//! Supplied source roots and explicitly replaced compiled snapshots.
use super::{
    Agent, AgentDetection, AgentManifest, AgentManifestSummary, DetectionExplain, DetectionInput,
    LoadedManifest, ManifestSource, Path, PathBuf, agent_label, bundled, compile_manifest,
    evaluate_loaded_manifest, fallback_explain, manifest_matches_agent, parse_agent_label,
    parse_manifest, parse_remote_manifest_for_agent,
};

/// A caller-owned compiled snapshot, isolated from every other detection cache.
#[derive(Debug)]
pub struct ManifestCache {
    /// Directory containing optional local agent documents.
    override_root: Option<PathBuf>,
    /// Directory containing downloaded documents and update status.
    catalog_root: Option<PathBuf>,
    /// Compiled documents in screen-agent catalogue order.
    manifests: Vec<(Agent, LoadedManifest)>,
}

impl ManifestCache {
    /// Loads bundled documents and any supplied local or downloaded sources.
    ///
    /// # Errors
    /// Returns a validation or compilation error for an invalid bundled document.
    pub fn new(
        override_root: Option<PathBuf>,
        catalog_root: Option<PathBuf>,
    ) -> Result<Self, String> {
        let mut cache = Self {
            override_root,
            catalog_root,
            manifests: Vec::new(),
        };
        cache.reload_manifests()?;
        Ok(cache)
    }

    /// Rebuilds the complete snapshot before replacing the retained one.
    ///
    /// # Errors
    /// Returns a bundled-document error without replacing the existing snapshot.
    pub fn reload_manifests(&mut self) -> Result<Vec<AgentManifestSummary>, String> {
        let manifests = Agent::SCREEN_MANIFEST_AGENTS
            .into_iter()
            .map(|agent| self.load_uncached(agent).map(|loaded| (agent, loaded)))
            .collect::<Result<_, _>>()?;
        self.manifests = manifests;
        Ok(self.manifest_summaries())
    }

    /// Reports active sources in catalogue order without copying compiled rules.
    #[must_use]
    pub fn manifest_summaries(&self) -> Vec<AgentManifestSummary> {
        self.manifests
            .iter()
            .map(|(agent, loaded)| AgentManifestSummary {
                agent: *agent,
                active_source: loaded.source.clone(),
                active_version: loaded.manifest.version.as_ref().map(ToString::to_string),
                cached_remote_version: loaded.cached_remote_version.clone(),
                local_override_shadowing_remote: loaded.local_override_shadowing_remote,
                warning: loaded.warning.clone(),
            })
            .collect()
    }

    /// Detects live state from screen content.
    #[must_use]
    pub fn detect(&self, agent: Agent, screen: &str) -> AgentDetection {
        self.detect_with_osc(
            agent,
            DetectionInput {
                screen,
                osc_title: "",
                osc_progress: "",
            },
        )
    }

    /// Detects live state from screen and OSC evidence.
    #[must_use]
    pub fn detect_with_osc(&self, agent: Agent, input: DetectionInput<'_>) -> AgentDetection {
        self.evaluate(agent, input, None).into_detection()
    }

    /// Explains screen detection with the currently published update status.
    #[must_use]
    pub fn explain(&self, agent: Agent, screen: &str) -> DetectionExplain {
        self.explain_with_input(
            agent,
            DetectionInput {
                screen,
                osc_title: "",
                osc_progress: "",
            },
        )
    }

    /// Explains detection using dedicated screen, title and progress fields.
    #[must_use]
    pub fn explain_with_input(&self, agent: Agent, input: DetectionInput<'_>) -> DetectionExplain {
        let status = self
            .catalog_root
            .as_deref()
            .and_then(|root| crate::updates::load_status(root).agent_status(agent));
        self.evaluate(agent, input, status)
    }

    /// Explains recognized labels, preserving unknown-label spelling on failure.
    #[must_use]
    pub fn explain_for_label(&self, label: &str, screen: &str) -> DetectionExplain {
        if let Some(agent) = parse_agent_label(label) {
            return self.explain(agent, screen);
        }
        let mut output = fallback_explain(None, None, None);
        output.agent = Some(label.to_owned());
        output.fallback_reason = Some("unknown_agent".to_owned());
        output
    }

    /// Returns whether the live screen is a transcript that must not update state.
    #[must_use]
    pub fn should_skip_state_update(&self, agent: Agent, screen: &str) -> bool {
        self.detect(agent, screen).skip_state_update
    }

    /// Evaluates only retained compiled rules; filesystem reads are status-only.
    fn evaluate(
        &self,
        agent: Agent,
        input: DetectionInput<'_>,
        status: Option<crate::updates::AgentRemoteStatus>,
    ) -> DetectionExplain {
        match self
            .manifests
            .iter()
            .find(|(candidate, _)| *candidate == agent)
        {
            Some((_, loaded)) => evaluate_loaded_manifest(agent, input, loaded, status),
            None => fallback_explain(Some(agent), None, status),
        }
    }

    /// Selects a validated source and compiles its rules once for this snapshot.
    fn load_uncached(&self, agent: Agent) -> Result<LoadedManifest, String> {
        let (_, content) = bundled::BUNDLED_MANIFESTS
            .iter()
            .find(|(id, _)| *id == agent_label(agent))
            .ok_or_else(|| format!("missing bundled manifest for {}", agent_label(agent)))?;
        let bundled = parse_manifest(content)?;
        let mut selected = LoadedManifest::new(bundled, ManifestSource::Bundled)?;
        if let Some(root) = &self.catalog_root {
            let path = crate::updates::remote_manifest_path(root, agent);
            if path.exists() {
                selected.select_remote(agent, &path);
            }
        }
        if let Some(root) = &self.override_root {
            let path = root.join(format!("{}.toml", agent_label(agent)));
            if path.exists() {
                selected.select_override(agent, &path);
            }
        }
        Ok(selected)
    }
}

impl LoadedManifest {
    /// Compiles an admitted document into a retained source snapshot.
    fn new(manifest: AgentManifest, source: ManifestSource) -> Result<Self, String> {
        let compiled_rules = compile_manifest(&manifest)?;
        Ok(Self {
            manifest,
            compiled_rules,
            source,
            warning: None,
            cached_remote_version: None,
            local_override_shadowing_remote: false,
        })
    }

    /// Admits a compatible download without demoting a newer bundled version.
    fn select_remote(&mut self, agent: Agent, path: &Path) {
        let result = std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|content| parse_remote_manifest_for_agent(agent, &content));
        match result {
            Ok(parsed) => {
                self.cached_remote_version = Some(parsed.version.to_string());
                if self
                    .manifest
                    .version
                    .as_ref()
                    .is_some_and(|bundled| parsed.version < *bundled)
                {
                    self.warning = Some(format!(
                        "ignored remote manifest {} because cached version {} is older than bundled {}",
                        path.display(),
                        parsed.version,
                        self.manifest
                            .version
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    ));
                    return;
                }
                let source = ManifestSource::Remote {
                    path: path.to_path_buf(),
                    version: parsed.version.to_string(),
                };
                match Self::new(parsed.manifest, source) {
                    Ok(mut loaded) => {
                        loaded.cached_remote_version = self.cached_remote_version.take();
                        *self = loaded;
                    }
                    Err(error) => {
                        self.warning = Some(format!(
                            "ignored remote manifest {} because it could not be compiled: {error}",
                            path.display()
                        ));
                    }
                }
            }
            Err(error) => {
                self.warning = Some(format!(
                    "ignored remote manifest {} because it could not be loaded: {error}",
                    path.display()
                ));
            }
        }
    }

    /// Gives a valid local override precedence while retaining download metadata.
    fn select_override(&mut self, agent: Agent, path: &Path) {
        self.local_override_shadowing_remote = self.cached_remote_version.is_some();
        let result = std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|content| parse_manifest(&content));
        match result {
            Ok(manifest) if manifest_matches_agent(&manifest, agent) => {
                match Self::new(manifest, ManifestSource::Override(path.to_path_buf())) {
                    Ok(mut loaded) => {
                        loaded.cached_remote_version = self.cached_remote_version.take();
                        loaded.local_override_shadowing_remote =
                            self.local_override_shadowing_remote;
                        *self = loaded;
                    }
                    Err(error) => {
                        self.warning = Some(format!(
                            "ignored override {} because it could not be compiled: {error}",
                            path.display()
                        ));
                    }
                }
            }
            Ok(manifest) => {
                self.warning = Some(format!(
                    "ignored override {} because manifest id {} does not match {}",
                    path.display(),
                    manifest.id,
                    agent_label(agent)
                ));
            }
            Err(error) => {
                self.warning = Some(format!(
                    "ignored override {} because it could not be loaded: {error}",
                    path.display()
                ));
            }
        }
    }
}
