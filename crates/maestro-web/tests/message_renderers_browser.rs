//! Registered custom messages mounted as native page content.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{contributions_support, message_renderer_support, renderer_recipes};
use maestro_web::{dom::MountedContent, register_message_renderer, render_message};
use message_renderer_support::{NativeMessage, Renderer, message};
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};
use wasm_bindgen::prelude::wasm_bindgen;

/// Original callback data, unchanged native selection, replacement and release follow the placement.
#[wasm_bindgen]
pub async fn custom_message_content_reaches_the_native_reading_consumer(
    import_module: &js_sys::Function,
) {
    use contributions_support::{call, container, page_module, select, selection, text, view};
    let view = view();
    let page = page_module(import_module).await;
    let parent = container();
    let handle = Arc::new(AtomicU32::new(1));
    let input = message("browser-custom", handle.clone());
    register_message_renderer(
        "browser-custom",
        Rc::new(Renderer(Rc::new(|message| {
            let maestro_app::AgentMessage::Custom(value) = message else {
                unreachable!()
            };
            assert_eq!(value.read().unwrap().handle.load(Ordering::Relaxed), 42);
            Ok(Box::new(renderer_recipes::RichText(
                "**Native message** [link](https://example.test/)".into(),
            )))
        }))),
    );
    handle.store(42, Ordering::Relaxed);
    let mut slot = MountedContent::default();
    show(&mut slot, &input, &parent, &view);
    let root = slot.root().unwrap().clone();
    let strong = select(&root, "strong");
    assert_eq!(text(&strong), "Native message");
    assert_eq!(
        select(&root, "a").get_attribute("href").unwrap(),
        "https://example.test/"
    );
    call(&page, "select", &js_sys::Array::of1(&strong)).await;
    show(&mut slot, &input, &parent, &view);
    assert!(slot.root().unwrap().is_same_node(Some(&root)));
    assert!(select(&root, "strong").is_same_node(Some(&strong)));
    assert_eq!(
        selection(&page, &parent).await,
        ("Native message".into(), true)
    );
    replace_renderer(&mut slot, &input, (&parent, &view), &strong);
    slot.clear();
    assert!(!root.is_connected());
    assert!(
        parent
            .query_selector(".maestro-markdown, .maestro-reading-frame")
            .unwrap()
            .is_none()
    );
    parent.remove();
}

/// A replacement registration rewrites the parsed descendants inside the same placement.
fn replace_renderer(
    slot: &mut MountedContent,
    input: &maestro_app::AgentMessage<NativeMessage>,
    (parent, view): (&web_sys::Element, &maestro_web::dom::DomView),
    strong: &web_sys::Element,
) {
    use contributions_support::{select, text};
    let root = slot.root().unwrap().clone();
    register_message_renderer(
        "browser-custom",
        Rc::new(Renderer(Rc::new(|_| {
            Ok(Box::new(renderer_recipes::RichText(
                "**Replacement message**".into(),
            )))
        }))),
    );
    show(slot, input, parent, view);
    assert_eq!(text(&select(&root, "strong")), "Replacement message");
    assert!(
        !strong.is_connected(),
        "changed source replaces its parsed descendants"
    );
    assert!(
        slot.root().unwrap().is_same_node(Some(&root)),
        "the containing placement stays"
    );
}

/// Prepare through the public registry and update the same native placement.
fn show(
    slot: &mut MountedContent,
    input: &maestro_app::AgentMessage<NativeMessage>,
    parent: &web_sys::Element,
    view: &maestro_web::dom::DomView,
) {
    let content = render_message(input).unwrap().unwrap();
    slot.show(content.as_ref(), parent, view).unwrap();
}
