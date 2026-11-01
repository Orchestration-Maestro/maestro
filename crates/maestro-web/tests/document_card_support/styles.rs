//! Native document-card brand roles and reached localization/highlighting causes.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{browser::Card, fixture, prepare};
use crate::contributions_support::{computed, host, rgb, select, view};
use maestro_web::{dom::DomView, tools::types::ToolUiError};
use std::{error::Error as _, rc::Rc, sync::Arc};

/// Both supplied packs and modes color native text and status without a canvas host.
pub fn run(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    for pack in [
        view.pack.clone(),
        Rc::new(crate::brand::pack("signal.json").unwrap()),
    ] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            for error in [false, true] {
                let view = Rc::new(DomView {
                    pack: pack.clone(),
                    mode,
                    ..(*view).clone()
                });
                card(view, error);
            }
        }
    }
    errors(&view);
}
/// The original result caption, URL typography and status use the supplied brand roles.
fn card(view: Rc<DomView>, error: bool) {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "extract-result-valid-false-false")
        .unwrap();
    row.input.result.as_mut().unwrap().is_error = error;
    let card = Card::new(view, &row);
    let view = &card.view;
    let presentation = view.pack.presentation(view.mode).unwrap();
    let role = if error { "error" } else { "success" };
    let status = select(
        &card.root(),
        if error {
            ".maestro-tool-error"
        } else {
            ".maestro-tool-complete"
        },
    );
    assert_eq!(computed(&status, "color"), rgb(presentation.colors[role]));
    let url = select(&card.root(), ".maestro-tool-url");
    assert_eq!(computed(&url, "color"), rgb(presentation.colors["muted"]));
    assert_eq!(computed(&url, "font-size"), "14px");
    assert_eq!(computed(&url, "line-height"), "20px");
    assert_eq!(select(&url, "strong").text_content().unwrap(), "URL:");
    assert_eq!(url.query_selector_all("strong").unwrap().length(), 1);
}
/// Reached failures retain their original typed cause instead of succeeding with stale content.
fn errors(view: &Rc<DomView>) {
    let row = fixture()
        .into_iter()
        .find(|r| r.id == "extract-result-valid-false-false")
        .unwrap();
    let recipe = prepare(&row).unwrap();
    let dictionaries = view.i18n.get_translations();
    let borrow = dictionaries.borrow_mut();
    assert!(matches!(
        recipe.content.mount(view),
        Err(ToolUiError::I18n(_))
    ));
    drop(borrow);
    let marker = Arc::new(());
    let failing = DomView {
        highlighter: Rc::new(crate::failing_highlighter::FailingEngine(marker.clone())),
        ..(**view).clone()
    };
    let error = recipe.content.mount(&failing).err().unwrap();
    let cause = error
        .source()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<crate::failing_highlighter::Cause>()
        .unwrap();
    assert!(Arc::ptr_eq(&cause.0, &marker));
}
