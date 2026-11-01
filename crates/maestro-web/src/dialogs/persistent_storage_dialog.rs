//! Asks before requesting persistent browser storage for saved conversations.
use super::{
    dialog::{DialogHeaderProps, dialog_content, dialog_header},
    dialog_base::DialogBase,
};
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        event_subscription::EventSubscription,
    },
    components::button::{ButtonProps, ButtonVariant, button},
    dom::{DomView, outline_glyph},
    storage::backends::indexeddb_storage_backend::storage_support,
    tools::types::ToolUiError,
};
use maestro_app::get_app_storage;
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
};
use tokio::sync::oneshot;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element};

/// Panel width of the permission dialog.
const WIDTH: &str = "min(500px, 90vw)";
/// Consequences of granting, in shown order.
const MEANINGS: [&str; 4] = [
    "Your conversations will be saved locally in your browser",
    "Data will not be deleted automatically to free up space",
    "You can still manually clear data at any time",
    "No data is sent to external servers",
];
/// Native click listener retained with the dialog.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// Retained state of one open permission dialog.
struct Permission {
    /// The shown dialog.
    base: DialogBase,
    /// The user's answer, taken by the first outcome.
    settle: RefCell<Option<oneshot::Sender<bool>>>,
    /// Continue and Grant activations.
    clicks: RefCell<Vec<Click>>,
}
impl Permission {
    /// Deliver `approved` unless an earlier answer was delivered, then close.
    fn answer(&self, approved: bool) {
        if let Some(settle) = self.settle.borrow_mut().take() {
            let _ = settle.send(approved);
        }
        self.base.close();
    }
}
/// The persistent storage permission request.
pub struct PersistentStorageDialog;
impl PersistentStorageDialog {
    /// Whether the browser keeps this page's storage persistently, asking first when it does not.
    ///
    /// Already persisted storage returns `true` without asking. Continuing without permission or
    /// closing the dialog returns `false`. After approval the application storage requests
    /// persistence; an unavailable platform request, a refusal and a failure all return `false`.
    /// # Errors
    /// Returns a localization or native failure of the dialog.
    pub async fn request(view: &Rc<DomView>) -> Result<bool, ToolUiError> {
        if persisted().await {
            web_sys::console::log_1(&"✓ Persistent storage already granted".into());
            return Ok(true);
        }
        let approved = Self::show(view)?.await.unwrap_or(false);
        if !approved {
            web_sys::console::warn_1(
                &"⚠ User declined persistent storage - sessions may be lost".into(),
            );
            return Ok(false);
        }
        if !matches!(storage_support("persist"), Ok(Some(_))) {
            web_sys::console::warn_1(&"⚠ Persistent storage API not available".into());
            return Ok(false);
        }
        let granted = match get_app_storage() {
            Ok(storage) => storage.request_persistence().await,
            Err(error) => Err(error),
        };
        Ok(match granted {
            Ok(true) => {
                web_sys::console::log_1(
                    &"✓ Persistent storage granted - sessions will be preserved".into(),
                );
                true
            }
            Ok(false) => {
                web_sys::console::warn_1(&"⚠ Browser denied persistent storage - sessions may be lost under storage pressure".into());
                false
            }
            Err(error) => {
                web_sys::console::error_2(
                    &"Failed to request persistent storage:".into(),
                    &error.to_string().into(),
                );
                false
            }
        })
    }
    /// Open the dialog and return the user's answer.
    fn show(view: &Rc<DomView>) -> Result<oneshot::Receiver<bool>, ToolUiError> {
        let document = &view.document;
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title: view.i18n.text("Storage Permission Required")?,
                description: Some(
                    view.i18n
                        .text("This app needs persistent storage to save your conversations")?,
                ),
                class_name: String::new(),
            },
        )?;
        let (sender, receiver) = oneshot::channel();
        let permission = Rc::new(Permission {
            base: DialogBase::new(Rc::clone(view), WIDTH, "auto"),
            settle: RefCell::new(Some(sender)),
            clicks: RefCell::default(),
        });
        let actions = actions(view, &permission)?;
        let content = dialog_content(
            document,
            &[header.into(), explanation(view)?.into(), actions.into()],
            "",
        )?;
        let weak = Rc::downgrade(&permission);
        permission.base.set_on_close(Some(Rc::new(move || {
            if let Some(permission) = weak.upgrade() {
                permission.answer(false);
            }
        })));
        let component: Rc<dyn Any> = Rc::clone(&permission) as Rc<dyn Any>;
        permission
            .base
            .open(vec![content.into()], Some(component))?;
        Ok(receiver)
    }
}
/// Whether the browser reports this page's storage as persisted; an absent API reports no.
async fn persisted() -> bool {
    let Ok(Some(storage)) = storage_support("persisted") else {
        return false;
    };
    match storage.persisted() {
        Ok(promise) => JsFuture::from(promise)
            .await
            .is_ok_and(|value| value.is_truthy()),
        Err(_) => false,
    }
}
/// The warning notice and the list of what granting means.
fn explanation(view: &DomView) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let i18n = &view.i18n;
    let root = element(document, "div", "maestro-storage-body")?;
    let notice = element(document, "div", "maestro-storage-notice")?;
    let sign = element(document, "div", "maestro-storage-sign")?;
    append(&sign, &warning(document)?)?;
    let text = element(document, "div", "maestro-storage-text")?;
    let why = element(document, "p", "maestro-storage-why")?;
    why.set_text_content(Some(&i18n.text("Why is this needed?")?));
    let reason = element(document, "p", "maestro-settings-muted")?;
    reason.set_text_content(Some(&i18n.text("Without persistent storage, your browser may delete saved conversations when it needs disk space. Granting this permission ensures your chat history is preserved.")?));
    append(&text, &why)?;
    append(&text, &reason)?;
    append(&notice, &sign)?;
    append(&notice, &text)?;
    let meaning = element(document, "div", "maestro-settings-muted")?;
    let lead = element(document, "p", "")?;
    lead.set_text_content(Some(&i18n.text("What this means:")?));
    let list = element(document, "ul", "maestro-storage-list")?;
    for line in MEANINGS {
        let item = element(document, "li", "")?;
        item.set_text_content(Some(&i18n.text(line)?));
        append(&list, &item)?;
    }
    append(&meaning, &lead)?;
    append(&meaning, &list)?;
    append(&root, &notice)?;
    append(&root, &meaning)?;
    Ok(root)
}
/// Continue Anyway and Grant Permission, retained with `permission`.
fn actions(view: &DomView, permission: &Rc<Permission>) -> Result<Element, ToolUiError> {
    let row = element(&view.document, "div", "maestro-storage-actions")?;
    for (label, variant, approved) in [
        ("Continue Anyway", ButtonVariant::Outline, false),
        ("Grant Permission", ButtonVariant::Default, true),
    ] {
        let control = button(
            &view.document,
            &ButtonProps {
                variant,
                ..ButtonProps::default()
            },
            &[],
        )
        .map_err(|error| host_error(&error))?;
        control.set_text_content(Some(&view.i18n.text(label)?));
        let weak: Weak<Permission> = Rc::downgrade(permission);
        let click = Closure::<dyn Fn()>::new(move || {
            if let Some(permission) = weak.upgrade() {
                permission.answer(approved);
            }
        });
        permission.clicks.borrow_mut().push(
            EventSubscription::new(control.clone().into(), "click", click)
                .map_err(|error| host_error(&error))?,
        );
        append(&row, &control)?;
    }
    Ok(row)
}
/// The warning sign.
fn warning(document: &Document) -> Result<Element, ToolUiError> {
    outline_glyph(
        document,
        "20",
        &[
            (
                "path",
                &[(
                    "d",
                    "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z",
                )],
            ),
            (
                "line",
                &[("x1", "12"), ("y1", "9"), ("x2", "12"), ("y2", "13")],
            ),
            (
                "line",
                &[("x1", "12"), ("y1", "17"), ("x2", "12.01"), ("y2", "17")],
            ),
        ],
    )
}
