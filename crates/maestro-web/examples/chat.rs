//! A runnable browser-local chat: browser stores, the chat screen, saved sessions restored from
//! the address, a custom notification message and the header's appearance and settings controls.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
#[path = "chat/app.rs"]
mod app;
#[cfg(target_arch = "wasm32")]
#[path = "chat/custom_messages.rs"]
mod custom_messages;

/// Start the example in the page's `app` element once the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            web_sys::console::error_1(&error);
        }
    });
}
/// Build the page operands and stores, then run the example for the page's lifetime.
#[cfg(target_arch = "wasm32")]
async fn run() -> Result<(), wasm_bindgen::JsValue> {
    use std::rc::Rc;
    let failure =
        |error: &dyn std::fmt::Display| wasm_bindgen::JsValue::from_str(&error.to_string());
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("missing document")?;
    let root = document
        .get_element_by_id("app")
        .ok_or("missing app element")?;
    let i18n = maestro_web::I18n::new(Rc::new(maestro_web::BrowserLanguageHost))
        .map_err(|error| failure(&error))?;
    let pack = maestro_theme::BrandPack::parse(
        "brand.json",
        include_str!("../../../assets/brand/brand.json"),
    )
    .map_err(|error| failure(&error))?;
    let view = Rc::new(maestro_web::dom::DomView {
        document,
        i18n: Rc::new(i18n),
        highlighter: Rc::new(
            maestro_theme::SyntectHighlighter::new().map_err(|error| failure(&error))?,
        ),
        pack: Rc::new(pack),
        mode: maestro_theme::BrandMode::Dark,
        tool_renderers: Rc::default(),
    });
    let storage = app::storage("maestro-chat-example");
    let running = app::App::init(&view, storage, &root).await?;
    std::mem::forget(running);
    Ok(())
}
