//! Native tool headers and cards through mounted page roots.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{Text, body, text, view};
use maestro_web::{
    dom::DomContent,
    tools::renderer_registry::{
        ToolHeaderState, ToolHeaderText,
        header::dom::{ToolHeaderRefs, render_collapsible_header, render_header},
    },
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::HtmlElement;

/// Status colors, literal captions, rich children and spinner placement remain independent.
#[wasm_bindgen]
pub async fn tool_headers_dom_keep_state_icons_and_literal_captions() {
    let view = view();
    for state in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        for rich in [false, true] {
            let caption = if rich {
                ToolHeaderText::Content(Box::new(Text("<literal>")) as Box<dyn DomContent>)
            } else {
                ToolHeaderText::Text("<literal>".into())
            };
            let recipe = render_header(
                state,
                maestro_web::icons::icons().unwrap()["Code"].clone(),
                caption,
            );
            let root = recipe.mount(&view).unwrap();
            body().append_child(&root.node).unwrap();
            let element: &web_sys::Element = root.node.unchecked_ref();
            assert_eq!(text(element), "<literal>");
            assert!(element.get_bounding_client_rect().height() > 0.);
            assert_eq!(
                element
                    .query_selector_all(".maestro-tool-spinner")
                    .unwrap()
                    .length(),
                u32::from(state == ToolHeaderState::Inprogress)
            );
            assert_eq!(
                element
                    .query_selector(".maestro-tool-complete")
                    .unwrap()
                    .is_some(),
                state == ToolHeaderState::Complete
            );
            assert_eq!(
                element
                    .query_selector(".maestro-tool-error")
                    .unwrap()
                    .is_some(),
                state == ToolHeaderState::Error
            );
            element.remove();
        }
    }
    collapsible_headers(&view).await;
}

/// Activation follows both live references and native descendant bubbling exactly once.
#[wasm_bindgen]
pub fn native_header_targets_and_pill_bubbling_follow_current_elements() {
    let view = view();
    let target: HtmlElement = view
        .document
        .create_element("div")
        .unwrap()
        .unchecked_into();
    target.set_class_name("max-h-0");
    let refs = ToolHeaderRefs {
        content: Rc::new(RefCell::new(Some(target.clone()))),
        chevron: Rc::default(),
    };
    let recipe = render_collapsible_header(
        ToolHeaderState::Complete,
        maestro_web::icons::icons().unwrap()["Code"].clone(),
        ToolHeaderText::Content(Box::new(Text("rich"))),
        refs.clone(),
        false,
    );
    let root = recipe.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let header: &HtmlElement = root.node.unchecked_ref();
    header
        .first_element_child()
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .click();
    assert!(!target.class_list().contains("max-h-0"));
    assert!(target.class_list().contains("max-h-[2000px]"));
    let second: HtmlElement = view
        .document
        .create_element("div")
        .unwrap()
        .unchecked_into();
    second.set_class_name("max-h-0");
    refs.content.replace(Some(second.clone()));
    header.click();
    assert!(!second.class_list().contains("max-h-0"));
    assert!(!target.class_list().contains("max-h-0"));
    for (content, chevron) in [(false, true), (true, false), (false, false)] {
        let arrow = refs.chevron.borrow().clone();
        refs.content.replace(content.then(|| second.clone()));
        refs.chevron
            .replace(chevron.then(|| arrow.clone()).flatten());
        second.set_class_name("max-h-0");
        header.click();
        assert!(second.class_list().contains("max-h-0"));
        refs.chevron.replace(arrow);
    }
    missing_header_icons(header, &refs, &second);
    explicit_parent_and_bubbling_toggle_twice(header, &refs, &second);
    header.remove();
}

/// The command corpus keeps exact caption, console text, result status and card choice.
#[wasm_bindgen]
pub fn bash_dom_matches_all_command_result_branches() {
    use maestro_web::{
        BashRenderer,
        tools::types::{ToolRenderResult, ToolRenderer},
    };
    let view = view();
    let cases: Vec<BashCase> =
        serde_json::from_str(include_str!("fixtures/native_bash_cards.json")).unwrap();
    assert_eq!(cases.len(), 96);
    for (index, row) in cases.into_iter().enumerate() {
        assert!(row.input.language.is_none());
        let result: ToolRenderResult<Box<dyn DomContent>> = BashRenderer
            .render(
                row.input.params.as_ref(),
                row.input.result.as_ref().map(Into::into),
                row.input.is_streaming,
            )
            .unwrap();
        assert_eq!(result.is_custom, row.expected.is_custom, "{index}");
        let root = result.content.mount(&view).unwrap();
        body().append_child(&root.node).unwrap();
        let element: &web_sys::Element = root.node.unchecked_ref();
        observe_card(element, &row.expected, index);
        element.remove();
    }
}

/// Compare the mounted native card with the existing independent source fixture.
fn observe_card(
    element: &web_sys::Element,
    expected: &crate::tool_cards_support::Expected,
    index: usize,
) {
    assert!(expected.errors.is_empty());
    assert_eq!(
        text(
            &element
                .query_selector(".maestro-tool-caption")
                .unwrap()
                .unwrap()
        ),
        expected.header,
        "{index}"
    );
    assert_eq!(
        element
            .query_selector_all(".maestro-tool-spinner")
            .unwrap()
            .length() as usize,
        expected.spinner,
        "{index}"
    );
    let console = element.query_selector(".maestro-console").unwrap();
    assert_eq!(console.is_some(), !expected.console.is_empty(), "{index}");
    if let Some(console) = console {
        assert_eq!(
            text(&console.query_selector("pre").unwrap().unwrap()),
            expected.console[0].content,
            "{index}"
        );
        assert_eq!(
            console
                .query_selector(".maestro-console-error")
                .unwrap()
                .is_some(),
            expected.console[0].variant == "error",
            "{index}"
        );
    }
    assert_eq!(
        element
            .query_selector(".maestro-tool-error")
            .unwrap()
            .is_some(),
        expected.status == "error",
        "{index}"
    );
}

use maestro_web::tools::types::{
    ToolParams, ToolRenderResult, ToolRenderer, ToolRendererBinding, ToolResultMessageView,
    ToolUiError,
};
use std::cell::Cell;
/// Raw binding witness: both operands are non-serializable shared handles.
struct IdentityRenderer {
    /// Expected original argument.
    argument: Rc<Cell<u32>>,
    /// Expected original details.
    details: Rc<Cell<u32>>,
    /// Reentrant operation invoked after identity observations.
    action: Rc<dyn Fn()>,
}
impl ToolRenderer<Rc<Cell<u32>>, Rc<Cell<u32>>, Box<dyn DomContent>> for IdentityRenderer {
    fn render(
        &self,
        argument: Option<&Rc<Cell<u32>>>,
        result: Option<ToolResultMessageView<'_, Rc<Cell<u32>>>>,
        _: bool,
    ) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
        assert!(Rc::ptr_eq(argument.unwrap(), &self.argument));
        assert!(Rc::ptr_eq(
            result.as_ref().unwrap().details().unwrap(),
            &self.details
        ));
        (self.action)();
        Ok(ToolRenderResult {
            content: Box::new(Text("selected")),
            is_custom: true,
        })
    }
}
/// One raw binding setup with its original envelope and renderer handle.
fn native_binding(
    action: Rc<dyn Fn()>,
) -> (
    Rc<IdentityRenderer>,
    maestro_app::ToolResultMessage<Rc<Cell<u32>>>,
) {
    let details = Rc::new(Cell::new(7));
    let renderer = Rc::new(IdentityRenderer {
        argument: Rc::new(Cell::new(9)),
        details: Rc::clone(&details),
        action,
    });
    let result = maestro_app::ToolResultMessage {
        tool_call_id: "native".into(),
        tool_name: "native".into(),
        content: Vec::new(),
        timestamp: 1.,
        details: Some(details),
        is_error: false,
    };
    (renderer, result)
}
/// Raw slots preserve original native operands and bypass the default formatter.
#[wasm_bindgen]
pub fn native_binding_keeps_typed_json_and_details_agnostic_views() {
    let (renderer, result) = native_binding(Rc::new(|| {}));
    let binding = ToolRendererBinding::from_dom(renderer.clone());
    assert!(binding.renderer::<Rc<Cell<u32>>, Rc<Cell<u32>>>().is_none());
    maestro_web::register_tool_renderer("native-identity", binding);
    let shown = maestro_web::tools::dom::render_tool(
        "native-identity",
        Some(ToolParams::typed(&renderer.argument, |_| {
            panic!("raw dispatch formatted native input")
        })),
        Some(&result),
        true,
    )
    .unwrap();
    assert!(shown.is_custom);
    let root = shown.content.mount(&view()).unwrap();
    assert_eq!(text(&root.node), "selected");
    maestro_web::tools::renderer_registry::TOOL_RENDERERS
        .with(|all| all.borrow_mut().shift_remove("native-identity"));
}
/// Reentrant removal and replacement leave the already selected callback alive.
#[wasm_bindgen]
pub fn native_binding_reentrant_replacement_and_retained_handle() {
    use maestro_web::tools::renderer_registry::TOOL_RENDERERS;
    let (replacement, _) = native_binding(Rc::new(|| {}));
    let next = replacement.clone();
    let (original, result) = native_binding(Rc::new(move || {
        let removed = TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("native-reentrant"));
        assert!(removed.is_some());
        drop(removed);
        maestro_web::register_tool_renderer(
            "native-reentrant",
            ToolRendererBinding::from_dom(next.clone()),
        );
    }));
    let lifetime = Rc::downgrade(&original);
    maestro_web::register_tool_renderer(
        "native-reentrant",
        ToolRendererBinding::from_dom(original.clone()),
    );
    let retained = TOOL_RENDERERS.with(|all| all.borrow()["native-reentrant"].clone());
    let dispatch = || {
        maestro_web::tools::dom::render_tool(
            "native-reentrant",
            Some(ToolParams::typed(&original.argument, |_| Ok(None))),
            Some(&result),
            false,
        )
        .unwrap()
    };
    assert!(dispatch().is_custom);
    assert!(lifetime.upgrade().is_some());
    drop(original);
    assert!(
        lifetime.upgrade().is_some(),
        "retained binding still owns the old renderer"
    );
    drop(retained);
    assert!(lifetime.upgrade().is_none());
    let result = maestro_app::ToolResultMessage {
        details: Some(Rc::clone(&replacement.details)),
        ..result
    };
    assert!(
        maestro_web::tools::dom::render_tool(
            "native-reentrant",
            Some(ToolParams::typed(&replacement.argument, |_| Ok(None))),
            Some(&result),
            false
        )
        .unwrap()
        .is_custom
    );
    let removed = TOOL_RENDERERS.with(|all| all.borrow_mut().shift_remove("native-reentrant"));
    drop(removed);
}

/// The twelve rich/status/default-expansion branches keep their native button and current body.
async fn collapsible_headers(view: &maestro_web::dom::DomView) {
    for state in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        for rich in [false, true] {
            for expanded in [false, true] {
                collapsible_header(view, state, (rich, expanded)).await;
            }
        }
    }
}
/// Observe one native header branch with a body deliberately opposite to its default icon.
async fn collapsible_header(
    view: &maestro_web::dom::DomView,
    state: ToolHeaderState,
    options: (bool, bool),
) {
    use crate::contributions_support::{computed, driver, localized_view};
    let (rich, expanded) = options;
    let target: HtmlElement = view
        .document
        .create_element("div")
        .unwrap()
        .unchecked_into();
    target.set_class_name(if expanded {
        "max-h-0"
    } else {
        "max-h-[2000px] mt-3"
    });
    let refs = ToolHeaderRefs {
        content: Rc::new(RefCell::new(Some(target.clone()))),
        chevron: Rc::default(),
    };
    let caption = if rich {
        ToolHeaderText::Content(Box::new(Text("<literal>")) as Box<dyn DomContent>)
    } else {
        ToolHeaderText::Text("<literal>".into())
    };
    let recipe = render_collapsible_header(
        state,
        maestro_web::icons::icons().unwrap()["Code"].clone(),
        caption,
        refs.clone(),
        expanded,
    );
    let root = recipe.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let header: &HtmlElement = root.node.unchecked_ref();
    observe_header(header, state, expanded);
    assert_eq!(
        target.class_list().contains("max-h-0"),
        expanded,
        "default expansion never rewrites caller body"
    );
    header.focus().unwrap();
    driver("driverPress", "Enter").await;
    assert_eq!(target.class_list().contains("max-h-0"), !expanded);
    assert_eq!(computed(header, "outline-style"), "solid");
    recipe.update(&root, &localized_view(view, "de")).unwrap();
    assert_eq!(
        text(header),
        "<literal>",
        "header does not translate authored content"
    );
    header.remove();
}

/// Measure the reached native chevron and pending glyph placement.
fn observe_header(header: &HtmlElement, state: ToolHeaderState, expanded: bool) {
    use crate::contributions_support::computed;
    assert_eq!(header.tag_name(), "BUTTON");
    assert_eq!(text(header), "<literal>");
    assert_eq!(
        computed(
            &header.query_selector(".chevron-up").unwrap().unwrap(),
            "display"
        ) == "none",
        !expanded
    );
    let spinner = header.query_selector(".maestro-tool-spinner").unwrap();
    assert_eq!(spinner.is_some(), state == ToolHeaderState::Inprogress);
    if let Some(spinner) = spinner {
        assert!(
            spinner
                .next_element_sibling()
                .unwrap()
                .class_list()
                .contains("maestro-tool-icon")
        );
    }
}

/// Existing source observations projected only onto fields consumed by the native card.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BashCase {
    /// Original argument, result and streaming inputs.
    input: crate::support::tool_input::Input,
    /// Independently recorded visible branches and bytes.
    expected: crate::tool_cards_support::Expected,
}

/// Missing either or both glyphs does not suppress a reached body toggle.
fn missing_header_icons(header: &HtmlElement, refs: &ToolHeaderRefs, target: &HtmlElement) {
    let original = refs.chevron.borrow().clone().unwrap();
    refs.content.replace(Some(target.clone()));
    for selector in [".chevron-up", ".chevrons-up-down", "*"] {
        let arrow: HtmlElement = original
            .clone_node_with_deep(true)
            .unwrap()
            .unchecked_into();
        let missing = arrow.query_selector_all(selector).unwrap();
        for index in 0..missing.length() {
            missing
                .item(index)
                .unwrap()
                .unchecked_into::<web_sys::Element>()
                .remove();
        }
        let before = arrow.inner_html();
        refs.chevron.replace(Some(arrow.clone()));
        target.set_class_name("max-h-0");
        header.click();
        assert!(!target.class_list().contains("max-h-0"));
        assert_eq!(
            arrow.inner_html(),
            before,
            "partial chevrons stay unchanged"
        );
    }
    refs.chevron.replace(Some(original));
}
/// The rejected explicit-parent fallback plus native bubbling toggles twice, not once.
fn explicit_parent_and_bubbling_toggle_twice(
    header: &HtmlElement,
    refs: &ToolHeaderRefs,
    target: &HtmlElement,
) {
    use wasm_bindgen::closure::Closure;
    refs.content.replace(Some(target.clone()));
    let child = header.first_element_child().unwrap();
    let parent = header.clone();
    let fallback = Closure::<dyn Fn(web_sys::Event)>::new(move |_| parent.click());
    child
        .add_event_listener_with_callback("click", fallback.as_ref().unchecked_ref())
        .unwrap();
    target.set_class_name("max-h-0");
    child.unchecked_ref::<HtmlElement>().click();
    assert!(
        target.class_list().contains("max-h-0"),
        "fallback and bubbling toggle twice"
    );
    child
        .remove_event_listener_with_callback("click", fallback.as_ref().unchecked_ref())
        .unwrap();
    drop(fallback);
    child.unchecked_ref::<HtmlElement>().click();
    assert!(
        !target.class_list().contains("max-h-0"),
        "native bubbling alone toggles once"
    );
}
