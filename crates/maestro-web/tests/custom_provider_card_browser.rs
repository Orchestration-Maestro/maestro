//! Native provider-card status, actions and root lifetime.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{container, text, view};
use maestro_web::{CustomProvider, CustomProviderCard, dom::DomContent};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::Element;

mod fixture;
use fixture::{fixture, localized};

/// A provider retaining literal caller strings.
fn provider() -> Rc<RefCell<CustomProvider>> {
    Rc::new(RefCell::new(
        serde_json::from_str(
            r#"{
  "id": "original",
  "name": " <b>雪</b> ",
  "type": "ollama",
  "baseUrl": ""
}"#,
        )
        .unwrap(),
    ))
}
/// Constructor defaults and native manual status.
#[wasm_bindgen]
pub fn provider_card_defaults_and_status_branches() {
    let view = view();
    let provider = provider();
    let card = CustomProviderCard::new(Rc::clone(&provider));
    assert!(Rc::ptr_eq(&provider, &card.provider));
    assert!(!card.is_auto_discovery);
    assert!(card.status.is_none());
    assert!(card.on_refresh.is_none() && card.on_edit.is_none() && card.on_delete.is_none());
    let root = card.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let element: &Element = root.node.unchecked_ref();
    assert_eq!(
        text(
            &element
                .query_selector(".maestro-provider-status")
                .unwrap()
                .unwrap()
        ),
        "Models: 0"
    );
    parent.remove();
    let cases = fixture();
    assert_eq!(cases.rows.len(), 100);
    for row in cases.rows {
        let card = row.input.card();
        assert_eq!(
            row.input.model_case.as_deref(),
            Some(model_case(&card.provider.borrow().models)),
            "{}",
            row.id
        );
        assert_eq!(row.input.mask, None);
        card.update(&root, &row.input.view()).unwrap();
        assert_eq!(status(element), row.expected.text, "{}", row.id);
    }
    assert_eq!(cases.route_controls.len(), 14);
    for row in cases.route_controls {
        assert!(row.input.model_case.is_none() && row.input.mask.is_none());
        let root = row.input.card().mount(&row.input.view()).unwrap();
        assert_eq!(status(&root.node.clone().unchecked_into()), row.expected);
    }
}
/// Status text, with an omitted discovery row kept distinct from an empty node.
fn status(root: &Element) -> String {
    root.query_selector(".maestro-provider-status")
        .unwrap()
        .map_or_else(String::new, |node| text(&node))
}

/// Names, protocol spellings and URLs stay literal through native updates.
#[wasm_bindgen]
pub fn provider_card_labels_literal_fields_and_locales() {
    let view = localized("en");
    let mut card = CustomProviderCard::new(provider());
    let root = card.mount(&view).unwrap();
    let element: Element = root.node.clone().unchecked_into();
    let name = element.query_selector(".maestro-provider-name").unwrap();
    assert!(name.is_some(), "native provider name must be rendered");
    let name = name.unwrap();
    assert_eq!(text(&name), " <b>雪</b> ");
    assert_eq!(name.child_element_count(), 0);
    let details = element
        .query_selector(".maestro-provider-details")
        .unwrap()
        .unwrap();
    for url in [
        "",
        "https://server.test/v1",
        " <script>literal</script> ",
        " ",
    ] {
        card.provider.borrow_mut().base_url = url.into();
        card.update(&root, &view).unwrap();
        let expected = if url.is_empty() {
            "ollama".into()
        } else {
            format!("ollama • {url}")
        };
        assert_eq!(text(&details), expected);
        assert_eq!(details.query_selector_all("script").unwrap().length(), 0);
    }
    for control in fixture().route_controls.into_iter().step_by(2) {
        card.provider = control.input.card().provider;
        card.update(&root, &view).unwrap();
        let literal = serde_json::to_value(card.provider.borrow().r#type).unwrap();
        assert_eq!(
            text(
                &element
                    .query_selector(".maestro-provider-type")
                    .unwrap()
                    .unwrap()
            ),
            literal.as_str().unwrap()
        );
    }
    card.provider.borrow_mut().name = "next 雪 <img>".into();
    card.update(&root, &localized("de")).unwrap();
    assert_eq!(text(&name), "next 雪 <img>");
    assert_eq!(status(&element), "Modelle: 2");
    card.update(&root, &view).unwrap();
    assert_eq!(status(&element), "Models: 2");
}

/// Native controls below a mounted card, in activation order.
fn buttons(root: &Element) -> Vec<web_sys::HtmlElement> {
    let nodes = root.query_selector_all("button").unwrap();
    (0..nodes.length())
        .map(|index| nodes.item(index).unwrap().unchecked_into())
        .collect()
}
/// Each supplied callback is independent and receives the current original handle.
#[wasm_bindgen]
pub fn provider_card_actions_use_current_provider_and_callbacks() {
    let cases = fixture();
    assert_eq!(cases.callbacks.len(), 32);
    for row in cases.callbacks {
        assert!(row.input.model_case.is_none() && row.input.status.is_none());
        let mut card = row.input.card();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let identities = Rc::new(RefCell::new(Vec::new()));
        record_actions(&mut card, row.input.mask.unwrap(), &calls, &identities);
        let root = card.mount(&row.input.view()).unwrap();
        let parent = container();
        parent.append_child(&root.node).unwrap();
        let element: Element = root.node.clone().unchecked_into();
        let controls = buttons(&element);
        assert_eq!(
            controls.iter().map(|node| text(node)).collect::<Vec<_>>(),
            row.expected.labels
        );
        for button in controls {
            button.click();
        }
        assert_eq!(*calls.borrow(), row.expected.callbacks);
        assert_eq!(
            identities.borrow().iter().all(|same| *same),
            row.expected.provider_identity
        );
        parent.remove();
    }
    current_actions(&cases.updates);
}
/// A retained native listener reads replaced operands; removed callbacks revoke it.
fn current_actions(expected: &fixture::Updates) {
    let view = view();
    let mut card = CustomProviderCard::new(provider());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let first = Rc::clone(&calls);
    card.on_edit = Some(Rc::new(move |_| first.borrow_mut().push("old".into())));
    let root = card.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let element: Element = root.node.clone().unchecked_into();
    let button = buttons(&element).remove(0);
    card.provider = provider();
    card.provider.borrow_mut().id = "next".into();
    let next = Rc::clone(&card.provider);
    let output = Rc::clone(&calls);
    card.on_edit = Some(Rc::new(move |provider| {
        assert!(Rc::ptr_eq(&provider, &next));
        output
            .borrow_mut()
            .push(format!("new:{}", provider.borrow().id));
        provider.borrow_mut().name = "mutated".into();
    }));
    card.update(&root, &view).unwrap();
    assert!(button.is_same_node(buttons(&element).first().map(AsRef::as_ref)));
    button.click();
    assert_eq!(
        card.provider.borrow().name == "mutated",
        expected.mutated_original
    );
    assert_eq!(
        *calls.borrow() == expected.calls,
        expected.replaced_provider_and_callback
    );
    card.on_edit = None;
    card.update(&root, &view).unwrap();
    let before = calls.borrow().len();
    button.click();
    assert_eq!(
        calls.borrow().len() == before,
        expected.removed_callback_noop
    );
    parent.remove();
}

/// Stable roots retain focus/selection, revoke detached actions and release captures.
#[wasm_bindgen]
pub async fn provider_card_updates_reconnect_and_release_bindings(import: js_sys::Function) {
    use crate::contributions_support::{call, host, page_module, selection};
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let mut card = CustomProviderCard::new(provider());
    let observed = Rc::new(std::cell::Cell::new(0));
    let weak = Rc::downgrade(&observed);
    let counted = Rc::clone(&observed);
    card.on_edit = Some(Rc::new(move |provider| {
        counted.set(counted.get() + 1);
        provider.borrow_mut().name = "updated inside callback".into();
    }));
    let root = card.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let element: Element = root.node.clone().unchecked_into();
    let edit = buttons(&element).remove(0);
    edit.focus().unwrap();
    let name = element
        .query_selector(".maestro-provider-name")
        .unwrap()
        .unwrap();
    call(&page, "select", &js_sys::Array::of1(&name)).await;
    let selected = selection(&page, &element).await;
    card.update(&root, &view).unwrap();
    assert_eq!(view.document.active_element().unwrap(), edit.clone().into());
    assert_eq!(selection(&page, &element).await, selected);
    assert!(edit.is_same_node(buttons(&element).first().map(AsRef::as_ref)));
    edit.click();
    assert_eq!(observed.get(), 1);
    assert_eq!(card.provider.borrow().name, "updated inside callback");
    parent.remove_child(&root.node).unwrap();
    edit.click();
    assert_eq!(observed.get(), 1);
    parent.append_child(&root.node).unwrap();
    edit.click();
    assert_eq!(observed.get(), 2);
    independent_card(&parent, &view);
    assert_eq!(observed.get(), 2);
    action_updates(&mut card, &root, &view);
    failed_update(&card, &root, &view);
    drop(root);
    edit.click();
    assert_eq!(observed.get(), 2);
    drop(card);
    drop(observed);
    assert!(
        weak.upgrade().is_none(),
        "mounted callback must not retain its owner"
    );
    reentrant_update(&view);
    parent.remove();
}
/// A reached translation failure leaves all previously published labels and actions intact.
fn failed_update(
    card: &CustomProviderCard,
    root: &maestro_web::dom::DomRoot,
    view: &maestro_web::dom::DomView,
) {
    let bad = localized("en");
    bad.i18n.get_translations().borrow()["en"]
        .borrow_mut()
        .insert(
            "Delete".into(),
            maestro_web::TranslationValue::Category(Rc::default()),
        );
    let before = text(&root.node);
    let mut next = card.clone();
    next.provider = provider();
    next.provider.borrow_mut().name = "not published".into();
    next.on_delete = Some(Rc::new(|_| panic!("failed update published an action")));
    assert!(next.update(root, &bad).is_err());
    assert_eq!(text(&root.node), before);
    card.update(root, view).unwrap();
}
/// An action may replace its own callback through the containing current owner.
fn reentrant_update(view: &Rc<maestro_web::dom::DomView>) {
    let card = Rc::new(RefCell::new(CustomProviderCard::new(provider())));
    let mounted = Rc::new(RefCell::new(None::<maestro_web::dom::DomRoot>));
    let (current, target, view) = (
        Rc::downgrade(&card),
        Rc::downgrade(&mounted),
        Rc::clone(view),
    );
    card.borrow_mut().on_edit = Some(Rc::new(move |provider| {
        provider.borrow_mut().name = "reentrant".into();
        let card = current.upgrade().unwrap();
        card.borrow_mut().on_edit = None;
        let next = card.borrow().clone();
        next.update(target.upgrade().unwrap().borrow().as_ref().unwrap(), &view)
            .unwrap();
    }));
    let root = card
        .borrow()
        .mount(&crate::contributions_support::view())
        .unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let edit = buttons(&root.node.clone().unchecked_into()).remove(0);
    *mounted.borrow_mut() = Some(root);
    edit.click();
    assert_eq!(card.borrow().provider.borrow().name, "reentrant");
    assert!(
        buttons(
            &mounted
                .borrow()
                .as_ref()
                .unwrap()
                .node
                .clone()
                .unchecked_into()
        )
        .is_empty()
    );
    edit.click();
    parent.remove();
}

/// Native layout, supplied brand roles, selectable text and real keyboard actions.
#[wasm_bindgen]
pub async fn provider_card_native_focus_layout_and_brand_roles(import: js_sys::Function) {
    paint::check(import).await;
}

mod paint;

/// Observe the original missing/null/present model shape.
fn model_case(models: &maestro_app::Presence<Vec<maestro_app::Model>>) -> &'static str {
    match models {
        maestro_app::Presence::Missing => "missing",
        maestro_app::Presence::Null => "null",
        maestro_app::Presence::Present(models) => match models.len() {
            0 => "empty",
            1 => "one",
            2 => "two",
            _ => panic!("unexpected model count"),
        },
    }
}
/// Install the independent callbacks selected by the recorded mask.
fn record_actions(
    card: &mut CustomProviderCard,
    mask: u8,
    calls: &Rc<RefCell<Vec<fixture::Call>>>,
    identities: &Rc<RefCell<Vec<bool>>>,
) {
    for (index, (name, action)) in [
        ("onRefresh", &mut card.on_refresh),
        ("onEdit", &mut card.on_edit),
        ("onDelete", &mut card.on_delete),
    ]
    .into_iter()
    .enumerate()
    {
        if mask & (1 << index) != 0 {
            let (calls, identities, original) = (
                Rc::clone(calls),
                Rc::clone(identities),
                Rc::clone(&card.provider),
            );
            *action = Some(Rc::new(move |provider| {
                identities
                    .borrow_mut()
                    .push(Rc::ptr_eq(&provider, &original));
                calls.borrow_mut().push(fixture::Call {
                    name: name.into(),
                    provider_id: provider.borrow().id.clone(),
                });
            }));
        }
    }
}

/// A sibling root invokes only its own callback.
fn independent_card(parent: &Element, view: &maestro_web::dom::DomView) {
    let second_count = Rc::new(std::cell::Cell::new(0));
    let second = Rc::clone(&second_count);
    let mut sibling = CustomProviderCard::new(provider());
    sibling.on_delete = Some(Rc::new(move |_| second.set(second.get() + 1)));
    let sibling_root = sibling.mount(view).unwrap();
    parent.append_child(&sibling_root.node).unwrap();
    buttons(&sibling_root.node.unchecked_into())
        .remove(0)
        .click();
    assert_eq!(second_count.get(), 1);
}

/// Inserted actions preserve the existing control; retired nodes never revive.
fn action_updates(
    card: &mut CustomProviderCard,
    root: &maestro_web::dom::DomRoot,
    view: &maestro_web::dom::DomView,
) {
    let element: &Element = root.node.unchecked_ref();
    let edit = buttons(element).remove(0);
    edit.focus().unwrap();
    let count = Rc::new(std::cell::Cell::new(0));
    let counted = Rc::clone(&count);
    card.on_refresh = Some(Rc::new(move |_| counted.set(counted.get() + 1)));
    let counted = Rc::clone(&count);
    card.on_delete = Some(Rc::new(move |_| counted.set(counted.get() + 1)));
    card.is_auto_discovery = true;
    card.update(root, view).unwrap();
    let inserted = buttons(element);
    assert_eq!(
        inserted.iter().map(|node| text(node)).collect::<Vec<_>>(),
        ["Refresh", "Edit", "Delete"]
    );
    assert!(inserted[1].is_same_node(Some(&edit)));
    assert_eq!(view.document.active_element().unwrap(), edit.into());
    inserted[0].click();
    inserted[2].click();
    assert_eq!(count.get(), 2);
    localized_actions(card, root, view);
    card.on_refresh = None;
    card.on_delete = None;
    card.update(root, view).unwrap();
    assert_eq!(buttons(element).len(), 1);
    let counted = Rc::clone(&count);
    card.on_delete = Some(Rc::new(move |_| counted.set(counted.get() + 1)));
    card.update(root, view).unwrap();
    let replacement = buttons(element).remove(1);
    assert!(!replacement.is_same_node(Some(&inserted[2])));
    inserted[0].click();
    inserted[2].click();
    assert_eq!(count.get(), 2);
    replacement.click();
    assert_eq!(count.get(), 3);
    card.on_delete = None;
    card.is_auto_discovery = false;
    card.update(root, view).unwrap();
}

/// Replacing localization updates all current action labels without replacing focus stops.
fn localized_actions(
    card: &CustomProviderCard,
    root: &maestro_web::dom::DomRoot,
    view: &maestro_web::dom::DomView,
) {
    let element: &Element = root.node.unchecked_ref();
    let before = buttons(element);
    card.update(root, &localized("de")).unwrap();
    let current = buttons(element);
    assert_eq!(
        current.iter().map(|node| text(node)).collect::<Vec<_>>(),
        ["Aktualisieren", "Bearbeiten", "Löschen"]
    );
    assert!(
        current
            .iter()
            .zip(&before)
            .all(|(new, old)| new.is_same_node(Some(old)))
    );
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(&before[1]))
    );
    card.update(root, view).unwrap();
    assert_eq!(
        buttons(element)
            .iter()
            .map(|node| text(node))
            .collect::<Vec<_>>(),
        ["Refresh", "Edit", "Delete"]
    );
}
