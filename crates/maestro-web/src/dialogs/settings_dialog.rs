//! The settings dialog and its built-in tabs.
mod tabs;

use super::dialog::{DialogHeaderProps, dialog_content, dialog_header};
use super::dialog_base::DialogBase;
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        event_subscription::EventSubscription,
    },
    dom::{DomContent, DomView, MountedContent},
    tools::types::ToolUiError,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
pub use tabs::{ApiKeysTab, ProxyTab};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, HtmlButtonElement};

/// Panel width of the settings dialog.
const WIDTH: &str = "min(1000px, 90vw)";
/// Panel height of the settings dialog.
const HEIGHT: &str = "min(800px, 90vh)";
/// One settings tab: named page content mounted in the page's own tree.
pub trait SettingsTab: DomContent {
    /// The tab's name, shown in the tab list.
    /// # Errors
    /// Returns a localization failure.
    fn get_tab_name(&self, view: &DomView) -> Result<String, ToolUiError>;
}
/// Native click listener retained with the dialog.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// An open settings dialog. Clones share one dialog.
#[derive(Clone)]
pub struct SettingsDialog {
    /// Shared dialog state.
    state: Rc<Settings>,
}
/// The shown tabs, their retained contents and the active tab.
struct Settings {
    /// The shown dialog.
    base: DialogBase,
    /// Phone tab and sidebar controls of each tab, in caller order.
    controls: Vec<[HtmlButtonElement; 2]>,
    /// Each tab's content slot and mounted content, kept while another tab is shown.
    contents: Vec<(Element, MountedContent)>,
    /// Shown tab.
    active: Cell<usize>,
    /// Tab activations.
    clicks: RefCell<Vec<Click>>,
}
impl SettingsDialog {
    /// Open the dialog with `tabs` in order, the first shown; an empty list opens nothing.
    ///
    /// Closing removes the dialog, then notifies `on_close`.
    /// # Errors
    /// Returns a localization or native failure; nothing is shown then.
    pub fn open(
        view: &Rc<DomView>,
        tabs: &[Rc<dyn SettingsTab>],
        on_close: Option<Rc<dyn Fn()>>,
    ) -> Result<Option<Self>, ToolUiError> {
        if tabs.is_empty() {
            return Ok(None);
        }
        let document = &view.document;
        let mobile = element(document, "div", "maestro-settings-tabs")?;
        let sidebar = element(document, "div", "maestro-settings-sidebar")?;
        let content = element(document, "div", "maestro-settings-content")?;
        let mut controls = Vec::new();
        let mut contents = Vec::new();
        for tab in tabs {
            let name = tab.get_tab_name(view)?;
            let pair = [tab_button(view, &name)?, tab_button(view, &name)?];
            append(&mobile, &pair[0])?;
            append(&sidebar, &pair[1])?;
            controls.push(pair);
            let slot = element(document, "div", "")?;
            let mut mounted = MountedContent::default();
            let shown: &dyn DomContent = &**tab;
            mounted.show(shown, &slot, view)?;
            append(&content, &slot)?;
            contents.push((slot, mounted));
        }
        let settings = Rc::new(Settings {
            base: DialogBase::new(Rc::clone(view), WIDTH, HEIGHT),
            controls,
            contents,
            active: Cell::new(0),
            clicks: RefCell::default(),
        });
        settings.bind()?;
        settings.show();
        settings.base.set_on_close(on_close);
        let root = layout(view, &mobile, &sidebar, &content)?;
        let component: Rc<dyn Any> = Rc::clone(&settings) as Rc<dyn Any>;
        settings.base.open(vec![root.into()], Some(component))?;
        Ok(Some(Self { state: settings }))
    }
    /// Close the dialog.
    pub fn close(&self) {
        self.state.base.close();
    }
    /// Whether the dialog is shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.base.is_open()
    }
}
impl Settings {
    /// Activate a tab from either of its controls.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let mut clicks = self.clicks.borrow_mut();
        for (index, pair) in self.controls.iter().enumerate() {
            for control in pair {
                clicks.push(
                    EventSubscription::new(control.clone().into(), "click", self.activation(index))
                        .map_err(|error| host_error(&error))?,
                );
            }
        }
        Ok(())
    }
    /// A callback showing the tab at `index` while the dialog exists.
    fn activation(self: &Rc<Self>, index: usize) -> Closure<dyn Fn()> {
        let weak: Weak<Self> = Rc::downgrade(self);
        Closure::new(move || {
            if let Some(settings) = weak.upgrade() {
                settings.active.set(index);
                settings.show();
            }
        })
    }
    /// Show the active tab's content and mark its controls, hiding the other contents.
    fn show(&self) {
        let active = self.active.get();
        for (index, ([mobile, sidebar], (slot, _))) in
            self.controls.iter().zip(&self.contents).enumerate()
        {
            let (tab, item, current, display) = if index == active {
                (
                    "maestro-settings-tab maestro-settings-active",
                    "maestro-settings-item maestro-settings-active",
                    "page",
                    "display: block",
                )
            } else {
                (
                    "maestro-settings-tab",
                    "maestro-settings-item",
                    "false",
                    "display: none",
                )
            };
            mobile.set_class_name(tab);
            sidebar.set_class_name(item);
            for control in [mobile, sidebar] {
                let _ = control.set_attribute("aria-current", current);
            }
            let _ = slot.set_attribute("style", display);
        }
    }
}
/// A tab control named `name`.
fn tab_button(view: &DomView, name: &str) -> Result<HtmlButtonElement, ToolUiError> {
    let control: HtmlButtonElement = wasm_bindgen::JsCast::dyn_into(
        view.document
            .create_element("button")
            .map_err(|error| host_error(&error))?,
    )
    .map_err(|_| ToolUiError::Presentation("Created element is not a button".into()))?;
    control.set_type("button");
    control.set_text_content(Some(name));
    Ok(control)
}
/// Heading, phone tabs, then the sidebar beside the contents.
fn layout(
    view: &DomView,
    mobile: &Element,
    sidebar: &Element,
    content: &Element,
) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let frame = element(document, "div", "maestro-settings")?;
    let head = element(document, "div", "maestro-settings-head")?;
    let header = dialog_header(
        document,
        &DialogHeaderProps {
            title: view.i18n.text("Settings")?,
            ..DialogHeaderProps::default()
        },
    )?;
    append(&head, &header)?;
    let body = element(document, "div", "maestro-settings-body")?;
    append(&body, sidebar)?;
    append(&body, content)?;
    for part in [&head, mobile, &body] {
        append(&frame, part)?;
    }
    dialog_content(document, &[frame.into()], "maestro-settings-frame")
}
