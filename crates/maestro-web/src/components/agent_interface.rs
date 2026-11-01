//! The session view: transcript, streaming response, message editor and usage totals.
use crate::components::message_editor::EditorControls;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{
    Agent, AgentMessage, CustomAgentMessages, DiagnosticErrorInfo, Local, NoCustomMessages, Usage,
};
use std::{future::Future, pin::Pin, rc::Rc};

/// The browser-local conversation a view shows.
pub type BrowserSession<C = NoCustomMessages> = Agent<BrowserMessages<C>, Local>;
/// A pending local operation.
pub type LocalFuture<T> = Pin<Box<dyn Future<Output = T>>>;
/// Ask for a provider's key; resolves `true` once one is stored.
pub type ApiKeyRequired = Rc<dyn Fn(String) -> LocalFuture<bool>>;
/// Run before a send; a failure keeps the draft and nothing is sent.
pub type BeforeSend = Rc<dyn Fn() -> LocalFuture<Result<(), DiagnosticErrorInfo>>>;

/// View operands; the callbacks in effect are the most recently supplied ones.
pub struct AgentInterfaceProps<C: CustomAgentMessages = NoCustomMessages> {
    /// The conversation shown; replacing it moves the subscription to the new one.
    pub session: Option<Rc<BrowserSession<C>>>,
    /// The editor controls offered.
    pub controls: EditorControls,
    /// Show the appearance button beside the usage totals.
    pub show_theme_toggle: bool,
    /// Ask for a missing key before sending.
    pub on_api_key_required: Option<ApiKeyRequired>,
    /// Run before each admitted send.
    pub on_before_send: Option<BeforeSend>,
    /// Make the usage totals an action.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
    /// Replace the built-in model picker.
    pub on_model_select: Option<Rc<dyn Fn()>>,
}
impl<C: CustomAgentMessages> Default for AgentInterfaceProps<C> {
    fn default() -> Self {
        Self {
            session: None,
            controls: EditorControls::default(),
            show_theme_toggle: false,
            on_api_key_required: None,
            on_before_send: None,
            on_cost_click: None,
            on_model_select: None,
        }
    }
}
impl<C: CustomAgentMessages> Clone for AgentInterfaceProps<C> {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            controls: self.controls,
            show_theme_toggle: self.show_theme_toggle,
            on_api_key_required: self.on_api_key_required.clone(),
            on_before_send: self.on_before_send.clone(),
            on_cost_click: self.on_cost_click.clone(),
            on_model_select: self.on_model_select.clone(),
        }
    }
}

/// Whether the transcript follows new content, from the reader's scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    /// Whether new content scrolls into view.
    pub enabled: bool,
    /// The last observed scroll offset.
    last_top: f64,
    /// The last observed viewport height.
    last_client: f64,
}
impl Default for Follow {
    fn default() -> Self {
        Self {
            enabled: true,
            last_top: 0.0,
            last_client: 0.0,
        }
    }
}
impl Follow {
    /// Apply one scroll observation.
    ///
    /// A shrinking viewport is a relayout, not reading: it only records the new height. Otherwise
    /// scrolling up from a nonzero offset more than 50 px from the bottom stops following, and
    /// coming within 10 px of the bottom resumes it.
    pub fn scrolled(&mut self, top: f64, scroll_height: f64, client_height: f64) {
        if client_height < self.last_client {
            self.last_client = client_height;
            return;
        }
        let distance = scroll_height - top - client_height;
        if top != 0.0 && top < self.last_top && distance > 50.0 {
            self.enabled = false;
        } else if distance < 10.0 {
            self.enabled = true;
        }
        self.last_top = top;
        self.last_client = client_height;
    }
}

/// The summed usage of every assistant message, or `None` when no tokens were counted.
#[must_use]
pub fn usage_totals<C: CustomAgentMessages>(messages: &[AgentMessage<C>]) -> Option<Usage> {
    let mut totals = Usage::default();
    for message in messages {
        if let AgentMessage::Assistant(assistant) = message {
            let assistant = assistant
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let usage = &assistant.usage;
            totals.input += usage.input;
            totals.output += usage.output;
            totals.cache_read += usage.cache_read;
            totals.cache_write += usage.cache_write;
            totals.cost.total += usage.cost.total;
        }
    }
    [
        totals.input,
        totals.output,
        totals.cache_read,
        totals.cache_write,
    ]
    .iter()
    .any(|count| *count != 0.0 && !count.is_nan())
    .then_some(totals)
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod send;
#[cfg(target_arch = "wasm32")]
mod session;
#[cfg(target_arch = "wasm32")]
pub use browser::AgentInterface;

#[cfg(test)]
mod tests;
