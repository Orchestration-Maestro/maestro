//! Contributes a skill, prompt template and theme installed beside the component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionEventResult, ExtensionFuture, Presence, ResourcesDiscoverResult,
};
use std::rc::Rc;

/// Registers the sibling resources during discovery.
#[must_use]
pub fn dynamic_resources_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let mut directory =
            std::env::var("MAESTRO_EXTENSION_PATH").map_err(|error| error.to_string())?;
        // The path belongs to the host, which may use either directory separator.
        let separator = directory
            .rfind(['/', '\\'])
            .ok_or("missing component directory")?;
        directory.truncate(separator + 1);
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
