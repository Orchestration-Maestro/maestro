//! Contributes a skill, prompt template and theme installed beside the component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
// maestro-component: MAESTRO_DYNAMIC_RESOURCES_COMPONENT
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionEventResult, ExtensionFuture, Presence, ResourcesDiscoverResult,
    tools::path_utils,
};
use std::rc::Rc;

/// Registers the sibling resources during discovery.
///
/// # Errors
///
/// The returned future fails if the component path is unavailable, its filename
/// cannot be resolved, or the discovery handler cannot be registered.
#[must_use]
pub fn dynamic_resources_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let location =
            std::env::var("MAESTRO_EXTENSION_PATH").map_err(|error| error.to_string())?;
        let leaf = path_utils::basename(&api, &location)?;
        let directory = location
            .strip_suffix(&leaf)
            .ok_or("missing component filename")?
            .to_owned();
        api.on(
            "resources_discover",
            Rc::new(move |_, _| {
                let sibling = |name: &str| Presence::Present(vec![format!("{directory}{name}")]);
                let result = ResourcesDiscoverResult {
                    skill_paths: sibling("SKILL.md"),
                    prompt_paths: sibling("dynamic.md"),
                    theme_paths: sibling("dynamic.json"),
                };
                Box::pin(async move { Ok(Some(ExtensionEventResult::ResourcesDiscover(result))) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(dynamic_resources_extension);
