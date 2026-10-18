//! Native resources retained by output recipes after their borrowing call ends.
#![cfg(test)]
use super::browser;
use crate::surface;
use maestro_web::{
    tools::types::ToolUiError,
    tools::types::{
        ToolContent, ToolRenderResult, ToolRenderState, ToolRenderView, ToolResultMessageView,
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::prelude::*;

struct Action {
    label: String,
    original: Rc<Cell<i32>>,
}
#[derive(Default)]
struct Row {
    clicks: usize,
    original: Option<Rc<Cell<i32>>>,
}
impl ToolContent for Action {
    fn render(
        &self,
        ui: &mut egui::Ui,
        _: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> Result<egui::Response, ToolUiError> {
        let slot = state.state::<Row>();
        let mut row = slot.take();
        row.original = Some(self.original.clone());
        let response = ui.button(format!("{} {}", self.label, row.clicks));
        if response.clicked() {
            row.clicks += 1;
            self.original.set(self.original.get() + 1);
        }
        let old = slot.replace(row);
        drop(old);
        Ok(response)
    }
}
struct Renderer;
impl maestro_web::ToolRenderer<String, Rc<Cell<i32>>> for Renderer {
    fn render(
        &self,
        params: Option<&String>,
        result: Option<ToolResultMessageView<'_, Rc<Cell<i32>>>>,
        _: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        Ok(ToolRenderResult {
            content: Box::new(Action {
                label: params.unwrap().clone(),
                original: result.unwrap().details().unwrap().clone(),
            }),
            is_custom: true,
        })
    }
}
#[wasm_bindgen]
pub fn tool_content_keeps_owned_inputs_and_independent_row_lifetimes() {
    let (prepared, weak) = prepare_owned();
    let alias = Rc::new(RefCell::new(None));
    let recipe = ToolRenderResult {
        content: Box::new(crate::Aliased {
            inner: prepared.content,
            alias: alias.clone(),
        }),
        is_custom: true,
    };
    let mut surface = surface();
    let mut first = ToolRenderState::default();
    let response = browser::draw(&mut surface, &recipe, &mut first);
    browser::click(&mut surface, response.rect.center(), &recipe, &mut first);
    assert_eq!(weak.upgrade().unwrap().get(), 43);
    let first_alias = alias.take().unwrap();
    let mut second = ToolRenderState::default();
    browser::draw(&mut surface, &recipe, &mut second);
    assert!(
        surface
            .painted
            .iter()
            .any(|(text, _)| text == "owned caption 0")
    );
    let second_alias = alias.take().unwrap();
    browser::draw(&mut surface, &recipe, &mut first);
    assert!(
        surface
            .painted
            .iter()
            .any(|(text, _)| text == "owned caption 1")
    );
    alias.take();
    drop(first);
    drop(second);
    drop(recipe);
    super::reset();
    assert_eq!(first_alias.state::<Row>().borrow().clicks, 1);
    assert_eq!(second_alias.state::<Row>().borrow().clicks, 0);
    first_alias
        .state::<Row>()
        .borrow()
        .original
        .as_ref()
        .unwrap()
        .set(44);
    assert_eq!(weak.upgrade().unwrap().get(), 44);
    drop(first_alias);
    assert!(weak.upgrade().is_some());
    drop(second_alias);
    assert!(weak.upgrade().is_none());
}

fn prepare_owned() -> (ToolRenderResult, std::rc::Weak<Cell<i32>>) {
    super::reset();
    maestro_web::register_tool_renderer(
        "owned",
        maestro_web::tools::types::ToolRendererBinding::from_raw(Rc::new(Renderer)),
    );
    let original = Rc::new(Cell::new(42));
    let weak = Rc::downgrade(&original);
    let message = super::message(Some(original.clone()));
    let params = String::from("owned caption");
    let prepared = maestro_web::render_tool(
        "owned",
        Some(maestro_web::tools::types::ToolParams::typed(
            &params,
            |_| unreachable!(),
        )),
        Some(&message),
        false,
    )
    .unwrap();
    drop(params);
    drop(message);
    drop(original);

    (prepared, weak)
}
