//! Genuine tile clicks forwarded through registered, rich and containing row paths.
#![cfg(test)]
use super::{escape, frame_owner};
use crate::{
    attachment_support::{Frame, Host, attachment, click},
    recipes,
};
use maestro_web::{
    AttachmentTile, ToolMessage, ToolMessageProps, ToolRenderer,
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText, render_header},
        types::{
            ToolContent, ToolContentOutput, ToolRenderResult, ToolRenderView, ToolRendererBinding,
            ToolUiError,
        },
    },
};
use std::{cell::Cell, rc::Rc};

pub(super) struct TileRecipe {
    pub(super) record: maestro_web::SharedAttachment,
    pub(super) fail: Rc<Cell<bool>>,
}
impl ToolContent for TileRecipe {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> ToolContentOutput {
        let slot = state.state::<Option<AttachmentTile>>();
        let mut tile = slot
            .borrow_mut()
            .take()
            .unwrap_or_else(|| AttachmentTile::new(self.record.clone(), view.host.clone()));
        let mut overlays = Vec::new();
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            overlays.extend(
                tile.render(ui, view.i18n, view.presentation)
                    .map_err(ToolUiError::Attachment)?,
            );
            if self.fail.get() {
                return Err(ToolUiError::Presentation(
                    "controlled later child failure".into(),
                ));
            }
            Ok(())
        });
        *slot.borrow_mut() = Some(tile);
        ToolContentOutput {
            response: row.inner.map(|()| row.response),
            overlays,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Header {
    Plain,
    Rich,
}
pub(super) fn registration(name: &str, fail: bool, header: Header) -> Rc<Cell<bool>> {
    let flag = Rc::new(Cell::new(fail));
    let fail = flag.clone();
    let record = attachment();
    let renderer: Rc<dyn ToolRenderer> = Rc::new(recipes::Renderer(Rc::new(move |_, _, _| {
        let content: Box<dyn ToolContent> = Box::new(TileRecipe {
            record: record.clone(),
            fail: fail.clone(),
        });
        let content = if matches!(header, Header::Rich) {
            render_header(
                ToolHeaderState::Complete,
                maestro_web::icons::icons().unwrap()["Code"].clone(),
                ToolHeaderText::Content(content),
            )
        } else {
            content
        };
        Ok(ToolRenderResult {
            content,
            is_custom: true,
        })
    })));
    maestro_web::register_tool_renderer(name, ToolRendererBinding::from_raw(renderer));
    flag
}
pub fn registered() {
    for header in [Header::Plain, Header::Rich] {
        for fail in [false, true] {
            registration("document-forwarding", fail, header);
            let host = Rc::new(Host::default());
            let mut frame = Frame::default();
            let mut row = ToolMessage::new();
            let call: maestro_app::ToolCall = serde_json::from_value(serde_json::json!({"type":"toolCall","id":"call","name":"document-forwarding","arguments":{}})).unwrap();
            let props = ToolMessageProps::<serde_json::Value> {
                tool_call: &call,
                tool: None,
                result: None,
                pending: false,
                aborted: false,
                is_streaming: false,
            };
            let first = frame_owner(&mut frame, &host, vec![], |ui, view| {
                row.render(ui, props, view)
            });
            assert_eq!(first.response.is_err(), fail);
            assert!(first.overlays.is_empty());
            let at = if matches!(header, Header::Rich) {
                egui::pos2(70., 32.)
            } else {
                egui::pos2(32., 32.)
            };
            let mut output = frame_owner(&mut frame, &host, click(at), |ui, view| {
                row.render(ui, props, view)
            });
            assert_eq!(output.response.is_err(), fail);
            assert_eq!(output.overlays.len(), 1);
            let mut overlay = output.overlays.pop().unwrap();
            drop(row);
            assert!(frame.overlay(&mut overlay, vec![]).unwrap());
            frame.overlay(&mut overlay, vec![]).unwrap();
            let download = frame.control(1);
            frame.overlay(&mut overlay, click(download)).unwrap();
            assert_eq!(host.downloads.borrow().len(), 1);
            assert!(!frame.overlay(&mut overlay, vec![escape()]).unwrap());
        }
    }
}
