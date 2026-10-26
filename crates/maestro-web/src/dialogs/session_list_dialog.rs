//! Saved conversations: open one or delete them after confirmation.
use super::{
    dialog::{DialogHeaderProps, dialog_content, dialog_header},
    dialog_base::DialogBase,
};
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        event_subscription::EventSubscription,
    },
    dom::{DomView, outline_glyph, report},
    format_usage,
    tools::types::ToolUiError,
};
use maestro_app::{SessionMetadata, get_app_storage};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event};

/// Panel width of the session list.
const WIDTH: &str = "min(600px, 90vw)";
/// Panel height of the session list.
const HEIGHT: &str = "min(700px, 90vh)";
/// Milliseconds in a day.
const DAY_MS: f64 = 86_400_000.0;
/// Native listener retained with the list.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// An open session list. Clones share one dialog.
#[derive(Clone)]
pub struct SessionListDialog {
    /// Shared dialog state.
    state: Rc<Listing>,
}
/// Loaded sessions, deletions and the retained list.
struct Listing {
    /// Page operands.
    view: Rc<DomView>,
    /// The shown dialog.
    base: DialogBase,
    /// Receives the chosen session's identifier before the dialog closes.
    on_select: Rc<dyn Fn(String)>,
    /// Receives each deleted identifier when the dialog closes without a choice.
    on_delete: Option<Rc<dyn Fn(String)>>,
    /// Stored sessions in storage order.
    sessions: RefCell<Vec<SessionMetadata>>,
    /// Whether the sessions are loading.
    loading: Cell<bool>,
    /// Successfully deleted identifiers in deletion order.
    deleted_sessions: RefCell<Vec<String>>,
    /// Whether the dialog closes because a session was chosen.
    closed_via_selection: Cell<bool>,
    /// Row column.
    list: Element,
    /// Row activation and delete listeners.
    listeners: RefCell<Vec<Listener>>,
}
impl SessionListDialog {
    /// Open the list and load the stored sessions.
    ///
    /// Choosing a row passes its identifier to `on_select` and closes. Closing without a choice
    /// passes each session deleted meanwhile to `on_delete`.
    /// # Errors
    /// Returns a localization or native failure; nothing is shown then.
    pub fn open(
        view: &Rc<DomView>,
        on_select: Rc<dyn Fn(String)>,
        on_delete: Option<Rc<dyn Fn(String)>>,
    ) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title: view.i18n.text("Sessions")?,
                description: Some(view.i18n.text("Load a previous conversation")?),
                class_name: String::new(),
            },
        )?;
        let list = element(document, "div", "maestro-session-list")?;
        let content = dialog_content(
            document,
            &[header.into(), list.clone().into()],
            "maestro-session-frame",
        )?;
        let sessions = Rc::new(Listing {
            view: Rc::clone(view),
            base: DialogBase::new(Rc::clone(view), WIDTH, HEIGHT),
            on_select,
            on_delete,
            sessions: RefCell::default(),
            loading: Cell::new(true),
            deleted_sessions: RefCell::default(),
            closed_via_selection: Cell::new(false),
            list,
            listeners: RefCell::default(),
        });
        let weak = Rc::downgrade(&sessions);
        sessions.base.set_on_close(Some(Rc::new(move || {
            if let Some(sessions) = weak.upgrade() {
                sessions.closed();
            }
        })));
        sessions.render()?;
        let component: Rc<dyn Any> = Rc::clone(&sessions) as Rc<dyn Any>;
        sessions.base.open(vec![content.into()], Some(component))?;
        sessions.load_sessions();
        Ok(Self { state: sessions })
    }
    /// Close without choosing a session.
    pub fn close(&self) {
        self.state.base.close();
    }
    /// Whether the dialog is shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.base.is_open()
    }
}
impl Listing {
    /// Read the stored sessions, then show them.
    fn load_sessions(self: &Rc<Self>) {
        self.loading.set(true);
        report(self.render());
        wasm_bindgen_futures::spawn_local(loaded(Rc::downgrade(self)));
    }
    /// Show the loaded sessions.
    fn show_sessions(self: &Rc<Self>, sessions: Vec<SessionMetadata>) {
        self.sessions.replace(sessions);
        self.loading.set(false);
        report(self.render());
    }
    /// Record a committed deletion once, notifying immediately if the list already closed.
    fn record_deleted(&self, id: String) {
        let mut deleted = self.deleted_sessions.borrow_mut();
        if deleted.contains(&id) {
            return;
        }
        deleted.push(id.clone());
        drop(deleted);
        if !self.base.is_open()
            && !self.closed_via_selection.get()
            && let Some(on_delete) = self.on_delete.clone()
        {
            on_delete(id);
        }
    }
    /// Show the loading text, the empty text or one row per session.
    fn render(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let i18n = &self.view.i18n;
        self.list.set_text_content(None);
        self.listeners.borrow_mut().clear();
        let sessions = self.sessions.borrow().clone();
        let notice = if self.loading.get() {
            Some(i18n.text("Loading...")?)
        } else if sessions.is_empty() {
            Some(i18n.text("No sessions yet")?)
        } else {
            None
        };
        if let Some(notice) = notice {
            let text = element(&self.view.document, "div", "maestro-session-notice")?;
            text.set_text_content(Some(&notice));
            return append(&self.list, &text).map_err(Into::into);
        }
        for session in &sessions {
            let row = self.row(session)?;
            append(&self.list, &row)?;
        }
        Ok(())
    }
    /// One session's row: title, date, message count, usage and its delete control.
    fn row(self: &Rc<Self>, session: &SessionMetadata) -> Result<Element, ToolUiError> {
        let document = &self.view.document;
        let i18n = &self.view.i18n;
        let row = element(document, "div", "maestro-session-row")?;
        // The details are a native button, so the keyboard reaches each row before its delete control.
        let details = element(document, "button", "maestro-session-details")?;
        details
            .set_attribute("type", "button")
            .map_err(|e| host_error(&e))?;
        let title = element(document, "span", "maestro-session-title")?;
        title.set_text_content(Some(&session.title));
        let date = element(document, "span", "maestro-session-meta")?;
        date.set_text_content(Some(&format_date(
            &self.view,
            &session.last_modified,
            js_sys::Date::now(),
        )?));
        let counts = element(document, "span", "maestro-session-meta")?;
        let count = ryu_js::Buffer::new()
            .format(session.message_count)
            .to_owned();
        let usage = format_usage(Some(&session.usage));
        counts.set_text_content(Some(&format!(
            "{count} {} · {usage}",
            i18n.text("messages")?
        )));
        for part in [&title, &date, &counts] {
            append(&details, part)?;
        }
        let delete = element(document, "button", "maestro-session-delete")?;
        delete
            .set_attribute("type", "button")
            .map_err(|e| host_error(&e))?;
        let label = i18n.text("Delete")?;
        delete
            .set_attribute("title", &label)
            .map_err(|e| host_error(&e))?;
        delete
            .set_attribute("aria-label", &label)
            .map_err(|e| host_error(&e))?;
        append(&delete, &trash(document)?)?;
        append(&row, &details)?;
        append(&row, &delete)?;
        let id = session.id.clone();
        let removed = session.id.clone();
        let mut listeners = self.listeners.borrow_mut();
        listeners.push(self.listener(&row, move |sessions, _| sessions.handle_select(&id))?);
        listeners.push(self.listener(&delete, move |sessions, event| {
            event.stop_propagation();
            sessions.handle_delete(&removed);
        })?);
        Ok(row)
    }
    /// A click listener on `target` reaching this list while it exists.
    fn listener(
        self: &Rc<Self>,
        target: &Element,
        action: impl Fn(&Rc<Self>, &Event) + 'static,
    ) -> Result<Listener, ToolUiError> {
        let weak: Weak<Self> = Rc::downgrade(self);
        let callback = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if let Some(sessions) = weak.upgrade() {
                action(&sessions, &event);
            }
        });
        EventSubscription::new(target.clone().into(), "click", callback)
            .map_err(|e| host_error(&e).into())
    }
    /// Choose a session, then close.
    fn handle_select(&self, id: &str) {
        self.closed_via_selection.set(true);
        (self.on_select)(id.to_owned());
        self.base.close();
    }
    /// Delete a session after confirmation, retaining its notification until the write settles.
    fn handle_delete(self: &Rc<Self>, id: &str) {
        let question = self
            .view
            .i18n
            .text("Delete this session?")
            .unwrap_or_else(|_| "Delete this session?".into());
        let confirmed = self
            .view
            .document
            .default_view()
            .and_then(|window| window.confirm_with_message(&question).ok())
            .unwrap_or(false);
        if confirmed {
            wasm_bindgen_futures::spawn_local(deleted(Rc::clone(self), id.to_owned()));
        }
    }
    /// Report the deletions when the dialog closes without a choice.
    fn closed(&self) {
        if self.closed_via_selection.get() {
            return;
        }
        if let Some(on_delete) = self.on_delete.clone() {
            let deleted = self.deleted_sessions.borrow().clone();
            for id in deleted {
                on_delete(id);
            }
        }
    }
}
/// Load the stored sessions and show them while the list exists.
async fn loaded(sessions: Weak<Listing>) {
    let loaded = match get_app_storage() {
        Ok(storage) => storage.sessions.get_all_metadata().await,
        Err(error) => Err(error),
    };
    let loaded = loaded.unwrap_or_else(|error| {
        web_sys::console::error_2(
            &"Failed to load sessions:".into(),
            &error.to_string().into(),
        );
        Vec::new()
    });
    if let Some(sessions) = sessions.upgrade() {
        sessions.show_sessions(loaded);
    }
}
/// Delete one session's history and metadata, then reload the list.
async fn deleted(sessions: Rc<Listing>, id: String) {
    let result = match get_app_storage() {
        Ok(storage) => storage.sessions.delete_session(&id).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            sessions.record_deleted(id);
            if sessions.base.is_open() {
                loaded(Rc::downgrade(&sessions)).await;
            }
        }
        Err(error) => web_sys::console::error_2(
            &"Failed to delete session:".into(),
            &error.to_string().into(),
        ),
    }
}
/// Today, yesterday, a number of days ago within a week, otherwise the browser's local date.
///
/// A time later than `now` counts as today.
pub(crate) fn format_date(view: &DomView, iso: &str, now: f64) -> Result<String, ToolUiError> {
    let date = js_sys::Date::new(&iso.into());
    let days = ((now - date.get_time()) / DAY_MS).floor();
    let i18n = &view.i18n;
    Ok(if days < 1.0 {
        i18n.text("Today")?
    } else if days < 2.0 {
        i18n.text("Yesterday")?
    } else if days < 7.0 {
        i18n.text("{days} days ago")?
            .replace("{days}", ryu_js::Buffer::new().format(days))
    } else {
        crate::browser::invoke0(&date, "toLocaleDateString")
            .ok()
            .and_then(|text| text.as_string())
            .unwrap_or_default()
    })
}
/// The delete glyph: a bin with its lid.
fn trash(document: &web_sys::Document) -> Result<Element, ToolUiError> {
    outline_glyph(
        document,
        "16",
        &[
            ("path", &[("d", "M3 6h18")]),
            ("path", &[("d", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6")]),
            ("path", &[("d", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2")]),
        ],
    )
}
