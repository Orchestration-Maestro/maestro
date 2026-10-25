//! Independent header controls of the HTML artifact.
use super::{HtmlArtifact, sandbox_id};
use crate::{
    browser::event_subscription::EventSubscription,
    components::{
        button::{ButtonProps, ButtonSize, ButtonVariant, button},
        copy_button::dom as copy_button,
        download_button::dom::{DownloadButtonProps, DownloadContent, download_button},
        preview_code_toggle::PreviewCodeToggle,
        sandboxed_iframe::PrepareHtmlOptions,
    },
    dom::{DomContent as _, DomRoot, DomView},
    icons,
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::{any::Any, borrow::Cow, cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;

/// One owned native event registration.
type Event = EventSubscription<Closure<dyn Fn()>>;
/// Resources released with one header.
struct Owned {
    /// Mounted preview/source control.
    _toggle: DomRoot,
    /// Selection and reload registrations.
    _events: [Event; 2],
    /// Download control resources.
    _download: Option<Box<dyn Any>>,
}
impl HtmlArtifact {
    /// Build an independent header: preview/source selection, reload, copy and download.
    ///
    /// Copy and download keep the source of this call; selection and reload act on the body
    /// this artifact last mounted or updated, with that body's current operands.
    /// # Errors
    /// Returns localization, standalone preparation or native control failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let row = header::row(view)?;
        row.class_list()
            .add_1("maestro-html-header")
            .map_err(|error| ToolUiError::native(&error))?;
        let (toggle, selection) = self.toggle(view)?;
        let (reload, reloading) = self.reload(view)?;
        let localization = Rc::new(RefCell::new(Rc::clone(&view.i18n)));
        let copy = copy_button::create(
            &view.document,
            &localization,
            copy_button::FEEDBACK_MS,
            None,
            None,
        )
        .map_err(|error| ToolUiError::native(&error))?;
        let title = view.i18n.text("Copy HTML")?;
        for (name, value) in [("text", self.content.as_str()), ("title", title.as_str())] {
            copy.set_attribute(name, value)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        let saved = self.download_text()?;
        let mut download = download_button(
            view,
            DownloadButtonProps {
                mime_type: "text/html".to_owned(),
                title: Some(view.i18n.text("Download HTML")?),
                ..DownloadButtonProps::new(
                    DownloadContent::Bytes(js_sys::Uint8Array::from(saved.as_bytes())),
                    self.filename.clone(),
                )
            },
        )?;
        for node in [&toggle.node, &reload, &copy.into(), &download.node] {
            row.append_child(node)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        Ok(DomRoot {
            node: row.into(),
            owner: Some(Box::new(Owned {
                _toggle: toggle,
                _events: [selection, reloading],
                _download: download.owner.take(),
            })),
        })
    }
    /// The preview/source control, reporting each changed selection to the current body.
    fn toggle(&self, view: &DomView) -> Result<(DomRoot, Event), ToolUiError> {
        let toggle = PreviewCodeToggle::new();
        toggle.set_mode(self.mode.get());
        let root = toggle.mount(view)?;
        let (mode, body) = (Rc::clone(&self.mode), Rc::clone(&self.body));
        let changed = Closure::<dyn Fn()>::new(move || {
            let live = body.borrow().upgrade();
            match live {
                // Callbacks have no error channel; a failed change keeps the shown region.
                Some(body) => {
                    let _ = body.select(toggle.mode());
                }
                None => mode.set(toggle.mode()),
            }
        });
        let event = EventSubscription::new(root.node.clone().into(), "mode-change", changed)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok((root, event))
    }
    /// The reload control, clearing the logs and loading the current body's source again.
    fn reload(&self, view: &DomView) -> Result<(web_sys::Node, Event), ToolUiError> {
        let glyph = crate::browser::attachment_content::glyph_node("RefreshCw")?;
        let icon = icons::icon_dom(&glyph, icons::IconSize::Sm, None)
            .map_err(|error| ToolUiError::native(&error))?;
        let title = view.i18n.text("Reload HTML")?;
        let props = ButtonProps {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Sm,
            title: Some(&title),
            ..ButtonProps::default()
        };
        let control = button(&view.document, &props, &[icon.into()])
            .map_err(|error| ToolUiError::native(&error))?;
        let (logs, body) = (Rc::clone(&self.logs), Rc::clone(&self.body));
        let clicked = Closure::<dyn Fn()>::new(move || {
            let live = body.borrow().upgrade();
            match live {
                // Callbacks have no error channel; a failed load leaves the frame empty.
                Some(body) => {
                    let _ = body.reload();
                }
                None => logs.borrow_mut().clear(),
            }
        });
        let event = EventSubscription::new(control.clone().into(), "click", clicked)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok((control.into(), event))
    }
    /// The saved document: standalone preparation through the connected frame, otherwise the
    /// original source.
    fn download_text(&self) -> Result<Cow<'_, str>, ToolUiError> {
        let sandbox = self.sandbox_iframe_ref.borrow().clone();
        let Some(sandbox) = sandbox else {
            return Ok(Cow::Borrowed(&self.content));
        };
        let options = PrepareHtmlOptions {
            is_html_artifact: true,
            is_standalone: true,
        };
        sandbox
            .prepare_html_document(
                &sandbox_id(&self.filename),
                &self.content,
                &self.runtime_providers,
                Some(options),
            )
            .map(Cow::Owned)
            .map_err(|error| ToolUiError::Presentation(error.message))
    }
}
