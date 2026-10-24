//! Owned adapter inputs and preparation-time panel capture through native actions.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    document_card_support::browser::open,
    markdown_support::{Surface, check, js},
    tool_renderer_browser as drawing,
};
use maestro_app::{
    JsonObject,
    browser::tools::documents::{ArtifactsParams, LocalArtifactsPanel},
};
use maestro_web::{ArtifactsToolRenderer, ToolRenderer, tools::types::ToolRenderState};
use serde_json::{Value, json};
use std::rc::Rc;

/// Observe retained adapter data and the originally supplied panel.
/// # Panics
/// Panics if a retained value, action or release differs.
pub fn run() {
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let presentation = Rc::new(crate::presentation::Presentation::new(surface.host()));
    let panel = Rc::new(LocalArtifactsPanel::new_local(presentation.clone()));
    let filename = "literal <&> 雪.rs";
    crate::presentation::ready((panel.tool().execute)(
        "create".into(),
        json!({"command":"create", "filename":filename, "content":"before"}),
        None,
        None,
    ))
    .unwrap();
    presentation.opened.set(0);
    let mut renderer = ArtifactsToolRenderer::new(Some(panel.clone()));
    let recipes = retained_recipes(&renderer, filename);
    let replacement = Rc::new(crate::presentation::Presentation::new(surface.host()));
    renderer.artifacts_panel = Some(Rc::new(LocalArtifactsPanel::new_local(replacement.clone())));
    drop(renderer);
    for (index, recipe) in recipes.iter().enumerate() {
        panel.show_artifact("sentinel");
        let mut state = ToolRenderState::default();
        drawing::draw(&mut surface, recipe, &mut state);
        drawing::draw(&mut surface, recipe, &mut state);
        assert_eq!(
            js("document.querySelector('.maestro-artifact-pill').textContent")
                .as_string()
                .as_deref(),
            Some(filename)
        );
        open(&mut surface, recipe, &mut state);
        check("document.querySelector('.maestro-code code').textContent==='YWJj <&> 雪'");
        let _ = js("document.querySelector('.maestro-artifact-pill').click()");
        assert_eq!(panel.active_filename().as_deref(), Some(filename));
        assert_eq!(presentation.opened.get(), index + 1);
        assert_eq!(replacement.opened.get(), 0);
        let _ =
            js("globalThis.detachedArtifactPill=document.querySelector('.maestro-artifact-pill')");
        drop(state);
        let _ = js("detachedArtifactPill.click()");
        assert_eq!(presentation.opened.get(), index + 1);
    }
}

/// Prepare all adapter families before their borrowed source values are dropped.
fn retained_recipes(
    renderer: &ArtifactsToolRenderer,
    filename: &str,
) -> [maestro_web::tools::types::ToolRenderResult; 3] {
    let mut params = json!({"command":"create", "filename":filename, "content":"YWJj <&> 雪"});
    let object: JsonObject = serde_json::from_value(params.clone()).unwrap();
    let native: ArtifactsParams = serde_json::from_value(params.clone()).unwrap();
    let recipes = [
        <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
            renderer,
            Some(&params),
            None,
            false,
        )
        .unwrap(),
        <ArtifactsToolRenderer as ToolRenderer<JsonObject, ()>>::render(
            renderer,
            Some(&object),
            None,
            false,
        )
        .unwrap(),
        <ArtifactsToolRenderer as ToolRenderer<ArtifactsParams, ()>>::render(
            renderer,
            Some(&native),
            None,
            false,
        )
        .unwrap(),
    ];
    params["filename"] = "changed.rs".into();
    params["content"] = "changed".into();
    drop((params, object, native));
    recipes
}
