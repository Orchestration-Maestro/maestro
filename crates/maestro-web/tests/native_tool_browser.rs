//! Native tool headers and cards through mounted page roots.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{Text, body, text, view};
use maestro_web::{
    dom::DomContent,
    tools::renderer_registry::{
        ToolHeaderState, ToolHeaderText,
        header::{ToolHeaderRefs, render_collapsible_header, render_header},
    },
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::HtmlElement;

mod artifact_owners;
mod optional;
mod registered;
mod sections;

/// Command, calculation, current-time and artifact cards, their filename pills and direct
/// differences replay every recorded branch through mounted page content; artifact cards own
/// their prepared inputs and keep their console across body changes.
#[wasm_bindgen]
pub fn native_optional_tools_artifacts_and_diff_replay_existing_corpus(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    optional::optional_tools(&view);
    optional::artifacts(&view);
    optional::transitions(&view);
    optional::diffs(&view);
    optional::pills(&view);
    artifact_owners::adapters_own_inputs(&view);
    artifact_owners::caption_without_pill(&view);
    artifact_owners::console_scroll(&view);
}

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

/// Activation follows both live references and native descendant bubbling exactly once, and the
/// keyboard reaches a panel pill and the disclosure as separate buttons.
#[wasm_bindgen]
pub async fn native_header_targets_and_pill_bubbling_follow_current_elements(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
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
    optional::created_artifact(&view);
    optional::keyboard_pills(&view).await;
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
    let errors: Vec<_> = all(element, "div.maestro-tool-error")
        .iter()
        .map(|error| text(error))
        .collect();
    assert_eq!(errors, expected.errors, "{index}");
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
    let retained = binding.renderer::<Rc<Cell<u32>>, Rc<Cell<u32>>>().unwrap();
    let expected: Rc<dyn ToolRenderer<Rc<Cell<u32>>, Rc<Cell<u32>>>> = renderer.clone();
    assert!(Rc::ptr_eq(&retained, &expected));
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
/// The built-ins prepare page content through the root dispatcher for both operand families, a
/// later replacement or removal survives initialization, and forced JSON keeps the registration.
#[wasm_bindgen]
pub fn native_defaults_register_once_and_keep_override_json_modes() {
    use maestro_web::{render_tool, set_show_json_mode};
    let view = view();
    let caption = |shown: ToolRenderResult| {
        let root = shown.content.mount(&view).unwrap();
        let caption = root.node.unchecked_ref::<web_sys::Element>();
        let caption = caption.query_selector(".maestro-tool-caption").unwrap();
        caption.map_or_else(|| text(&root.node), |caption| text(&caption))
    };
    let params = serde_json::json!({"command": "ls", "code": "1", "url": "u"});
    let members = params.as_object().unwrap();
    for (name, expected) in [
        ("bash", "Running command..."),
        ("javascript_repl", "Executing JavaScript"),
        ("extract_document", "Extracting document..."),
        ("unknown", "Tool Call"),
    ] {
        let object = Some(ToolParams::object(members));
        let shown =
            render_tool::<maestro_app::JsonObject, serde_json::Value>(name, object, None, false);
        assert_eq!(caption(shown.unwrap()), expected, "{name}");
        let json = Some(ToolParams::json(&params));
        let shown = render_tool::<serde_json::Value, serde_json::Value>(name, json, None, false);
        assert_eq!(caption(shown.unwrap()), expected, "{name}");
    }
    typed_default_captions(&caption);
    registered::recorded(&view);
    let (replacement, result) = native_binding(Rc::new(|| {}));
    let dispatch = || {
        let typed = Some(ToolParams::typed(&replacement.argument, |_| Ok(None)));
        caption(render_tool("bash", typed, Some(&result), false).unwrap())
    };
    let handle: Rc<dyn ToolRenderer<Rc<Cell<u32>>, Rc<Cell<u32>>>> = replacement.clone();
    maestro_web::register_tool_renderer("bash", ToolRendererBinding::from_raw(Rc::clone(&handle)));
    let registered = maestro_web::get_tool_renderer::<Rc<Cell<u32>>, Rc<Cell<u32>>>("bash");
    assert!(Rc::ptr_eq(&registered.unwrap(), &handle));
    assert_eq!(dispatch(), "selected");
    set_show_json_mode(true);
    let object = Some(ToolParams::object(members));
    let forced =
        render_tool::<maestro_app::JsonObject, serde_json::Value>("bash", object, None, false);
    assert_eq!(caption(forced.unwrap()), "Tool Call");
    set_show_json_mode(false);
    assert_eq!(
        dispatch(),
        "selected",
        "disabling JSON reveals the retained registration"
    );
    let removed = maestro_web::tools::renderer_registry::TOOL_RENDERERS
        .with(|all| all.borrow_mut().shift_remove("bash"));
    drop(removed);
    let object = Some(ToolParams::object(members));
    let shown =
        render_tool::<maestro_app::JsonObject, serde_json::Value>("bash", object, None, false);
    assert_eq!(
        caption(shown.unwrap()),
        "Tool Call",
        "removal is not undone by the defaults"
    );
    registered::artifacts(&view);
}
/// Typed application arguments use the same built-in preparations as raw model objects.
fn typed_default_captions(caption: &impl Fn(ToolRenderResult) -> String) {
    use maestro_app::browser::tools::{
        extract_document::{ExtractDocumentParams, ExtractDocumentResult},
        javascript_repl::{JavaScriptReplParams, JavaScriptReplToolResult},
    };
    use maestro_web::render_tool;
    let repl: JavaScriptReplParams =
        serde_json::from_value(serde_json::json!({"title": "", "code": "1"})).unwrap();
    let typed = Some(ToolParams::typed(&repl, |_| Ok(None)));
    let shown = render_tool::<_, JavaScriptReplToolResult>("javascript_repl", typed, None, false);
    assert_eq!(caption(shown.unwrap()), "Executing JavaScript");
    let extract: ExtractDocumentParams =
        serde_json::from_value(serde_json::json!({"url": "u"})).unwrap();
    let typed = Some(ToolParams::typed(&extract, |_| Ok(None)));
    let shown = render_tool::<_, ExtractDocumentResult>("extract_document", typed, None, false);
    assert_eq!(caption(shown.unwrap()), "Extracting document...");
}

/// A caller child whose native state only its own root holds.
struct CapturedField;
impl DomContent for CapturedField {
    fn mount(
        &self,
        view: &maestro_web::dom::DomView,
    ) -> Result<maestro_web::dom::DomRoot, ToolUiError> {
        Ok(maestro_web::dom::DomRoot::new(
            view.document.create_element("input").unwrap(),
        ))
    }
    fn update(
        &self,
        _: &maestro_web::dom::DomRoot,
        _: &maestro_web::dom::DomView,
    ) -> Result<(), ToolUiError> {
        Ok(())
    }
}

/// A section keeps its captured child's original root while collapsed, expanded and toggled
/// again; its summary stays literal, reconnection reapplies the current default, instances
/// stay independent, the recorded moments replay and dropping it releases the child.
#[wasm_bindgen]
pub async fn expandable_section_dom_keeps_captured_child_across_toggles() {
    use maestro_web::ExpandableSection;
    use web_sys::HtmlInputElement;
    let view = view();
    let mut section = ExpandableSection::new(vec![Box::new(CapturedField)]);
    section.summary = "Details <b>".into();
    let root = section.mount(&view).unwrap();
    body().append_child(&root.node).unwrap();
    let element: &web_sys::Element = root.node.unchecked_ref();
    let field = || {
        element
            .query_selector("input")
            .unwrap()
            .map(wasm_bindgen::JsCast::unchecked_into::<HtmlInputElement>)
    };
    let toggle: HtmlElement =
        crate::contributions_support::select(element, "button").unchecked_into();
    assert_eq!(text(&toggle), "Details <b>");
    assert!(field().is_none(), "collapsed by default");
    toggle.click();
    let input = field().unwrap();
    input.set_value("edited");
    toggle.click();
    assert!(field().is_none());
    toggle.focus().unwrap();
    crate::contributions_support::driver("driverPress", "Enter").await;
    let again = field().unwrap();
    assert!(
        again.is_same_node(Some(&input)),
        "the captured child is reinserted"
    );
    assert_eq!(again.value(), "edited");
    section.summary = "Renamed".into();
    section.update(&root, &view).unwrap();
    assert_eq!(text(&toggle), "Renamed");
    assert!(
        field().unwrap().is_same_node(Some(&input)),
        "update keeps expansion and child"
    );
    section.connected_callback(&root).unwrap();
    assert!(
        field().is_none(),
        "reconnection reapplies the collapsed default"
    );
    section.default_expanded = true;
    section.connected_callback(&root).unwrap();
    assert!(field().unwrap().is_same_node(Some(&input)));
    independent_section(&view, element, &input, &toggle);
    sections::recorded(&view);
    sections::release(&view);
}
/// A separately mounted section keeps its own original child and expansion.
fn independent_section(
    view: &maestro_web::dom::DomView,
    element: &web_sys::Element,
    input: &web_sys::HtmlInputElement,
    toggle: &HtmlElement,
) {
    let mut other = maestro_web::ExpandableSection::new(vec![Box::new(CapturedField)]);
    other.default_expanded = true;
    let other_root = other.mount(view).unwrap();
    body().append_child(&other_root.node).unwrap();
    let other_element: &web_sys::Element = other_root.node.unchecked_ref();
    let other_input = other_element.query_selector("input").unwrap().unwrap();
    assert!(
        !other_input.is_same_node(Some(input)),
        "instances capture their own children"
    );
    toggle.click();
    assert!(element.query_selector("input").unwrap().is_none());
    assert!(other_element.query_selector("input").unwrap().is_some());
    element.remove();
    other_element.remove();
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
    let names = driver("driverAria", ".maestro-tool-toggle")
        .await
        .as_string()
        .unwrap();
    assert!(names.contains("button \"<literal>\""), "{names}");
    crate::contributions_support::disclosure(header)
        .unwrap()
        .focus()
        .unwrap();
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
    let disclosure = crate::contributions_support::disclosure(header).unwrap();
    let caption = header
        .query_selector(".maestro-tool-caption")
        .unwrap()
        .unwrap();
    assert!(!caption.id().is_empty());
    assert_eq!(
        disclosure.get_attribute("aria-labelledby"),
        Some(caption.id())
    );
    assert!(!disclosure.has_attribute("aria-expanded"));
    assert_eq!(header.tag_name(), "DIV");
    assert_eq!(
        header.query_selector_all("button").unwrap().length(),
        1,
        "the row is not itself a control and holds only the disclosure"
    );
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

/// One corrected document-tool record projected onto the fields native cards consume.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentCase {
    /// Recorded tool family.
    tool: String,
    /// Original argument, result and streaming inputs.
    input: crate::support::tool_input::Input,
    /// Visible branches, or absence when the reached metadata is malformed.
    expected: Option<crate::document_card_support::Expected>,
}
/// Prepare one recorded document card for page content.
fn document_card(
    case: &DocumentCase,
) -> Result<ToolRenderResult<Box<dyn DomContent>>, ToolUiError> {
    use maestro_web::tools::{
        extract_document::ExtractDocumentRenderer, javascript_repl::JavaScriptReplRenderer,
    };
    let (params, streaming) = (case.input.params.as_ref(), case.input.is_streaming);
    let result = case.input.result.as_ref().map(Into::into);
    match case.tool.as_str() {
        "repl" => JavaScriptReplRenderer.render(params, result, streaming),
        "extract" => ExtractDocumentRenderer.render(params, result, streaming),
        other => panic!("unknown document tool {other}"),
    }
}
/// Every recorded execution and extraction branch keeps captions, literal listings, output,
/// URLs and returned files.
#[wasm_bindgen]
pub fn document_tools_dom_preserve_captions_code_output_and_files(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _elements = crate::contributions_support::host(&view, import_module);
    let cases: Vec<DocumentCase> =
        serde_json::from_str(include_str!("fixtures/native_document_cards.json")).unwrap();
    for (tool, count) in [("repl", 60), ("extract", 82)] {
        assert_eq!(cases.iter().filter(|case| case.tool == tool).count(), count);
    }
    for (index, case) in cases.iter().enumerate() {
        let shown = document_card(case);
        let Some(expected) = &case.expected else {
            assert!(
                matches!(shown, Err(ToolUiError::Presentation(_))),
                "{index}"
            );
            continue;
        };
        let shown = shown.unwrap();
        assert!(!shown.is_custom, "{index}");
        let root = shown.content.mount(&view).unwrap();
        body().append_child(&root.node).unwrap();
        observe_document_card(root.node.unchecked_ref(), expected, index);
        root.node.unchecked_ref::<web_sys::Element>().remove();
    }
}
/// Compare a mounted document card with its recorded visible fields.
fn observe_document_card(
    card: &web_sys::Element,
    expected: &crate::document_card_support::Expected,
    index: usize,
) {
    use crate::contributions_support::select;
    let header = card.first_element_child().unwrap();
    assert_eq!(
        crate::contributions_support::disclosure(&header).is_some(),
        expected.collapsible,
        "{index}"
    );
    assert_eq!(
        text(&select(&header, ".maestro-tool-caption")),
        expected.caption,
        "{index}"
    );
    let count = |selector: &str| card.query_selector_all(selector).unwrap().length() as usize;
    assert_eq!(count(".maestro-tool-spinner"), expected.spinner, "{index}");
    for status in ["complete", "error"] {
        assert_eq!(
            count(&format!(".maestro-tool-{status}")),
            usize::from(expected.status == status),
            "{index}"
        );
    }
    let url = card
        .query_selector(".maestro-tool-body > .maestro-tool-url")
        .unwrap();
    assert_eq!(
        url.map(|url| text(&url)),
        expected.url.as_ref().map(|url| format!("URL: {url}")),
        "{index}"
    );
    let listings: Vec<_> = all(card, ".maestro-tool-body > .maestro-code")
        .iter()
        .map(|shown| {
            let language = shown.get_attribute("language").unwrap();
            (text(&select(shown, "pre code")), language)
        })
        .collect();
    let blocks: Vec<_> = (expected.blocks.iter())
        .map(|block| (block.code.clone(), block.language.clone()))
        .collect();
    assert_eq!(listings, blocks, "{index}");
    let consoles: Vec<_> = all(card, ".maestro-tool-body > .maestro-console")
        .iter()
        .map(|shown| {
            let error = shown.query_selector(".maestro-console-error").unwrap();
            (text(&select(shown, "pre")), error.is_some())
        })
        .collect();
    let outputs: Vec<_> = (expected.console.iter())
        .map(|console| (console.content.clone(), console.variant == "error"))
        .collect();
    assert_eq!(consoles, outputs, "{index}");
    observe_files(card, &expected.attachments, index);
}
/// Every element below `root` matching `selector`, in document order.
fn all(root: &web_sys::Element, selector: &str) -> Vec<web_sys::Element> {
    let list = root.query_selector_all(selector).unwrap();
    (0..list.length())
        .map(|position| list.item(position).unwrap().unchecked_into())
        .collect()
}
/// Tiles sit outside the body, in order and with their names and image previews.
fn observe_files(
    card: &web_sys::Element,
    expected: &[crate::document_card_support::DownloadExpectation],
    index: usize,
) {
    assert!(
        card.query_selector(".maestro-tool-body .maestro-attachment-tile")
            .unwrap()
            .is_none()
    );
    let tiles = all(
        card,
        ":scope > .maestro-tool-files > .maestro-attachment-tile",
    );
    assert_eq!(tiles.len(), expected.len(), "{index}");
    for (tile, file) in tiles.iter().zip(expected) {
        let open = crate::contributions_support::select(tile, ".maestro-tile-open");
        assert_eq!(
            open.get_attribute("title").unwrap(),
            file.file_name,
            "{index}"
        );
        let image = open.query_selector("img").unwrap();
        assert_eq!(image.is_some(), file.preview.is_some(), "{index}");
        if let (Some(image), Some(preview)) = (image, &file.preview) {
            assert!(
                image
                    .get_attribute("src")
                    .unwrap()
                    .ends_with(preview.as_str()),
                "{index}"
            );
        }
    }
}

/// A rendered execution card for page content.
fn execution_card(
    code: &str,
    result: Option<&maestro_app::ToolResultMessage<serde_json::Value>>,
) -> Box<dyn DomContent> {
    use maestro_web::tools::javascript_repl::JavaScriptReplRenderer;
    let params = serde_json::json!({"title": "Run", "code": code});
    let shown: ToolRenderResult<Box<dyn DomContent>> = JavaScriptReplRenderer
        .render(params.as_object(), result.map(Into::into), true)
        .unwrap();
    shown.content
}
/// Pending expansion, motion, same-branch updates, branch replacement, independent instances
/// and released roots each keep the disclosure's children where the template keeps them.
#[wasm_bindgen]
pub async fn tool_disclosure_dom_retains_children_across_toggles_and_retargets(
    import_module: &js_sys::Function,
) {
    use crate::contributions_support::{container, driver, host, page_module, select};
    use maestro_web::dom::MountedContent;
    let view = view();
    let _elements = host(&view, import_module);
    let page = page_module(import_module).await;
    let parent = container();
    let mut slot = MountedContent::default();
    slot.show(execution_card("a", None).as_ref(), &parent, &view)
        .unwrap();
    let card: web_sys::Element = slot.root().unwrap().clone().unchecked_into();
    let header: HtmlElement = card.first_element_child().unwrap().unchecked_into();
    let pending = select(&card, ".maestro-tool-body");
    assert_collapsed(&header, &pending);
    let toggle = crate::contributions_support::disclosure(&header).unwrap();
    toggle.focus().unwrap();
    driver("driverPress", "Enter").await;
    assert!(toggle.is_same_node(view.document.active_element().as_deref()));
    assert_motion(&page, &header, &pending).await;
    let code = select(&pending, ".maestro-code");
    slot.show(execution_card("b", None).as_ref(), &parent, &view)
        .unwrap();
    assert!(select(&card, ".maestro-tool-body").is_same_node(Some(&pending)));
    assert!(select(&pending, ".maestro-code").is_same_node(Some(&code)));
    assert_eq!(text(&select(&code, "pre code")), "b");
    assert!(
        pending.class_list().contains("max-h-[2000px]"),
        "same template keeps expansion"
    );
    let files = serde_json::json!({"files": [{"fileName": "a.txt", "mimeType": "text/plain", "contentBase64": "YQ==", "size": 1}]});
    let result = maestro_app::ToolResultMessage {
        details: Some(files),
        ..crate::contributions_support::result("out", false)
    };
    slot.show(execution_card("b", Some(&result)).as_ref(), &parent, &view)
        .unwrap();
    let finished = select(&card, ".maestro-tool-body");
    assert!(
        !finished.is_same_node(Some(&pending)),
        "a result is a new template"
    );
    assert!(!select(&finished, ".maestro-code").is_same_node(Some(&code)));
    assert_collapsed(&header_of(&card), &finished);
    assert_result_branch_retains(&page, (&mut slot, &parent), &view, &result).await;
    assert_instances_and_release(slot, &card, &parent, &view);
}
/// The card's current header.
fn header_of(card: &web_sys::Element) -> HtmlElement {
    card.first_element_child().unwrap().unchecked_into()
}
/// A collapsed body paints no height and shows the expand chevron.
fn assert_collapsed(header: &HtmlElement, body: &web_sys::Element) {
    use crate::contributions_support::{computed, select};
    assert!(body.class_list().contains("max-h-0"));
    assert!(body.get_bounding_client_rect().height() < 0.5);
    assert_eq!(computed(&select(header, ".chevron-up"), "display"), "none");
    assert_ne!(
        computed(&select(header, ".chevrons-up-down"), "display"),
        "none"
    );
}
/// Pause `body`'s height transition at `time` milliseconds, returning its duration when one runs.
async fn seek_height(
    page: &wasm_bindgen::JsValue,
    body: &web_sys::Element,
    time: f64,
) -> Option<f64> {
    let arguments = js_sys::Array::of3(body, &"max-height".into(), &time.into());
    crate::contributions_support::call(page, "seek", &arguments)
        .await
        .as_f64()
}
/// Expansion animates over 300 ms, reverses from its partial height and settles collapsed.
async fn assert_motion(
    page: &wasm_bindgen::JsValue,
    header: &HtmlElement,
    body: &web_sys::Element,
) {
    use crate::contributions_support::{computed, select};
    let seek = |time| seek_height(page, body, time);
    assert!(body.class_list().contains("max-h-[2000px]") && body.class_list().contains("mt-3"));
    assert_ne!(computed(&select(header, ".chevron-up"), "display"), "none");
    assert_eq!(seek(10.).await, Some(300.));
    let partial = body.get_bounding_client_rect().height();
    assert!(
        partial > 0. && partial < f64::from(body.scroll_height()),
        "{partial}"
    );
    header.click();
    assert!(seek(0.).await.is_some());
    let reversed = body.get_bounding_client_rect().height();
    assert!(
        (reversed - partial).abs() < 1.,
        "collapse starts from the partial height: {reversed} {partial}"
    );
    seek(300.).await;
    assert_collapsed(header, body);
    header.click();
    seek(300.).await;
    let height = body.get_bounding_client_rect().height();
    assert!(
        (height - f64::from(body.scroll_height())).abs() < 1.,
        "{height}"
    );
}
/// Within the result template, listing, console, copy feedback and unchanged tiles stay mounted.
async fn assert_result_branch_retains(
    page: &wasm_bindgen::JsValue,
    (slot, parent): (&mut maestro_web::dom::MountedContent, &web_sys::Element),
    view: &maestro_web::dom::DomView,
    result: &maestro_app::ToolResultMessage<serde_json::Value>,
) {
    use crate::contributions_support::{Clipboard, Recorded, select, until};
    let card = slot.root().unwrap().clone();
    let body = select(&card, ".maestro-tool-body");
    let tile = select(
        &card,
        ":scope > .maestro-tool-files > .maestro-attachment-tile",
    );
    assert!(
        tile.get_bounding_client_rect().height() > 0.,
        "files stay outside the collapsed body"
    );
    let (code, console) = (
        select(&body, ".maestro-code"),
        select(&body, ".maestro-console"),
    );
    let clipboard = Recorded::install(Clipboard::Accept);
    let control = select(&console, "copy-button");
    select(&control, "button")
        .unchecked_into::<HtmlElement>()
        .click();
    until(page, &control, || text(&control).contains("Copied!")).await;
    assert_eq!(clipboard.texts(), ["out"]);
    slot.show(execution_card("c", Some(result)).as_ref(), parent, view)
        .unwrap();
    assert!(select(&card, ".maestro-tool-body").is_same_node(Some(&body)));
    assert!(select(&body, ".maestro-code").is_same_node(Some(&code)));
    assert_eq!(text(&select(&code, "pre code")), "c");
    assert!(select(&body, ".maestro-console").is_same_node(Some(&console)));
    assert!(
        text(&control).contains("Copied!"),
        "copy feedback survives the update"
    );
    assert!(select(&card, ".maestro-attachment-tile").is_same_node(Some(&tile)));
    let without = crate::contributions_support::result("out", false);
    slot.show(execution_card("c", Some(&without)).as_ref(), parent, view)
        .unwrap();
    assert!(
        card.unchecked_ref::<web_sys::Element>()
            .query_selector(".maestro-tool-files")
            .unwrap()
            .is_none()
    );
}
/// Instances toggle independently; a reconnected root keeps its activation and a released one
/// loses it.
fn assert_instances_and_release(
    slot: maestro_web::dom::MountedContent,
    card: &web_sys::Element,
    parent: &web_sys::Element,
    view: &maestro_web::dom::DomView,
) {
    use crate::contributions_support::select;
    let mut other = maestro_web::dom::MountedContent::default();
    other
        .show(execution_card("z", None).as_ref(), parent, view)
        .unwrap();
    let other_body = select(other.root().unwrap(), ".maestro-tool-body");
    let body = select(card, ".maestro-tool-body");
    header_of(card).click();
    assert!(!body.class_list().contains("max-h-0"));
    assert!(other_body.class_list().contains("max-h-0"));
    card.remove();
    parent.append_child(card).unwrap();
    header_of(card).click();
    assert!(
        body.class_list().contains("max-h-0"),
        "reconnected root keeps its activation"
    );
    drop(slot);
    header_of(card).click();
    assert!(
        body.class_list().contains("max-h-0"),
        "released root no longer toggles"
    );
    header_of(other.root().unwrap().unchecked_ref()).click();
    assert!(!other_body.class_list().contains("max-h-0"));
    card.remove();
    parent.remove();
}
