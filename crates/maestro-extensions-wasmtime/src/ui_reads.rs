//! Retained UI requests use the extension's original local receiver.
use crate::local_resources::{Lease, Message};
use maestro_extensions::{
    ExtensionUIContext, ExtensionWidgetOptions, WorkingIndicatorOptions,
    types::{NotificationType, WidgetContent},
};
use std::sync::{Arc, mpsc::Sender};

/// Catchable native UI completion.
type Reply = Sender<Result<(), String>>;

/// Canonical UI operands retain absence independently from present empty values.
pub(crate) enum Request {
    /// Displays a notification with the supplied optional severity.
    Notify(String, Option<NotificationType>, Reply),
    /// Sets or clears one status slot.
    Status(String, Option<String>, Reply),
    /// Overrides or restores the working message.
    WorkingMessage(Option<String>, Reply),
    /// Changes working-row visibility.
    WorkingVisible(bool, Reply),
    /// Overrides or restores indicator options.
    WorkingIndicator(Option<WorkingIndicatorOptions>, Reply),
    /// Overrides or restores the hidden-thinking label.
    HiddenThinkingLabel(Option<String>, Reply),
    /// Sets fixed widget lines or removes the widget.
    Widget(
        String,
        Option<Vec<String>>,
        Option<ExtensionWidgetOptions>,
        Reply,
    ),
    /// Sets the terminal title verbatim.
    Title(String, Reply),
}

impl Request {
    /// Calls the original native UI outside all capability-table borrows.
    pub(crate) fn answer(self, ui: &ExtensionUIContext) {
        let (result, reply) = match self {
            Self::Notify(message, kind, reply) => (ui.notify(message, kind), reply),
            Self::Status(key, text, reply) => (ui.set_status(key, text), reply),
            Self::WorkingMessage(message, reply) => (ui.set_working_message(message), reply),
            Self::WorkingVisible(visible, reply) => (ui.set_working_visible(visible), reply),
            Self::WorkingIndicator(options, reply) => (ui.set_working_indicator(options), reply),
            Self::HiddenThinkingLabel(label, reply) => (ui.set_hidden_thinking_label(label), reply),
            Self::Widget(key, lines, options, reply) => (
                ui.set_widget(key, lines.map(WidgetContent::Lines), options),
                reply,
            ),
            Self::Title(title, reply) => (ui.set_title(title), reply),
        };
        let _ = reply.send(result.map_err(|error| error.to_string()));
    }

    /// Settles a released capability without leaving the Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        let (Self::Notify(_, _, reply)
        | Self::Status(_, _, reply)
        | Self::WorkingMessage(_, reply)
        | Self::WorkingVisible(_, reply)
        | Self::WorkingIndicator(_, reply)
        | Self::HiddenThinkingLabel(_, reply)
        | Self::Widget(_, _, _, reply)
        | Self::Title(_, reply)) = self;
        let _ = reply.send(Err(error.to_owned()));
    }
}

/// Store-safe ownership of the originally acquired local UI.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl crate::read_imports::ReadEndpoint for Endpoint {
    type Request = Request;
    fn read<T>(
        &self,
        request: impl FnOnce(Sender<Result<T, String>>) -> Request,
    ) -> Result<T, String> {
        self.0.request(|id, reply| Message::Ui(id, request(reply)))
    }
    fn route(
        &self,
        request: impl FnOnce(Sender<Result<(), String>>) -> Request,
    ) -> crate::import_route::Route {
        self.0.route(|id, reply| Message::Ui(id, request(reply)))
    }
}
