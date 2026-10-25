//! Markdown mounted as native page content through its public component.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    call, computed, container, document, host, page_module, rgb, selection, view,
};
use crate::markdown_support::{
    BlockRow, ContextRow, rows,
    walk::{self, Mounter as _},
};
use maestro_web::{
    components::markdown_block::MarkdownBlock,
    contributions::ContributionHost,
    dom::{DomView, MountedContent},
};
use std::{any::Any, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::Element;

/// Markdown with a paragraph and one listing.
const WITH_CODE: &str = "Findable **first** paragraph\n\n```rust\nlet kept = 1;\n```";

/// Native content placed in one container; dropping it removes its root.
struct Placed(MountedContent);
impl Drop for Placed {
    fn drop(&mut self) {
        if let Some(root) = self.0.root() {
            root.parent_node().unwrap().remove_child(root).unwrap();
        }
    }
}
/// The page-content owner as a fixture walk mounter.
struct PageMounter {
    /// Page operands.
    view: Rc<DomView>,
    /// Container of the shown document.
    parent: Element,
}
impl walk::Mounter for PageMounter {
    fn mount(&mut self, content: &str, (escape_html, thinking): (bool, bool)) -> Box<dyn Any> {
        let mut block = MarkdownBlock::default();
        content.clone_into(&mut block.content);
        block.escape_html = escape_html;
        block.is_thinking = thinking;
        let mut placed = Placed(MountedContent::default());
        placed.0.show(&block, &self.parent, &self.view).unwrap();
        Box::new(placed)
    }
}
/// Markdown content with `content`.
fn markdown(content: &str) -> MarkdownBlock {
    let mut block = MarkdownBlock::default();
    content.clone_into(&mut block.content);
    block
}
/// Page content shown in one container with its page operands.
struct Fixture {
    /// Page operands.
    view: Rc<DomView>,
    /// Container of the root.
    parent: Element,
    /// The placed root.
    shown: MountedContent,
}
impl Fixture {
    /// An empty fixture whose built-in elements are defined.
    fn new(import_module: &js_sys::Function) -> (Self, ContributionHost) {
        let view = view();
        let host = host(&view, import_module);
        let fixture = Self {
            view,
            parent: container(),
            shown: MountedContent::default(),
        };
        (fixture, host)
    }
    /// Show `block` with the fixture's operands.
    fn show(&mut self, block: &MarkdownBlock) {
        self.shown.show(block, &self.parent, &self.view).unwrap();
    }
    /// The mounted root.
    fn root(&self) -> Element {
        self.shown.root().cloned().unwrap().unchecked_into()
    }
}
/// Select every child of `node`, returning the selected text.
async fn select(page: &JsValue, node: &Element) -> String {
    let selected = call(page, "select", &js_sys::Array::of1(node)).await;
    selected.as_string().unwrap()
}
/// The selected pack's color for `role`, as computed CSS spells it.
fn color(view: &DomView, role: &str) -> String {
    rgb(view.pack.presentation(view.mode).unwrap().colors[role])
}

/// Defaults are exact, an empty document adds no text, and updates keep the root and unchanged nodes.
#[wasm_bindgen]
pub async fn markdown_dom_defaults_and_unchanged_roots(import_module: js_sys::Function) {
    let page = page_module(&import_module).await;
    let (mut fixture, _host) = Fixture::new(&import_module);
    let block = MarkdownBlock::default();
    assert!(block.content.is_empty() && !block.is_thinking && block.escape_html);
    fixture.show(&block);
    let mounted = fixture.root();
    assert_eq!(
        (mounted.local_name(), mounted.class_name()),
        ("div".to_owned(), "maestro-markdown".to_owned())
    );
    assert_eq!(mounted.child_nodes().length(), 0);
    assert!(document().query_selector("canvas").unwrap().is_none());
    let paragraph = unchanged_update_keeps_nodes(&mut fixture, &page).await;
    fixture.show(&markdown("Changed *second* text"));
    assert_eq!(fixture.root(), mounted);
    assert!(!mounted.contains(Some(&paragraph)));
    assert_eq!(mounted.text_content().unwrap(), "Changed second text\n");
    let found = call(&page, "find", &js_sys::Array::of1(&"second text".into())).await;
    assert_eq!(found.as_bool(), Some(true));
    assert!(selection(&page, &mounted).await.1);
}
/// Publish a document, then require an unchanged update to keep its nodes and selection.
async fn unchanged_update_keeps_nodes(fixture: &mut Fixture, page: &JsValue) -> Element {
    let mounted = fixture.root();
    let block = markdown(WITH_CODE);
    fixture.show(&block);
    assert_eq!(fixture.root(), mounted);
    let paragraph = mounted.first_element_child().unwrap();
    let listing = mounted.query_selector(".maestro-code").unwrap();
    assert_eq!(select(page, &paragraph).await, "Findable first paragraph");
    fixture.show(&block);
    assert_eq!(mounted.first_element_child(), Some(paragraph.clone()));
    assert_eq!(mounted.query_selector(".maestro-code").unwrap(), listing);
    assert_eq!(
        selection(page, &mounted).await,
        ("Findable first paragraph".to_owned(), true)
    );
    paragraph
}

/// The qualified document, contextual and caller corpora replay through page content.
#[wasm_bindgen]
pub async fn markdown_dom_replays_qualified_document_rows(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    walk::install(&import_module).await;
    let mut mounter = PageMounter {
        view,
        parent: container(),
    };
    let blocks = rows(
        include_str!("fixtures/markdown/blocks.json"),
        |_: &BlockRow| true,
    );
    assert_eq!(blocks.len(), 86);
    walk::rows_match(&mut mounter, blocks);
    let contexts = rows(
        include_str!("fixtures/markdown/contextual.json"),
        |_: &ContextRow| true,
    );
    assert_eq!(contexts.len(), 71);
    walk::contexts_match(&mut mounter, contexts);
    walk::caller_replay_matches(&mut mounter);
    let source = "```\nplain\n```\n\n```nosuchlang\nx\n```\n\n```html\n<script>&amp;\"'</script> 雪\n```\n\n```html\n<script>&amp;\"'</script> 雪\n```";
    let shown = mounter.mount(source, (true, false));
    let labels = walk::root()
        .query_selector_all(".maestro-code-label")
        .unwrap();
    let copies = walk::root().query_selector_all("copy-button").unwrap();
    let listed: Vec<_> = (0..copies.length())
        .map(|index| {
            let label = labels.item(index).unwrap().text_content().unwrap();
            let copy: Element = copies.item(index).unwrap().unchecked_into();
            (label, copy.get_attribute("text").unwrap())
        })
        .collect();
    let script = "<script>&amp;\"'</script> 雪\n".to_owned();
    assert_eq!(
        listed,
        [
            ("text".to_owned(), "plain\n".to_owned()),
            ("nosuchlang".to_owned(), "x\n".to_owned()),
            ("html".to_owned(), script.clone()),
            ("html".to_owned(), script),
        ]
    );
    drop(shown);
}

/// Presentation changes keep the listing, its copy feedback and the selection, and change only text roles.
#[wasm_bindgen]
pub async fn markdown_dom_keeps_code_controls_across_presentation_changes(
    import_module: js_sys::Function,
) {
    let page = page_module(&import_module).await;
    let (mut fixture, _host) = Fixture::new(&import_module);
    let mut block = markdown(WITH_CODE);
    fixture.show(&block);
    let mounted = fixture.root();
    let paragraph = mounted.first_element_child().unwrap();
    assert_eq!(computed(&paragraph, "color"), color(&fixture.view, "text"));
    assert_eq!(computed(&paragraph, "font-style"), "normal");
    let copy = copied(&page, &mounted).await;
    let code = mounted.query_selector("code.hljs").unwrap().unwrap();
    let selected = select(&page, &code).await;
    assert!(selected.starts_with("let kept = 1;"));
    block.is_thinking = true;
    fixture.show(&block);
    assert_eq!(
        mounted.query_selector("copy-button").unwrap(),
        Some(copy.clone())
    );
    assert_eq!(mounted.query_selector("code.hljs").unwrap(), Some(code));
    assert!(copy.text_content().unwrap().contains("Copied!"));
    assert_eq!(selection(&page, &mounted).await, (selected, true));
    assert_eq!(computed(&paragraph, "color"), color(&fixture.view, "muted"));
    assert_eq!(computed(&paragraph, "font-style"), "italic");
    supplied_adapters_reach_updates(&page, &mut fixture, &block, &copy).await;
}
/// Click the document's copy control with a succeeding clipboard and wait for its feedback.
async fn copied(page: &JsValue, mounted: &Element) -> Element {
    let copy = mounted.query_selector("copy-button").unwrap().unwrap();
    click_copy(page, &copy).await;
    assert!(copy.text_content().unwrap().contains("Copied!"));
    copy
}
/// Click `copy` with a succeeding clipboard and wait until it shows success feedback.
async fn click_copy(page: &JsValue, copy: &Element) {
    let write = Closure::<dyn Fn(String) -> js_sys::Promise>::new(|_: String| {
        js_sys::Promise::resolve(&JsValue::UNDEFINED)
    });
    let navigator = web_sys::window().unwrap().navigator();
    let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into()).unwrap();
    js_sys::Reflect::set(&clipboard, &"writeText".into(), write.as_ref()).unwrap();
    write.forget();
    let control: web_sys::HtmlElement = copy
        .query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into();
    control.click();
    crate::contributions_support::until(page, copy, || {
        copy.query_selector("span").unwrap().is_some()
    })
    .await;
}
/// A supplied language reaches the copy title and feedback; a supplied failing engine fails without publishing.
async fn supplied_adapters_reach_updates(
    page: &JsValue,
    fixture: &mut Fixture,
    block: &MarkdownBlock,
    copy: &Element,
) {
    let view = Rc::clone(&fixture.view);
    fixture.view = crate::contributions_support::localized_view(&view, "de");
    fixture.show(block);
    assert_eq!(
        copy.get_attribute("title").as_deref(),
        Some("Code kopieren")
    );
    click_copy(page, copy).await;
    assert!(copy.text_content().unwrap().contains("Kopiert!"));
    let failing = DomView {
        highlighter: Rc::new(crate::failing_highlighter::FailingEngine(Arc::new(()))),
        ..(*view).clone()
    };
    let mounted = fixture.root();
    let published = mounted.inner_html();
    let (parent, shown) = (&fixture.parent, &mut fixture.shown);
    assert!(shown.show(block, parent, &failing).is_err());
    assert!(
        shown
            .show(&markdown("```js\nnew\n```"), parent, &failing)
            .is_err()
    );
    assert_eq!(mounted.inner_html(), published);
    fixture.view = view;
    fixture.show(block);
    assert_eq!(
        mounted.query_selector("copy-button").unwrap().as_ref(),
        Some(copy)
    );
    assert_eq!(copy.get_attribute("title").as_deref(), Some("Copy code"));
}
