//! Native provider-card nodes retained across operand updates.
use super::{CustomProviderCard, ProviderStatus};
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use maestro_app::{AutoDiscoveryProviderType, CustomProviderType, ManualProviderType, Presence};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, HtmlButtonElement};

/// Callback fields in visible action order.
const ACTIONS: [&str; 3] = ["Refresh", "Edit", "Delete"];
/// One native action with its retained event registration.
struct Binding {
    /// Native button, preserved while its action stays present.
    button: HtmlButtonElement,
    /// Event subscription released with this action.
    _click: EventSubscription<Closure<dyn Fn()>>,
}
/// Nodes and activation operands whose identity survives a provider update.
struct Card {
    /// Outer border and content flow.
    root: Element,
    /// Parent retaining the optional status row.
    content: Element,
    /// Literal display name.
    name: Element,
    /// Protocol spelling with native capitalization presentation.
    kind: Element,
    /// Optional untrimmed server URL, including its separator.
    endpoint: Element,
    /// Selected status text, absent when discovery has no result.
    status: Element,
    /// Status glyph with its independent semantic color.
    dot: Element,
    /// Status text sharing the muted reading role.
    status_label: Element,
    /// Native controls in source order.
    actions: Element,
    /// Current callbacks and original provider handle.
    current: RefCell<CustomProviderCard>,
    /// Independently retained action bindings.
    bindings: [RefCell<Option<Binding>>; 3],
}
impl DomContent for CustomProviderCard {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "maestro-provider-card")?;
        let content = element(document, "div", "maestro-provider-content")?;
        let name = element(document, "div", "maestro-provider-name")?;
        let details = element(document, "div", "maestro-provider-details")?;
        let kind = element(document, "span", "maestro-provider-type")?;
        let endpoint = element(document, "span", "maestro-provider-url")?;
        let status = element(document, "div", "maestro-provider-status")?;
        let dot = element(document, "span", "maestro-provider-dot")?;
        let status_label = element(document, "span", "")?;
        append(&status, &dot)?;
        append(&status, &status_label)?;
        let actions = element(document, "div", "maestro-provider-actions")?;
        append(&details, &kind)?;
        append(&details, &endpoint)?;
        for node in [&name, &details, &status] {
            append(&content, node)?;
        }
        append(&root, &content)?;
        append(&root, &actions)?;
        let mounted = DomRoot {
            node: root.clone().into(),
            owner: Some(Box::new(Rc::new(Card {
                root,
                content,
                name,
                kind,
                endpoint,
                status,
                dot,
                status_label,
                actions,
                current: RefCell::new(self.clone()),
                bindings: std::array::from_fn(|_| RefCell::default()),
            }))),
        };
        self.update(&mounted, view)?;
        Ok(mounted)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let card = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<Card>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing provider card".into()))?;
        // Render strings outlive the provider borrow across localization and native callbacks.
        let (name, kind, url) = {
            let provider = self.provider.borrow();
            let url = if provider.base_url.is_empty() {
                String::new()
            } else {
                format!(" • {}", provider.base_url)
            };
            (provider.name.clone(), provider_type(provider.r#type), url)
        };
        let status = self.status_text(view)?;
        let labels = self.labels(view)?;
        set_text_if_changed(&card.name, &name);
        set_text_if_changed(&card.kind, kind);
        set_text_if_changed(&card.endpoint, &url);
        card.show_status(&status, self.status.filter(|_| self.is_auto_discovery));
        if status.is_empty() {
            card.status.remove();
        } else if card.status.parent_node().is_none() {
            append(&card.content, &card.status)?;
        }
        let previous = card.current.replace(self.clone());
        drop(previous);
        for (index, label) in labels.into_iter().enumerate() {
            card.show_action(index, label.as_deref(), view)?;
        }
        Ok(())
    }
}
impl CustomProviderCard {
    /// Prepare only the selected localized status before changing native nodes.
    fn status_text(&self, view: &DomView) -> Result<String, ToolUiError> {
        if !self.is_auto_discovery {
            let label = view.i18n.text("Models")?;
            let count = match &self.provider.borrow().models {
                Presence::Present(models) => models.len(),
                Presence::Missing | Presence::Null => 0,
            };
            return Ok(format!("{label}: {count}"));
        }
        Ok(match self.status {
            None => String::new(),
            Some(ProviderStatus::Connected { model_count }) => {
                format!("{model_count} {}", view.i18n.text("models")?)
            }
            Some(ProviderStatus::Checking) => view.i18n.text("Checking...")?,
            Some(ProviderStatus::Disconnected) => view.i18n.text("Disconnected")?,
        })
    }
    /// Prepare labels only for actions this card actually displays.
    fn labels(&self, view: &DomView) -> Result<[Option<String>; 3], ToolUiError> {
        let visible = [
            self.is_auto_discovery && self.on_refresh.is_some(),
            self.on_edit.is_some(),
            self.on_delete.is_some(),
        ];
        let mut labels = [None, None, None];
        for (index, key) in ACTIONS.into_iter().enumerate() {
            if visible[index] {
                labels[index] = Some(view.i18n.text(key)?);
            }
        }
        Ok(labels)
    }
}
impl Card {
    /// Update only the changed status text and its independently colored glyph.
    fn show_status(&self, status: &str, discovery: Option<ProviderStatus>) {
        let (dot, label, class) = match discovery {
            Some(ProviderStatus::Connected { .. }) => (
                "● ",
                status,
                "maestro-provider-dot maestro-provider-connected",
            ),
            Some(ProviderStatus::Checking) => (
                "● ",
                status,
                "maestro-provider-dot maestro-provider-checking",
            ),
            Some(ProviderStatus::Disconnected) => (
                "● ",
                status,
                "maestro-provider-dot maestro-provider-disconnected",
            ),
            None => ("", status, "maestro-provider-dot"),
        };
        set_text_if_changed(&self.dot, dot);
        set_text_if_changed(&self.status_label, label);
        if self.dot.class_name() != class {
            self.dot.set_class_name(class);
        }
    }
    /// Preserve a present action's button, or release a removed action's listener.
    fn show_action(
        self: &Rc<Self>,
        index: usize,
        label: Option<&str>,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        let Some(label) = label else {
            let retired = self.bindings[index].borrow_mut().take();
            if let Some(retired) = retired {
                retired.button.remove();
            }
            return Ok(());
        };
        let existing = self.bindings[index]
            .borrow()
            .as_ref()
            .map(|binding| binding.button.clone());
        if let Some(button) = existing {
            set_text_if_changed(button.as_ref(), label);
            return Ok(());
        }
        let button = button(
            &view.document,
            &ButtonProps {
                variant: ButtonVariant::Ghost,
                size: ButtonSize::Sm,
                ..ButtonProps::default()
            },
            &[],
        )
        .map_err(|error| host_error(&error))?;
        button.set_text_content(Some(label));
        let weak = Rc::downgrade(self);
        let click = Closure::<dyn Fn()>::new(move || {
            if let Some(card) = weak.upgrade() {
                card.activate(index);
            }
        });
        let click = EventSubscription::new(button.clone().into(), "click", click)
            .map_err(|error| host_error(&error))?;
        let next = self
            .bindings
            .iter()
            .skip(index + 1)
            .find_map(|slot| slot.borrow().as_ref().map(|binding| binding.button.clone()));
        self.actions
            .insert_before(&button, next.as_ref().map(AsRef::as_ref))
            .map_err(|error| host_error(&error))?;
        let previous = self.bindings[index].replace(Some(Binding {
            button,
            _click: click,
        }));
        drop(previous);
        Ok(())
    }
    /// Invoke current operands after releasing all component and provider borrows.
    fn activate(&self, index: usize) {
        if !self.root.is_connected() {
            return;
        }
        let (callback, provider) = {
            let current = self.current.borrow();
            let callback = match index {
                0 if current.is_auto_discovery => current.on_refresh.clone(),
                1 => current.on_edit.clone(),
                2 => current.on_delete.clone(),
                _ => None,
            };
            (callback, Rc::clone(&current.provider))
        };
        if let Some(callback) = callback {
            callback(provider);
        }
    }
}
/// The existing provider protocol's serialized spelling, without display rewriting.
fn provider_type(kind: CustomProviderType) -> &'static str {
    match kind {
        CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Ollama) => "ollama",
        CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::LlamaCpp) => "llama.cpp",
        CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Vllm) => "vllm",
        CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Lmstudio) => "lmstudio",
        CustomProviderType::Manual(ManualProviderType::OpenaiCompletions) => "openai-completions",
        CustomProviderType::Manual(ManualProviderType::OpenaiResponses) => "openai-responses",
        CustomProviderType::Manual(ManualProviderType::AnthropicMessages) => "anthropic-messages",
    }
}
