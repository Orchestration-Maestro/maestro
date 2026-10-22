//! Enabled package contributions read for the browser frontend.
pub use maestro_protocol::contributions::{
    BrowserContribution, BrowserModule, ContributionDescriptor, ContributionSlot,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_resources::{DiagnosticType, ResourceDiagnostic};

/// Read each enabled contribution file in order, reporting unreadable or invalid files.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn load_contributions(
    resources: &[crate::ResolvedResource],
) -> (Vec<BrowserContribution>, Vec<ResourceDiagnostic>) {
    let mut accepted = Vec::new();
    let mut diagnostics = Vec::new();
    for resource in resources.iter().filter(|resource| resource.enabled) {
        let parsed = maestro_resources::read_text(std::path::Path::new(&resource.path))
            .map_err(|error| error.to_string())
            .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()));
        match parsed {
            Ok(descriptor) => accepted.push(BrowserContribution {
                package: resource.metadata.source.clone(),
                descriptor,
            }),
            Err(message) => diagnostics.push(ResourceDiagnostic {
                r#type: DiagnosticType::Warning,
                message,
                path: Some(resource.path.clone()),
                collision: None,
            }),
        }
    }
    (accepted, diagnostics)
}
