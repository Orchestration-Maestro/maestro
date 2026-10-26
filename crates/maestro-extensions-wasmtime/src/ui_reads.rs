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
    /// Sends literal text through the native editor's paste handling.
    PasteToEditor(String, Reply),
    /// Replaces the original editor's text.
    SetEditorText(String, Reply),
    /// Reads the original editor's current text.
    GetEditorText(Sender<Result<String, String>>),
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
            Self::PasteToEditor(text, reply) => (ui.paste_to_editor(text), reply),
            Self::SetEditorText(text, reply) => (ui.set_editor_text(text), reply),
            Self::GetEditorText(reply) => {
                let _ = reply.send(ui.get_editor_text().map_err(|error| error.to_string()));
                return;
            }
        };
        let _ = reply.send(result.map_err(|error| error.to_string()));
    }

    /// Settles a released capability without leaving the Store caller waiting.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::GetEditorText(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Notify(_, _, reply)
            | Self::Status(_, _, reply)
            | Self::WorkingMessage(_, reply)
            | Self::WorkingVisible(_, reply)
            | Self::WorkingIndicator(_, reply)
            | Self::HiddenThinkingLabel(_, reply)
            | Self::Widget(_, _, _, reply)
            | Self::Title(_, reply)
            | Self::PasteToEditor(_, reply)
            | Self::SetEditorText(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

/// Settles one dialog with its own answer type.
type Answer<T> = tokio::sync::oneshot::Sender<Result<T, String>>;

/// One dialog on the original UI, with the reply its answer settles.
pub(crate) enum Dialog {
    /// Shows a selector over ordered literal choices.
    Select(String, Vec<String>, Answer<Option<String>>),
    /// Asks for confirmation of a literal message.
    Confirm(String, String, Answer<bool>),
    /// Reads one line with an optional placeholder.
    Input(String, Option<String>, Answer<Option<String>>),
    /// Reads several lines with optional prefilled text.
    Editor(String, Option<String>, Answer<Option<String>>),
}

/// A dialog with its independently retained cancellation and unchanged timeout.
pub(crate) struct DialogRequest {
    /// The dialog and its reply.
    pub(crate) dialog: Dialog,
    /// Original native signal, resolved on the local owner.
    pub(crate) signal: Option<crate::local_resources::SignalEndpoint>,
    /// Caller milliseconds.
    pub(crate) timeout: Option<f64>,
}

impl DialogRequest {
    /// Starts the dialog on the original UI and settles its reply when it ends.
    pub(crate) fn start(
        self,
        ui: &ExtensionUIContext,
        signal: Option<maestro_extensions::Cancellation>,
    ) -> crate::local_resources::Work {
        let opts = Some(maestro_extensions::ExtensionUIDialogOptions {
            signal,
            timeout: self.timeout,
        });
        match self.dialog {
            Dialog::Select(title, options, reply) => settle(ui.select(title, options, opts), reply),
            Dialog::Confirm(title, message, reply) => {
                settle(ui.confirm(title, message, opts), reply)
            }
            Dialog::Input(title, placeholder, reply) => {
                settle(ui.input(title, placeholder, opts), reply)
            }
            Dialog::Editor(title, prefill, reply) => settle(ui.editor(title, prefill), reply),
        }
    }

    /// Settles the dialog with `error` without starting it.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        let _ = match self.dialog {
            Dialog::Select(.., reply) | Dialog::Input(.., reply) | Dialog::Editor(.., reply) => {
                reply.send(Err(error)).is_ok()
            }
            Dialog::Confirm(.., reply) => reply.send(Err(error)).is_ok(),
        };
    }
}

/// Settles `reply` with the dialog's answer or the host's message.
fn settle<T: 'static>(
    dialog: maestro_extensions::types::ExtensionUIFuture<T>,
    reply: Answer<T>,
) -> crate::local_resources::Work {
    Box::pin(async move {
        let _ = reply.send(dialog.await.map_err(|error| error.to_string()));
    })
}

/// Store-safe ownership of the originally acquired local UI.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Admits one dialog on the original local owner and waits for its answer.
    pub(crate) async fn dialog<T>(
        &self,
        dialog: impl FnOnce(Answer<T>) -> Dialog,
        signal: Option<crate::local_resources::SignalEndpoint>,
        timeout: Option<f64>,
    ) -> Result<T, String> {
        self.0
            .request_async(|id, reply, admission| {
                let request = DialogRequest {
                    dialog: dialog(reply),
                    signal,
                    timeout,
                };
                Message::Dialog(id, request, admission)
            })
            .await
    }
}

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
