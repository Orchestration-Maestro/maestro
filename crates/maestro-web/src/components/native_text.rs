//! Retained native captions and subsection labels.
use crate::tools::types::ToolUiError;
use crate::{
    browser::attachment_content::{
        self, AttachmentHost, AttachmentHostError, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    tools::types::ToolRenderView,
};
use std::{cell::RefCell, rc::Rc};

/// Current native action, replaced independently of the mounted text.
type TextAction = Rc<RefCell<TextActionState>>;
/// Live action and its repaint owner, never retained by native event targets.
#[derive(Default)]
struct TextActionState {
    /// Current caller-owned action.
    callback: Option<Rc<dyn Fn()>>,
    /// Drawing context to wake after native activation.
    context: Option<egui::Context>,
    /// Whether this nested action consumes the native event.
    consuming: bool,
}

/// Typography and selector of a tool's literal text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextKind {
    /// Shared header caption.
    Caption,
    /// Natural-width label within a rich header.
    InlineCaption,
    /// Literal artifact filename with an optional consuming action.
    Pill(bool),
    /// Literal section summary with native activation.
    Summary,
    /// Input or output label.
    Label,
    /// Normal-weight small detail text.
    Detail,
    /// Muted URL text with a bold literal label.
    Url(usize),
    /// Compact usage, optionally interactive.
    Usage(bool),
    /// Reasoning disclosure, optionally shimmering.
    Thinking(bool),
    /// Literal destructive abort text.
    Abort,
    /// Literal error text with a bold prefix of this byte length.
    Error(usize),
}
/// One native text lifetime beneath the caller's row state.
pub(crate) struct NativeText {
    /// Platform registry owning the published root.
    host: Rc<dyn AttachmentHost>,
    /// Live callback owned by the row, not by detached native targets.
    action: TextAction,
    /// Stable feature identity.
    id: egui::Id,
    /// Last successfully published text and typography.
    shown: Option<(String, TextKind)>,
    /// Published factory identity.
    content: Option<Rc<ReadingContent>>,
    /// Native caption target, captured weakly by factories.
    #[cfg(target_arch = "wasm32")]
    node: Rc<std::cell::RefCell<Option<web_sys::Element>>>,
}
impl NativeText {
    /// Bind a literal text owner to the supplied platform.
    pub(crate) fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            action: Rc::default(),
            id: attachment_content::identity(),
            shown: None,
            content: None,
            #[cfg(target_arch = "wasm32")]
            node: Rc::default(),
        }
    }
    /// Apply interaction color only to this literal caption, never rich descendants.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn set_hovered(&self, hovered: bool) -> Result<(), AttachmentHostError> {
        if let Some(node) = self.node.borrow().as_ref() {
            node.class_list()
                .toggle_with_force("maestro-text-hover", hovered)
                .map_err(|error| attachment_content::browser::host_error(&error))?;
        }
        Ok(())
    }
    /// Replace the current action without a guard spanning user-owned release.
    pub(crate) fn set_action(&mut self, action: Option<Rc<dyn Fn()>>) {
        let old = std::mem::replace(&mut self.action.borrow_mut().callback, action);
        drop(old);
    }
    /// Stable native identity for the containing layout scope.
    pub(crate) fn id(&self) -> egui::Id {
        self.id
    }
    /// Measure and publish literal text through the existing transaction.
    pub(crate) fn render(
        &mut self,
        ui: &mut egui::Ui,
        text: (&str, TextKind),
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        self.action.borrow_mut().context = Some(ui.ctx().clone());
        self.action.borrow_mut().consuming = matches!(text.1, TextKind::Pill(true));
        let changed = self
            .shown
            .as_ref()
            .is_none_or(|shown| shown.0 != text.0 || shown.1 != text.1);
        let candidate = changed.then(|| content(text.0, text.1, self)).transpose()?;
        let content = candidate
            .as_ref()
            .or(self.content.as_ref())
            .ok_or_else(|| {
                ToolUiError::Host(AttachmentHostError::Host("Missing tool text".into()))
            })?;
        let layout = ControlledReading {
            intrinsic_width: matches!(text.1, TextKind::Pill(_) | TextKind::InlineCaption),
            height: ReadingHeight::Flow,
            host: self.host.as_ref(),
            id: self.id,
            content,
            sizes: &[],
            modal: view.modal,
            presentation: view.presentation,
        }
        .draw(ui)?;
        if let Some(content) = candidate {
            self.content = Some(content);
            self.shown = Some((text.0.into(), text.1));
        }
        Ok(layout.response)
    }
}
impl Drop for NativeText {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Build a literal factory without retaining mounted owners or frame operands.
fn content(
    text: &str,
    kind: TextKind,
    owner: &NativeText,
) -> Result<Rc<ReadingContent>, ToolUiError> {
    let icon = matches!(kind, TextKind::Pill(_))
        .then(|| attachment_content::glyph_node("FileCode2"))
        .transpose()
        .map_err(ToolUiError::Control)?;
    #[cfg(target_arch = "wasm32")]
    {
        let text = text.to_owned();
        let target = Rc::downgrade(&owner.node);
        let action = Rc::downgrade(&owner.action);
        Ok(Rc::new(ReadingContent {
            mount: Rc::new(move |document| {
                mount(document, &text, kind, (&target, &action), icon.as_ref())
            }),
        }))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (text, kind, owner, icon);
        Ok(Rc::new(ReadingContent {}))
    }
}
/// Shared native click subscription type.
#[cfg(target_arch = "wasm32")]
type Click = crate::browser::event_subscription::EventSubscription<
    wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
>;
/// Mounted text and its native click subscription share one lifetime.
#[cfg(target_arch = "wasm32")]
struct Mounted {
    /// Native text target.
    node: web_sys::Element,
    /// Bold literal prefix, empty outside error text.
    prefix: web_sys::Element,
    /// Literal text following the prefix.
    body: web_sys::Element,
    /// Detaches before its native callback is released.
    _click: Click,
}
/// Weak targets do not extend the containing row's lifetime.
#[cfg(target_arch = "wasm32")]
type Targets<'a> = (
    &'a std::rc::Weak<RefCell<Option<web_sys::Element>>>,
    &'a std::rc::Weak<RefCell<TextActionState>>,
);
/// Mount one text node; successful edits reconcile into the retained node.
#[cfg(target_arch = "wasm32")]
fn mount(
    document: &web_sys::Document,
    text: &str,
    kind: TextKind,
    targets: Targets<'_>,
    icon: Option<&crate::icons::IconNode>,
) -> Result<attachment_content::ReadingRoot, AttachmentHostError> {
    use attachment_content::browser::{append, element, host_error};
    let root = element(document, "div", "maestro-tool-text")?;
    let style = element(document, "style", "")?;
    style.set_text_content(Some(include_str!("messages.css")));
    append(&root, &style)?;
    let node = element(document, "span", "")?;
    let prefix = element(document, "strong", "")?;
    let body = element(document, "span", "")?;
    if let Some(icon) = icon {
        let icon = crate::icons::icon_dom(icon, crate::icons::IconSize::Sm, None)
            .map_err(|error| host_error(&error))?;
        node.append_child(&icon)
            .map_err(|error| host_error(&error))?;
    }
    append(&node, &prefix)?;
    append(&node, &body)?;
    append(&root, &node)?;
    let mounted = Mounted {
        node: node.clone(),
        prefix,
        body,
        _click: subscribe(&node, targets.1)?,
    };
    update(&mounted, text, kind);
    let text = text.to_owned();
    if let Some(target) = targets.0.upgrade() {
        *target.borrow_mut() = Some(node);
    }
    let target = targets.0.clone();
    Ok(attachment_content::ReadingRoot {
        frame_style: "",
        decoration: None,
        element: root,
        slots: vec![],
        owner: Some(Box::new(mounted)),
        commit: Some(Box::new(move |previous| {
            if let Some(mounted) = previous
                .owner
                .as_ref()
                .and_then(|owner| owner.downcast_ref::<Mounted>())
            {
                update(mounted, &text, kind);
                if let Some(target) = target.upgrade() {
                    *target.borrow_mut() = Some(mounted.node.clone());
                }
            }
        })),
    })
}
/// Native click subscription retains only the weak action slot.
#[cfg(target_arch = "wasm32")]
fn subscribe(
    node: &web_sys::Element,
    action: &std::rc::Weak<RefCell<TextActionState>>,
) -> Result<Click, AttachmentHostError> {
    let action = action.clone();
    let callback = wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::Event| {
        let current = action.upgrade().map(|action| {
            let action = action.borrow();
            (
                action.callback.clone(),
                action.context.clone(),
                action.consuming,
            )
        });
        if let Some((Some(callback), context, consuming)) = current {
            if consuming {
                event.prevent_default();
                event.stop_propagation();
            }
            callback();
            if let Some(context) = context {
                context.request_repaint();
            }
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    crate::browser::event_subscription::EventSubscription::new(
        node.clone().into(),
        "click",
        callback,
    )
    .map_err(|error| attachment_content::browser::host_error(&error))
}
/// Update typography and literal text without interpreting authored markup.
#[cfg(target_arch = "wasm32")]
fn update(mounted: &Mounted, text: &str, kind: TextKind) {
    use attachment_content::browser::set_text_if_changed;
    let class = match kind {
        TextKind::Caption | TextKind::InlineCaption => "maestro-tool-caption",
        TextKind::Pill(false) => "maestro-artifact-pill",
        TextKind::Pill(true) => "maestro-artifact-pill maestro-text-action",
        TextKind::Summary => "maestro-tool-caption maestro-text-action",
        TextKind::Label => "maestro-tool-label",
        TextKind::Detail => "maestro-tool-detail",
        TextKind::Url(_) => "maestro-tool-url",
        TextKind::Usage(false) => "maestro-message-usage",
        TextKind::Usage(true) => "maestro-message-usage maestro-text-action",
        TextKind::Thinking(false) => "maestro-thinking-header maestro-text-action",
        TextKind::Thinking(true) => "maestro-thinking-header maestro-text-action maestro-shimmer",
        TextKind::Abort => "maestro-message-abort",
        TextKind::Error(_) => "maestro-message-error",
    };
    mounted.node.set_class_name(class);
    let prefix = if let TextKind::Error(prefix) | TextKind::Url(prefix) = kind {
        prefix
    } else {
        0
    };
    set_text_if_changed(&mounted.prefix, &text[..prefix]);
    set_text_if_changed(&mounted.body, &text[prefix..]);
}
