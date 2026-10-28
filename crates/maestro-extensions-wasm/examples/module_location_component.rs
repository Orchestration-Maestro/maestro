//! Author component that contributes resources installed beside its own artifact.
// maestro-component: MAESTRO_LOCATION_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    FlagOptions, FlagType, FlagValue, Presence, ResourcesDiscoverResult,
};

/// Variable through which the host names the artifact this instance was loaded from.
const LOCATION: &str = "MAESTRO_EXTENSION_PATH";

/// File name of every installation of this component.
const ARTIFACT: &str = "index.wasm";

/// Reports the instance's location as a flag default and registers its resource contribution.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "location",
            FlagOptions {
                description: None,
                type_: FlagType::String,
                default: std::env::var(LOCATION).ok().map(FlagValue::String),
            },
        )?;
        api.on("resources_discover", Rc::new(resources))
    })
}

/// Names the skill, prompt and theme beside the installed artifact.
///
/// The location is a host path, so only the known file name is removed and the host's
/// separator is kept.
fn resources(
    event: &mut ExtensionEvent,
    _context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::ResourcesDiscover(_) = event else {
            return Err("unexpected resource callback".into());
        };
        let location = std::env::var(LOCATION).map_err(|error| error.to_string())?;
        let directory = location
            .strip_suffix(ARTIFACT)
            .ok_or("unexpected artifact name")?;
        let sibling = |name: &str| Presence::Present(vec![format!("{directory}{name}")]);
        Ok(Some(ExtensionEventResult::ResourcesDiscover(
            ResourcesDiscoverResult {
                skill_paths: sibling("SKILL.md"),
                prompt_paths: sibling("dynamic.md"),
                theme_paths: sibling("dynamic.json"),
            },
        )))
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
