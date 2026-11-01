//! Native editor nodes, rendering and the text, key and control listeners.
use super::{
    MessageEditorProps, THINKING_LEVELS, can_send,
    controls::{icon_button, level_menu, model_control, send_control},
    level_value,
};
use crate::{
    AttachmentTile, SharedAttachment,
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
        timer::{Timer, frame},
    },
    dom::{DomView, MountedContent, icon, report},
    tools::types::ToolUiError,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{
    Element, Event, HtmlButtonElement, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement,
    KeyboardEvent, Node,
};

/// One retained native listener.
pub(super) type Listener = EventSubscription<Closure<dyn Fn(Event)>>;

/// The retained editor; dropping it releases its listeners, its pending frame and its tiles.
pub struct MessageEditor {
    /// Shared state the listeners reach weakly.
    state: Rc<Editor>,
}
/// Nodes and editor state retained across updates.
pub(super) struct Editor {
    /// Page operands.
    pub(super) view: DomView,
    /// Last supplied operands.
    pub(super) props: RefCell<MessageEditorProps>,
    /// Current text.
    value: RefCell<String>,
    /// Current attachments in display order.
    pub(super) attachments: RefCell<Vec<SharedAttachment>>,
    /// File slots reserved by batches that are still loading.
    pub(super) pending_files: Cell<usize>,
    /// Draft generation, advanced by every text or attachment edit.
    pub(super) revision: Cell<u64>,
    /// Whether files are dragged over the editor.
    pub(super) dragging: Cell<bool>,
    /// Bordered card.
    pub(super) root: Element,
    /// Drop hint shown while dragging.
    overlay: Element,
    /// Attachment tile row.
    tiles: Element,
    /// One mounted tile per attachment.
    tile_roots: RefCell<Vec<MountedContent>>,
    /// Text field.
    pub(super) textarea: HtmlTextAreaElement,
    /// Hidden file chooser.
    pub(super) picker: HtmlInputElement,
    /// Attachment and level controls.
    left: Element,
    /// Model and send or abort controls.
    right: Element,
    /// Attachment control.
    attach: HtmlButtonElement,
    /// Loading indicator replacing the attachment control.
    spinner: Element,
    /// Reasoning level menu.
    thinking: HtmlSelectElement,
    /// Model control.
    model: HtmlButtonElement,
    /// Model identifier on the model control.
    model_label: Element,
    /// Send control.
    send: HtmlButtonElement,
    /// Abort control.
    abort: HtmlButtonElement,
    /// Native listeners, released with the editor.
    listeners: RefCell<Vec<Listener>>,
    /// The pending first focus or model selection frame.
    pending: RefCell<Option<Timer>>,
}
impl MessageEditor {
    /// Build the editor with `props`; it focuses its text field on the first frame it is shown.
    /// # Errors
    /// Returns a localization, icon or native failure.
    pub fn new(view: &DomView, props: MessageEditorProps) -> Result<Self, ToolUiError> {
        crate::dom::install_styles(view).map_err(|error| host_error(&error))?;
        let state = Rc::new(Editor::build(view, props)?);
        state.bind()?;
        state.render()?;
        state.focus_when_shown();
        Ok(Self { state })
    }
    /// The card the caller places.
    #[must_use]
    pub fn root(&self) -> &Element {
        &self.state.root
    }
    /// Apply new operands, keeping the text and the attachments.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn update(&self, props: MessageEditorProps) -> Result<(), ToolUiError> {
        let previous = self.state.props.replace(props);
        drop(previous);
        self.state.render()
    }
    /// The current text.
    #[must_use]
    pub fn value(&self) -> String {
        self.state.value.borrow().clone()
    }
    /// Replace the text without reporting an edit.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn set_value(&self, value: &str) -> Result<(), ToolUiError> {
        self.state.value.replace(value.to_owned());
        self.state
            .revision
            .set(self.state.revision.get().wrapping_add(1));
        self.state.render()
    }
    /// The current attachments.
    #[must_use]
    pub fn attachments(&self) -> Vec<SharedAttachment> {
        self.state.attachments.borrow().clone()
    }
    /// Replace the attachments without reporting a change.
    /// # Errors
    /// Returns a localization, attachment or native failure.
    pub fn set_attachments(&self, attachments: Vec<SharedAttachment>) -> Result<(), ToolUiError> {
        self.state.attachments.replace(attachments);
        self.state
            .revision
            .set(self.state.revision.get().wrapping_add(1));
        self.state.show_tiles()?;
        self.state.render()
    }
    /// The generation of the current text and attachments.
    pub(crate) fn revision(&self) -> u64 {
        self.state.revision.get()
    }
    /// Move keyboard focus to the text field.
    pub fn focus(&self) {
        let _ = self.state.textarea.focus();
    }
}
impl Editor {
    /// Create every node once, in display order.
    fn build(view: &DomView, props: MessageEditorProps) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "maestro-editor")?;
        let overlay = element(document, "div", "maestro-editor-drop")?;
        let hint = element(document, "div", "maestro-editor-drop-text")?;
        append(&overlay, &hint)?;
        let tiles = element(document, "div", "maestro-editor-tiles")?;
        let textarea: HtmlTextAreaElement =
            element(document, "textarea", "maestro-editor-text")?.unchecked_into();
        textarea.set_rows(1);
        let picker: HtmlInputElement =
            element(document, "input", "maestro-editor-picker")?.unchecked_into();
        picker.set_type("file");
        picker.set_multiple(true);
        let bar = element(document, "div", "maestro-editor-bar")?;
        let left = element(document, "div", "maestro-editor-group")?;
        let right = element(document, "div", "maestro-editor-group")?;
        append(&bar, &left)?;
        append(&bar, &right)?;
        for node in [&tiles, textarea.as_ref(), picker.as_ref(), &bar] {
            append(&root, node)?;
        }
        let (model, model_label) = model_control(document)?;
        Ok(Self {
            props: RefCell::new(props),
            value: RefCell::default(),
            attachments: RefCell::default(),
            pending_files: Cell::new(0),
            revision: Cell::new(0),
            dragging: Cell::new(false),
            overlay,
            tiles,
            tile_roots: RefCell::default(),
            attach: icon_button(document, "Paperclip", "maestro-editor-attach")?,
            spinner: icon(document, "Loader2", "maestro-editor-spinner")?,
            thinking: level_menu(view)?,
            model,
            model_label,
            send: send_control(document)?,
            abort: icon_button(document, "Square", "maestro-editor-abort")?,
            textarea,
            picker,
            left,
            right,
            root,
            view: view.clone(),
            listeners: RefCell::default(),
            pending: RefCell::default(),
        })
    }
    /// Register every native listener, holding the editor weakly.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let mut listeners = vec![
            self.listen(self.textarea.as_ref(), "input", |editor, _| editor.edited())?,
            self.listen(self.textarea.as_ref(), "keydown", |editor, event| {
                editor.key_down(event.unchecked_ref());
            })?,
            self.listen(self.attach.as_ref(), "click", |editor, _| {
                editor.picker.click();
            })?,
            self.listen(self.thinking.as_ref(), "change", |editor, _| {
                editor.level_chosen();
            })?,
            self.listen(self.model.as_ref(), "click", |editor, _| {
                editor.choose_model();
            })?,
            self.listen(self.send.as_ref(), "click", |editor, _| editor.send())?,
            self.listen(self.abort.as_ref(), "click", |editor, _| editor.abort())?,
        ];
        listeners.extend(self.bind_files()?);
        self.listeners.replace(listeners);
        Ok(())
    }
    /// One listener on `target` reaching this editor while it exists.
    pub(super) fn listen(
        self: &Rc<Self>,
        target: &Element,
        name: &'static str,
        action: impl Fn(&Rc<Self>, &Event) + 'static,
    ) -> Result<Listener, ToolUiError> {
        let weak: Weak<Self> = Rc::downgrade(self);
        let callback = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if let Some(editor) = weak.upgrade() {
                action(&editor, &event);
            }
        });
        EventSubscription::new(target.clone().into(), name, callback)
            .map_err(|error| host_error(&error).into())
    }
    /// Focus the text field on the next frame if the caller has shown the editor by then.
    fn focus_when_shown(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.schedule(Rc::new(move || {
            if let Some(editor) = weak.upgrade().filter(|editor| editor.root.is_connected()) {
                let _ = editor.textarea.focus();
            }
        }));
    }
    /// Run `action` on the next frame, replacing any pending frame.
    fn schedule(&self, action: Rc<dyn Fn()>) {
        let Some(window) = self.view.document.default_view() else {
            return;
        };
        match frame(window, action) {
            Ok(timer) => {
                let previous = self.pending.replace(Some(timer));
                drop(previous);
            }
            Err(error) => report(Err(host_error(&error).into())),
        }
    }
    /// Keep the typed text and report it.
    fn edited(&self) {
        let value = self.textarea.value();
        self.value.replace(value.clone());
        self.revision.set(self.revision.get().wrapping_add(1));
        let on_input = self.props.borrow().on_input.clone();
        if let Some(on_input) = on_input {
            on_input(value);
        }
        report(self.render());
    }
    /// Enter sends unless Shift is held; Escape aborts while streaming; composition keys do nothing.
    fn key_down(&self, event: &KeyboardEvent) {
        let key = event.key();
        if event.is_composing() || key == "Process" {
            return;
        }
        let streaming = self.props.borrow().is_streaming;
        if key == "Enter" && !event.shift_key() {
            event.prevent_default();
            if !streaming && self.sendable() {
                self.send();
            }
        } else if key == "Escape" && streaming {
            event.prevent_default();
            self.abort();
        }
    }
    /// Whether the current draft can be sent.
    fn sendable(&self) -> bool {
        can_send(
            &self.value.borrow(),
            self.attachments.borrow().len(),
            self.pending_files.get() > 0,
        )
    }
    /// Pass the untrimmed text and the attachments to the current send callback.
    fn send(&self) {
        let on_send = self.props.borrow().on_send.clone();
        if let Some(on_send) = on_send {
            let value = self.value.borrow().clone();
            let attachments = self.attachments.borrow().clone();
            on_send(value, attachments);
        }
    }
    /// Call the current abort callback.
    fn abort(&self) {
        let on_abort = self.props.borrow().on_abort.clone();
        if let Some(on_abort) = on_abort {
            on_abort();
        }
    }
    /// Report the chosen reasoning level.
    fn level_chosen(&self) {
        let chosen = self.thinking.value();
        let Some(level) = THINKING_LEVELS
            .into_iter()
            .find(|level| level_value(level) == chosen)
        else {
            return;
        };
        let on_change = {
            let mut props = self.props.borrow_mut();
            props.thinking_level = level.clone();
            props.on_thinking_change.clone()
        };
        if let Some(on_change) = on_change {
            on_change(level);
        }
    }
    /// Focus the text field so selection returns there, then open model selection next frame.
    fn choose_model(self: &Rc<Self>) {
        let _ = self.textarea.focus();
        let weak = Rc::downgrade(self);
        self.schedule(Rc::new(move || {
            let on_select = weak
                .upgrade()
                .and_then(|editor| editor.props.borrow().on_model_select.clone());
            if let Some(on_select) = on_select {
                on_select();
            }
        }));
    }
    /// Remove the attachment named `id` and report the remaining list.
    pub(super) fn remove_file(self: &Rc<Self>, id: &str) {
        self.attachments.borrow_mut().retain(|attachment| {
            attachment
                .read()
                .map_or(true, |attachment| attachment.id != id)
        });
        report(self.show_tiles());
        self.files_changed();
    }
    /// Report the current attachment list.
    pub(super) fn files_changed(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        let on_change = self.props.borrow().on_files_change.clone();
        if let Some(on_change) = on_change {
            let attachments = self.attachments.borrow().clone();
            on_change(attachments);
        }
    }
    /// Mount one removable tile per attachment, in order.
    pub(super) fn show_tiles(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let attachments = self.attachments.borrow().clone();
        let mut roots = Vec::with_capacity(attachments.len());
        let previous = self.tile_roots.take();
        self.tiles.set_text_content(None);
        drop(previous);
        for attachment in attachments {
            let id = attachment
                .read()
                .map(|record| record.id.clone())
                .unwrap_or_default();
            let tile = AttachmentTile {
                attachment,
                show_delete: true,
                on_delete: Some(removal(Rc::downgrade(self), id)),
            };
            let mut mounted = MountedContent::default();
            mounted.show(&tile, self.tiles.as_ref(), &self.view)?;
            roots.push(mounted);
        }
        self.tile_roots.replace(roots);
        Ok(())
    }
    /// Bring every node in line with the text, the attachments and the operands.
    pub(super) fn render(&self) -> Result<(), ToolUiError> {
        let props = self.props.borrow().clone();
        let i18n = &self.view.i18n;
        let value = self.value.borrow().clone();
        if self.textarea.value() != value {
            self.textarea.set_value(&value);
        }
        self.textarea
            .set_placeholder(&i18n.text("Type a message...")?);
        self.picker.set_accept(&props.accepted_types);
        for (control, label) in [
            (&self.attach, "Attach files"),
            (&self.send, "Send"),
            (&self.abort, "Abort"),
        ] {
            control.set_title(&i18n.text(label)?);
        }
        let dragging = self.dragging.get();
        self.root
            .class_list()
            .toggle_with_force("maestro-editor-dragging", dragging)
            .map_err(|error| host_error(&error))?;
        if let Some(hint) = self.overlay.first_element_child() {
            set_text_if_changed(&hint, &i18n.text("Drop files here")?);
        }
        place(&self.root, &self.overlay, dragging, None)?;
        let attachments = self.attachments.borrow().len();
        place(
            &self.root,
            &self.tiles,
            attachments > 0,
            Some(self.textarea.as_ref()),
        )?;
        self.render_controls(&props);
        Ok(())
    }
    /// Show the controls the operands call for, keeping a control in place while it stays.
    fn render_controls(&self, props: &MessageEditorProps) {
        let processing = self.pending_files.get() > 0;
        let reasoning = props
            .current_model
            .as_ref()
            .is_some_and(|model| model.reasoning);
        let mut left: Vec<&Element> = Vec::new();
        if props.controls.attachments {
            left.push(if processing {
                &self.spinner
            } else {
                &self.attach
            });
        }
        if reasoning && props.controls.thinking {
            self.thinking.set_value(level_value(&props.thinking_level));
            left.push(&self.thinking);
        }
        let mut right: Vec<&Element> = Vec::new();
        if let Some(model) = props
            .current_model
            .as_ref()
            .filter(|_| props.controls.model)
        {
            set_text_if_changed(&self.model_label, &model.id);
            right.push(&self.model);
        }
        if props.is_streaming {
            right.push(&self.abort);
        } else {
            self.send.set_disabled(!self.sendable());
            right.push(&self.send);
        }
        arrange(&self.left, &left);
        arrange(&self.right, &right);
    }
}
/// Remove the attachment named `id` from the editor while it exists.
fn removal(editor: Weak<Editor>, id: String) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if let Some(editor) = editor.upgrade() {
            editor.remove_file(&id);
        }
    })
}
/// Attach `node` to `parent` before `before`, or last, while `shown`; otherwise detach it.
fn place(
    parent: &Element,
    node: &Element,
    shown: bool,
    before: Option<&Node>,
) -> Result<(), ToolUiError> {
    if !shown {
        node.remove();
        return Ok(());
    }
    if node.parent_node().is_none() {
        parent
            .insert_before(node, before)
            .map_err(|error| host_error(&error))?;
    }
    Ok(())
}
/// Make `nodes` the children of `group`, touching it only when they differ.
fn arrange(group: &Element, nodes: &[&Element]) {
    let children = group.child_nodes();
    let same = usize::try_from(children.length()).is_ok_and(|count| count == nodes.len())
        && nodes.iter().enumerate().all(|(index, node)| {
            u32::try_from(index)
                .ok()
                .and_then(|index| children.item(index))
                .is_some_and(|child| child.is_same_node(Some(node)))
        });
    if same {
        return;
    }
    let list = js_sys::Array::new();
    for node in nodes {
        list.push(node);
    }
    group.replace_children_with_node(&list);
}
