//! Native mounting and event ownership.
use super::AttachmentTile;
use crate::{
    AttachmentOverlay, SharedAttachment,
    browser::{
        attachment_content::browser::{append, element},
        event_subscription::EventSubscription,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{self, DomContent, DomRoot, DomView},
    icons::IconSize,
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::{Element, Event};
/// Native listener retained by the mounted tile.
type Binding = EventSubscription<Closure<dyn Fn(Event) -> Result<(), JsValue>>>;
/// The record and page an activation of the opener reads.
type Opening = Rc<RefCell<(SharedAttachment, Rc<DomView>)>>;
/// The removal callback an activation of the remove control reads.
type Removal = Rc<RefCell<Option<Rc<dyn Fn()>>>>;
/// Children of the opener: a preview image, or a spreadsheet or document glyph and label.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Thumbnail {
    /// Preview image.
    Image,
    /// Fallback glyph and shortened filename.
    Fallback {
        /// Whether the glyph is the spreadsheet one.
        spreadsheet: bool,
    },
}
/// Controls mounted once per tile root and the operands their listeners read.
struct Mounted {
    /// Container of the opener, the badge and the remove control.
    shell: Element,
    /// Opening button.
    open: Element,
    /// Children the opener shows, compared before replacing them.
    thumbnail: Cell<Option<Thumbnail>>,
    /// PDF badge, while a PDF preview shows.
    badge: RefCell<Option<Element>>,
    /// Remove control and its listener, while removal shows.
    remove: RefCell<Option<(Element, Binding)>>,
    /// Current record and page of the opener.
    opening: Opening,
    /// Current removal callback.
    removal: Removal,
    /// Opening listener, removed with the root.
    _open: Binding,
}
impl DomContent for AttachmentTile {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let node = element(&view.document, "div", "maestro-attachment-tile")?;
        let shell = element(&view.document, "div", "maestro-tile-shell")?;
        let open = element(&view.document, "button", "maestro-tile-open")?;
        for (name, value) in [("type", "button"), ("data-open", "")] {
            open.set_attribute(name, value)
                .map_err(|error| ToolUiError::native(&error))?;
        }
        append(&shell, &open)?;
        append(&node, &shell)?;
        let opening = Rc::new(RefCell::new((
            Arc::clone(&self.attachment),
            Rc::new(view.clone()),
        )));
        let mounted = Mounted {
            _open: open_binding(&open, &opening)?,
            shell,
            open,
            thumbnail: Cell::default(),
            badge: RefCell::default(),
            remove: RefCell::default(),
            opening,
            removal: Rc::default(),
        };
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(mounted)),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Read the record and localized text first, then change only what differs.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("Missing tile state".into()))?;
        let (name, preview, pdf, excel) =
            super::thumbnail_data(&self.attachment).map_err(ToolUiError::Attachment)?;
        let badge = (preview.is_some() && pdf)
            .then(|| view.i18n.text("PDF"))
            .transpose()?;
        let remove = self
            .show_delete
            .then(|| view.i18n.text("Remove"))
            .transpose()?;
        mounted.show_thumbnail(view, &name, preview.as_deref(), excel)?;
        mounted.show_badge(view, badge)?;
        mounted.show_remove(view, remove.as_deref())?;
        let previous = mounted
            .opening
            .replace((Arc::clone(&self.attachment), Rc::new(view.clone())));
        let removal = mounted.removal.replace(self.on_delete.clone());
        drop((previous, removal));
        Ok(())
    }
}
impl Mounted {
    /// Replace the opener's children only when their kind changes; names and sources update in place.
    fn show_thumbnail(
        &self,
        view: &DomView,
        name: &str,
        preview: Option<&str>,
        excel: bool,
    ) -> Result<(), ToolUiError> {
        let kind = preview.map_or(Thumbnail::Fallback { spreadsheet: excel }, |_| {
            Thumbnail::Image
        });
        if self.thumbnail.get() != Some(kind) {
            self.replace_thumbnail(view, kind)?;
        }
        let shown = self.open.last_element_child();
        match (preview, shown) {
            (Some(source), Some(image)) => {
                for (attribute, value) in [("src", source), ("alt", name), ("title", name)] {
                    set(&image, attribute, value)?;
                }
            }
            (None, Some(label)) => {
                let text = super::filename_label(name);
                if label.text_content().as_deref() != Some(text.as_str()) {
                    label.set_text_content(Some(&text));
                }
            }
            (_, None) => {}
        }
        set(&self.open, "title", name)?;
        set(&self.open, "aria-label", name)
    }
    /// Prepare new thumbnail children before replacing the old ones.
    fn replace_thumbnail(&self, view: &DomView, kind: Thumbnail) -> Result<(), ToolUiError> {
        let children = js_sys::Array::new();
        if let Thumbnail::Fallback { spreadsheet } = kind {
            let icon = if spreadsheet {
                "FileSpreadsheet"
            } else {
                "FileText"
            };
            children.push(&glyph(icon, IconSize::Md)?.into());
            children.push(&element(&view.document, "span", "maestro-tile-label")?.into());
            self.open
                .set_class_name("maestro-tile-open maestro-tile-fallback");
        } else {
            children.push(&element(&view.document, "img", "maestro-tile-image")?.into());
            self.open.set_class_name("maestro-tile-open");
        }
        self.open.replace_children_with_node(&children);
        self.thumbnail.set(Some(kind));
        Ok(())
    }
    /// Show, relabel or remove the PDF badge after the opener.
    fn show_badge(&self, view: &DomView, text: Option<String>) -> Result<(), ToolUiError> {
        let mut badge = self.badge.borrow_mut();
        match (text, badge.as_ref()) {
            (Some(text), Some(shown)) => {
                if shown.text_content().as_deref() != Some(text.as_str()) {
                    shown.set_text_content(Some(&text));
                }
            }
            (Some(text), None) => {
                let shown = element(&view.document, "div", "maestro-tile-badge")?;
                shown.set_text_content(Some(&text));
                self.open
                    .after_with_node_1(&shown)
                    .map_err(|error| ToolUiError::native(&error))?;
                *badge = Some(shown);
            }
            (None, Some(shown)) => {
                shown.remove();
                *badge = None;
            }
            (None, None) => {}
        }
        Ok(())
    }
    /// Show, retitle or remove the remove control.
    fn show_remove(&self, view: &DomView, title: Option<&str>) -> Result<(), ToolUiError> {
        let mut remove = self.remove.borrow_mut();
        match (title, remove.as_ref()) {
            (Some(title), Some((control, _))) => set(control, "title", title)?,
            (Some(title), None) => *remove = Some(self.remove_control(view, title)?),
            (None, Some(_)) => {
                if let Some((control, binding)) = remove.take() {
                    control.remove();
                    drop(binding);
                }
            }
            (None, None) => {}
        }
        Ok(())
    }
    /// Removal is an independent native button and never bubbles into opening.
    fn remove_control(
        &self,
        view: &DomView,
        title: &str,
    ) -> Result<(Element, Binding), ToolUiError> {
        let control = button(
            &view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Icon,
                class_name: "maestro-tile-remove",
                title: Some(title),
                ..ButtonProps::default()
            },
            &[glyph("X", IconSize::Xs)?.into()],
        )
        .map_err(|error| ToolUiError::native(&error))?;
        control
            .set_attribute("data-remove", "")
            .map_err(|error| ToolUiError::native(&error))?;
        append(&self.shell, &control)?;
        let removal = Rc::clone(&self.removal);
        let callback = Closure::new(move |event: Event| {
            event.stop_propagation();
            let current = removal.borrow().clone();
            if let Some(callback) = current {
                callback();
            }
            Ok(())
        });
        let binding = EventSubscription::new(control.clone().into(), "click", callback)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok((control.into(), binding))
    }
}
/// Open the current record independently of this tile's lifetime.
fn open_binding(target: &Element, opening: &Opening) -> Result<Binding, ToolUiError> {
    let opening = Rc::clone(opening);
    let callback = Closure::new(move |_: Event| {
        let (attachment, view) = opening.borrow().clone();
        AttachmentOverlay::open(&view, attachment, None)
            .map(|_| ())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    });
    EventSubscription::new(target.clone().into(), "click", callback)
        .map_err(|error| ToolUiError::native(&error))
}
/// Write an attribute only when it differs, so an unchanged image source does not reload.
fn set(element: &Element, name: &str, value: &str) -> Result<(), ToolUiError> {
    if element.get_attribute(name).as_deref() != Some(value) {
        element
            .set_attribute(name, value)
            .map_err(|error| ToolUiError::native(&error))?;
    }
    Ok(())
}
/// Resolve native glyphs through the shared catalog and SVG renderer.
fn glyph(name: &str, size: IconSize) -> Result<Element, ToolUiError> {
    let icon = crate::icons::icon_node(name)
        .map_err(|error| ToolUiError::Presentation(error.to_string()))?
        .ok_or_else(|| ToolUiError::Presentation(format!("Missing glyph: {name}")))?;
    crate::icons::icon_dom(&icon, size, None)
        .map(Into::into)
        .map_err(|error| ToolUiError::native(&error))
}
