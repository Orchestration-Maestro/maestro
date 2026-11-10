//! Enabled package contributions read for the browser frontend.
pub use maestro_protocol::contributions::{
    BrowserContribution, BrowserModule, ContributionDescriptor, ContributionSlot,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_resources::{DiagnosticType, ResourceDiagnostic};

/// Loaded browser contributions and their diagnostics.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct LoadContributionsResult {
    /// Accepted contributions in resolution order.
    pub contributions: Vec<BrowserContribution>,
    /// Unreadable or invalid files.
    pub diagnostics: Vec<ResourceDiagnostic>,
}

/// Read each enabled contribution file in order, reporting unreadable or invalid files.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_contributions(resources: &[crate::ResolvedResource]) -> LoadContributionsResult {
    let mut loaded = LoadContributionsResult::default();
    for resource in resources.iter().filter(|resource| resource.enabled) {
        let parsed = maestro_resources::read_text(std::path::Path::new(&resource.path))
            .map_err(|error| error.to_string())
            .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()));
        match parsed {
            Ok(descriptor) => loaded.contributions.push(BrowserContribution {
                package: resource.metadata.source.clone(),
                descriptor,
            }),
            Err(message) => loaded.diagnostics.push(ResourceDiagnostic {
                r#type: DiagnosticType::Warning,
                message,
                path: Some(resource.path.clone()),
                collision: None,
            }),
        }
    }
    loaded
}
