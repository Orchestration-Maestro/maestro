//! Literal native empty-session presentation.
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, ReadingContent, ReadingRoot,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::{ToolRenderView, ToolUiError},
};
use std::rc::Rc;

/// Retain the native literal only while the view has no session.
pub(super) struct EmptySession {
    /// Existing placement owner.
    host: Rc<dyn AttachmentHost>,
    /// Native surface identity.
    id: egui::Id,
    /// Current muted paint and its owned factory.
    content: Option<(String, String, Rc<ReadingContent>)>,
}
impl EmptySession {
    /// Bind without mounting before the first render.
    pub(super) fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            id: attachment_content::identity(),
            content: None,
        }
    }
    /// Publish the localized literal through the existing reading transaction.
    pub(super) fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        let color = view.presentation.colors["muted"];
        let text = view.i18n.text("No session set")?;
        if self
            .content
            .as_ref()
            .is_none_or(|(shown, literal, _)| shown != color || literal != &text)
        {
            self.content = Some((color.into(), text.clone(), content(color, text)));
        }
        let content = &self
            .content
            .as_ref()
            .ok_or_else(|| ToolUiError::Presentation("Missing empty-session content".into()))?
            .2;
        Ok(ControlledReading {
            host: self.host.as_ref(),
            id: self.id,
            content,
            sizes: &[],
            intrinsic_width: false,
            height: ReadingHeight::Flow,
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)?
        .response)
    }
    /// Retire the literal as soon as a session takes its place.
    pub(super) fn clear(&mut self) {
        self.host.remove(self.id);
        self.content = None;
    }
}
impl Drop for EmptySession {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// No markup parsing or message-row styling is needed for the empty literal.
fn content(color: &str, text: String) -> Rc<ReadingContent> {
    let color = color.to_owned();
    Rc::new(ReadingContent {
        mount: Rc::new(move |document| {
            use attachment_content::browser::{element, host_error};
            let root = element(document, "div", "maestro-transcript-empty")?;
            root.set_text_content(Some(&text));
            root.set_attribute(
                "style",
                &format!("box-sizing:border-box;padding:1rem;text-align:center;color:{color};"),
            )
            .map_err(|error| host_error(&error))?;
            Ok(ReadingRoot {
                frame_style: "",
                decoration: None,
                element: root,
                slots: vec![],
                owner: None,
                commit: None,
            })
        }),
    })
}
