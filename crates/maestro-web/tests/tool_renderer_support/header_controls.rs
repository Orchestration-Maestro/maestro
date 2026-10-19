//! Shared tool headers on the native frame: recorded status glyphs, loaders and controls.
#![cfg(test)]
use super::{Caption, cases, header, status};
use crate::{
    brand::Pack, canvas_paint, copy_button_support, effects_support::Host,
    highlighter_support::NoHighlight,
};
use maestro_web::{
    browser::attachment_content::AttachmentHost,
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText},
        types::{ToolRenderResult, ToolRenderState, ToolRenderView},
    },
};
use std::{cell::Cell, rc::Rc};

/// Header rows drawn through the shared native frame and final-clip paint.
pub(super) struct Canvas {
    /// Controlled platform receiving native text and listings.
    pub(super) host: Rc<Host>,
    /// Retained toolkit state across frames.
    pub(super) context: egui::Context,
    /// Texture-backed paint observer.
    paint: canvas_paint::Paint,
    /// Latest completed frame.
    output: egui::FullOutput,
}
impl Canvas {
    pub(super) fn new() -> Self {
        Self {
            host: Rc::new(Host::default()),
            context: egui::Context::default(),
            paint: canvas_paint::Paint::default(),
            output: egui::FullOutput::default(),
        }
    }
    /// Draw one row with the supplied input events.
    pub(super) fn draw(
        &mut self,
        recipe: &ToolRenderResult,
        row: &mut ToolRenderState,
        events: Vec<egui::Event>,
    ) -> egui::Response {
        let host: Rc<dyn AttachmentHost> = self.host.clone();
        let (response, output) = copy_button_support::draw(
            &self.context,
            &mut self.paint,
            events,
            (egui::Rect::EVERYTHING, true),
            |ui, i18n, presentation| {
                let view = ToolRenderView {
                    host: &host,
                    i18n,
                    presentation,
                    highlighter: &NoHighlight,
                    modal: None,
                };
                recipe.render(ui, view, row).unwrap()
            },
        );
        self.output = output;
        response
    }
    /// Press and release the primary button at one position.
    fn click(&mut self, at: egui::Pos2, recipe: &ToolRenderResult, row: &mut ToolRenderState) {
        self.draw(recipe, row, copy_button_support::pointer(at, true));
        self.draw(recipe, row, copy_button_support::pointer(at, false));
    }
    /// Whether the latest frame announced a click on a button with this label.
    fn announced(&self, label: &str) -> bool {
        self.output.platform_output.events.iter().any(|event| {
            matches!(event, egui::output::OutputEvent::Clicked(info)
                if info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(label))
        })
    }
    /// Painted glyph bounds in paint order; text samples the font atlas instead.
    fn glyphs(&self) -> Vec<egui::Rect> {
        self.output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            })
            .collect()
    }
    /// Coverage of the painted pixels in a region.
    fn alpha(&self, rect: egui::Rect) -> Vec<u8> {
        self.paint
            .region(&self.context, &self.output.shapes, rect)
            .into_iter()
            .map(|pixel| pixel[3])
            .collect()
    }
    /// Require visible paint in the brand color of `role`.
    fn assert_role(&self, rect: egui::Rect, role: &str) {
        let pack = maestro_theme::load_brand_pack("brand.json", &Pack).unwrap();
        let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
        let color = egui::Color32::from_hex(presentation.colors[role]).unwrap();
        let expected = [color.r(), color.g(), color.b()];
        let pixels = self.paint.region(&self.context, &self.output.shapes, rect);
        assert!(
            pixels.iter().any(|pixel| pixel[3] > 100
                && (0..3).all(|i| (i32::from(pixel[i]) * 255
                    - i32::from(expected[i]) * i32::from(pixel[3]))
                .abs()
                    < 1020)),
            "painted glyph does not use {role}"
        );
    }
    /// Require the recorded glyph roles in paint order.
    fn assert_roles(&self, roles: &serde_json::Value) -> Vec<egui::Rect> {
        let icons = self.glyphs();
        let roles = roles.as_array().unwrap();
        assert_eq!(icons.len(), roles.len());
        for (rect, role) in icons.iter().zip(roles) {
            self.assert_role(*rect, role.as_str().unwrap());
        }
        icons
    }
}

#[test]
fn tool_headers_paint_recorded_status_glyphs_and_turn_the_loader() {
    let mut canvas = Canvas::new();
    let mut tool = None;
    for case in cases()["headers"].as_array().unwrap() {
        let expected = &case["expected"];
        let text = expected["text"].as_str().unwrap();
        let text = if case["rich"].as_bool().unwrap() {
            ToolHeaderText::Content(Box::new(Caption(text.into())))
        } else {
            ToolHeaderText::Text(text.into())
        };
        let recipe = header(status(case["state"].as_str().unwrap()), text, None);
        let mut row = ToolRenderState::default();
        canvas.draw(&recipe, &mut row, vec![]);
        let icons = canvas.assert_roles(&expected["iconRoles"]);
        let code = canvas.alpha(icons[0]);
        assert_eq!(*tool.get_or_insert_with(|| code.clone()), code);
        if expected["spinner"] == 1 {
            assert!(icons[0].right() < icons[1].left());
            let loader = canvas.alpha(icons[1]);
            assert_ne!(loader, code);
            canvas.draw(&recipe, &mut row, vec![]);
            let icons = canvas.glyphs();
            assert_eq!(canvas.alpha(icons[0]), code);
            assert_ne!(canvas.alpha(icons[1]), loader, "the pending loader turns");
        }
    }
}

#[test]
fn collapsible_tool_headers_toggle_recorded_states() {
    let mut canvas = Canvas::new();
    for case in cases()["collapsible"].as_array().unwrap() {
        let expanded = Rc::new(Cell::new(case["expanded"].as_bool().unwrap()));
        let state = status(case["state"].as_str().unwrap());
        let recipe = header(
            state,
            ToolHeaderText::Text("caption".into()),
            Some(expanded.clone()),
        );
        let mut row = ToolRenderState::default();
        let mut previous = None;
        for phase in ["initial", "toggled", "restored"] {
            let response = canvas.draw(&recipe, &mut row, vec![]);
            let expected = &case["expected"][phase];
            assert_eq!(expanded.get(), expected["expanded"].as_bool().unwrap());
            let icons = canvas.assert_roles(&expected["iconRoles"]);
            if state == ToolHeaderState::Inprogress {
                assert!(icons[0].right() < icons[1].left(), "the loader leads");
            }
            let chevron = canvas.alpha(*icons.last().unwrap());
            assert_ne!(previous.replace(chevron.clone()), Some(chevron));
            if phase != "restored" {
                let at = egui::pos2(response.rect.right() - 8., response.rect.center().y);
                canvas.click(at, &recipe, &mut row);
                assert!(canvas.announced("caption"));
            }
        }
    }
}

#[test]
fn focused_collapsible_tool_header_toggles_from_the_keyboard() {
    let mut canvas = Canvas::new();
    let expanded = Rc::new(Cell::new(false));
    let recipe = header(
        ToolHeaderState::Complete,
        ToolHeaderText::Text("caption".into()),
        Some(expanded.clone()),
    );
    let mut row = ToolRenderState::default();
    let response = canvas.draw(&recipe, &mut row, vec![]);
    canvas
        .context
        .memory_mut(|memory| memory.request_focus(response.id));
    canvas.draw(&recipe, &mut row, vec![]);
    for (key, wanted) in [(egui::Key::Enter, true), (egui::Key::Space, false)] {
        let press = egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        canvas.draw(&recipe, &mut row, vec![press]);
        assert_eq!(expanded.get(), wanted);
    }
}

#[test]
fn collapsible_tool_headers_show_pointer_and_accent_focus_in_every_state() {
    let mut canvas = Canvas::new();
    for state in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        let recipe = header(
            state,
            ToolHeaderText::Text("caption".into()),
            Some(Rc::new(Cell::new(false))),
        );
        let mut row = ToolRenderState::default();
        let response = canvas.draw(&recipe, &mut row, vec![]);
        let hover = vec![egui::Event::PointerMoved(response.rect.center())];
        canvas.draw(&recipe, &mut row, hover);
        canvas.draw(&recipe, &mut row, vec![]);
        assert_eq!(
            canvas.output.platform_output.cursor_icon,
            egui::CursorIcon::PointingHand
        );
        let border = |icons: &[egui::Rect]| {
            egui::Rect::from_min_max(
                egui::pos2(icons[0].right() + 2., response.rect.top()),
                egui::pos2(icons.last().unwrap().left() - 2., response.rect.top() + 2.),
            )
        };
        canvas
            .context
            .memory_mut(|memory| memory.surrender_focus(response.id));
        canvas.draw(&recipe, &mut row, vec![egui::Event::PointerGone]);
        let unfocused = canvas.alpha(border(&canvas.glyphs()));
        canvas
            .context
            .memory_mut(|memory| memory.request_focus(response.id));
        assert!(canvas.draw(&recipe, &mut row, vec![]).has_focus());
        let border = border(&canvas.glyphs());
        assert_ne!(canvas.alpha(border), unfocused);
        canvas.assert_role(border, "accent");
    }
    let plain = header(
        ToolHeaderState::Complete,
        ToolHeaderText::Text("caption".into()),
        None,
    );
    let mut row = ToolRenderState::default();
    let response = canvas.draw(&plain, &mut row, vec![]);
    let hover = vec![egui::Event::PointerMoved(response.rect.center())];
    canvas.draw(&plain, &mut row, hover);
    canvas.draw(&plain, &mut row, vec![]);
    assert_eq!(
        canvas.output.platform_output.cursor_icon,
        egui::CursorIcon::Default
    );
}

#[test]
fn literal_captions_release_their_native_text_when_replaced_or_dropped() {
    let mut canvas = Canvas::new();
    let literal = header(
        ToolHeaderState::Complete,
        ToolHeaderText::Text("caption".into()),
        None,
    );
    let rich = header(
        ToolHeaderState::Complete,
        ToolHeaderText::Content(Box::new(Caption("rich".into()))),
        None,
    );
    let mut row = ToolRenderState::default();
    canvas.draw(&literal, &mut row, vec![]);
    canvas.draw(&literal, &mut row, vec![]);
    assert!(canvas.host.removed.borrow().is_empty());
    canvas.draw(&rich, &mut row, vec![]);
    let released = canvas.host.removed.borrow().clone();
    assert_eq!(
        released.len(),
        1,
        "the literal caption leaves with its text"
    );
    canvas.draw(&literal, &mut row, vec![]);
    drop(row);
    let removed = canvas.host.removed.borrow();
    assert_eq!(removed.len(), 2);
    assert_ne!(
        removed[1], released[0],
        "each literal caption owns its text"
    );
}
