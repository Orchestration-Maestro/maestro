//! The extension interface over the protocol: dialogs are requests answered by a matching
//! response, notices are requests nobody answers, and the terminal-only operations keep
//! their headless defaults.
use super::RpcRecord;
use super::connection::{Connection, write_to};
use crate::rpc_types::{RpcExtensionUIMethod, RpcExtensionUIRequest, RpcExtensionUIResponse};
use futures_util::future::OptionFuture;
use maestro_extensions::EventError;
use maestro_extensions::types::{
    ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionUIFuture, ExtensionWidgetOptions,
    NotificationType, ThemeSelection, ThemeSetResult, WidgetContent,
};
use maestro_models::Cancellation;
use maestro_theme::LiveTheme;
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::Duration,
};
use tokio::{sync::oneshot, time::Instant};

/// A dialog's settled answer: its default, or the client's response.
enum Answer {
    /// The client dismissed the dialog, or it was aborted or expired before the answer came.
    Cancelled,
    /// The client answered.
    Response(RpcExtensionUIResponse),
}

/// A pending dialog: where its answer goes and what settles it without one.
struct Waiting {
    /// Receives the answer.
    answer: oneshot::Sender<Result<Answer, EventError>>,
    /// Aborts the dialog.
    signal: Option<Cancellation>,
    /// When the dialog expires.
    deadline: Option<Instant>,
}

/// The pending dialogs, by request identifier.
type PendingDialogs = Rc<RefCell<HashMap<String, Waiting>>>;

/// The extension interface of one connection, shared by every session it binds.
pub(super) struct RpcUi {
    /// The connection that writes the requests.
    connection: Weak<Connection>,
    /// The dialogs waiting for an answer.
    pending: PendingDialogs,
    /// The live theme.
    theme: LiveTheme,
}

impl RpcUi {
    /// An interface writing through `connection` that reads `theme`.
    pub(super) fn new(connection: Weak<Connection>, theme: LiveTheme) -> Self {
        Self {
            connection,
            pending: Rc::default(),
            theme,
        }
    }

    /// Settles the pending dialog `record` answers; a record naming no pending dialog is
    /// ignored. An abort or deadline already passed, or `cancelled: true`, settles the
    /// default; otherwise each present member must have its kind.
    pub(super) fn answer(&self, record: &Value) {
        let Some(id) = record.get("id").and_then(Value::as_str) else {
            return;
        };
        let Some(waiting) = self.pending.borrow_mut().remove(id) else {
            return;
        };
        let aborted = waiting
            .signal
            .as_ref()
            .is_some_and(Cancellation::is_aborted);
        let expired = waiting
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline);
        let answer = if aborted || expired || record.get("cancelled") == Some(&Value::Bool(true)) {
            Ok(Answer::Cancelled)
        } else {
            RpcExtensionUIResponse::deserialize(record)
                .map(Answer::Response)
                .map_err(EventError::from)
        };
        drop(waiting.answer.send(answer));
    }

    /// Writes `method` as the request `id`.
    fn send(&self, id: String, method: RpcExtensionUIMethod) -> Result<(), EventError> {
        let request = RpcExtensionUIRequest { id, method };
        write_to(&self.connection, RpcRecord::ExtensionUiRequest(&request))
            .map_err(EventError::from)
    }

    /// Settles every pending dialog as cancelled.
    pub(super) fn cancel_pending(&self) {
        for (_, waiting) in self.pending.borrow_mut().drain() {
            drop(waiting.answer.send(Ok(Answer::Cancelled)));
        }
    }

    /// Writes `method` as a request nobody answers.
    fn notice(&self, method: RpcExtensionUIMethod) -> Result<(), EventError> {
        self.send(uuid::Uuid::new_v4().to_string(), method)
    }

    /// Writes `method` and settles with the picked answer, or with `default` once `signal`
    /// aborts, `timeout` milliseconds pass or the client dismisses it. While an ending
    /// connection still waits for its initial binding, it settles with `default` without
    /// asking.
    fn dialog<T: 'static>(
        &self,
        method: RpcExtensionUIMethod,
        options: Option<ExtensionUIDialogOptions>,
        default: T,
        pick: fn(RpcExtensionUIResponse) -> T,
    ) -> ExtensionUIFuture<T> {
        let ExtensionUIDialogOptions { signal, timeout } = options.unwrap_or_default();
        let unanswerable = self
            .connection
            .upgrade()
            .is_some_and(|connection| !connection.admits_dialogs());
        if unanswerable || signal.as_ref().is_some_and(Cancellation::is_aborted) {
            return Box::pin(std::future::ready(Ok(default)));
        }
        let deadline = expiry(timeout).map(|duration| Instant::now() + duration);
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, answer) = oneshot::channel();
        let waiting = Waiting {
            answer: sender,
            signal: signal.clone(),
            deadline,
        };
        self.pending.borrow_mut().insert(id.clone(), waiting);
        if let Err(error) = self.send(id.clone(), method) {
            self.pending.borrow_mut().remove(&id);
            return Box::pin(std::future::ready(Err(error)));
        }
        let pending = Rc::clone(&self.pending);
        Box::pin(async move {
            let (has_signal, has_deadline) = (signal.is_some(), deadline.is_some());
            let aborted = OptionFuture::from(signal.as_ref().map(Cancellation::cancelled));
            let expired = OptionFuture::from(deadline.map(tokio::time::sleep_until));
            // An answer already sent came first: `answer` decided against the abort and
            // the deadline when it arrived.
            let settled = tokio::select! {
                biased;
                answer = answer => match answer {
                    Ok(answer) => answer.map(|answer| match answer {
                        Answer::Cancelled => default,
                        Answer::Response(response) => pick(response),
                    }),
                    // The connection is gone, so no answer can arrive.
                    Err(_) => std::future::pending().await,
                },
                _ = aborted, if has_signal => Ok(default),
                _ = expired, if has_deadline => Ok(default),
            };
            pending.borrow_mut().remove(&id);
            settled
        })
    }
}

/// How long a dialog waits: a zero or absent timeout waits forever, a negative one not at all.
fn expiry(timeout: Option<f64>) -> Option<Duration> {
    let milliseconds =
        timeout.filter(|milliseconds| *milliseconds != 0.0 && !milliseconds.is_nan())?;
    Some(Duration::try_from_secs_f64(milliseconds.max(0.0) / 1000.0).unwrap_or(Duration::MAX))
}

impl ExtensionUIContextPort for RpcUi {
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let timeout = opts.as_ref().and_then(|opts| opts.timeout);
        let method = RpcExtensionUIMethod::Select {
            title,
            options,
            timeout,
        };
        self.dialog(method, opts, None, |response| response.value)
    }

    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let timeout = opts.as_ref().and_then(|opts| opts.timeout);
        let method = RpcExtensionUIMethod::Confirm {
            title,
            message,
            timeout,
        };
        self.dialog(method, opts, false, |response| {
            response.confirmed.unwrap_or(false)
        })
    }

    fn input(
        &self,
        title: String,
        placeholder: Option<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let timeout = opts.as_ref().and_then(|opts| opts.timeout);
        let method = RpcExtensionUIMethod::Input {
            title,
            placeholder,
            timeout,
        };
        self.dialog(method, opts, None, |response| response.value)
    }

    fn editor(&self, title: String, prefill: Option<String>) -> ExtensionUIFuture<Option<String>> {
        let method = RpcExtensionUIMethod::Editor { title, prefill };
        self.dialog(method, None, None, |response| response.value)
    }

    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.notice(RpcExtensionUIMethod::Notify {
            message,
            notify_type: kind,
        })
    }

    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        self.notice(RpcExtensionUIMethod::SetStatus {
            status_key: key,
            status_text: text,
        })
    }

    fn set_widget(
        &self,
        key: String,
        content: Option<WidgetContent>,
        options: Option<ExtensionWidgetOptions>,
    ) -> Result<(), EventError> {
        let lines = match content {
            None => None,
            Some(WidgetContent::Lines(lines)) => Some(lines),
            // Component factories need a terminal.
            Some(WidgetContent::Factory(_)) => return Ok(()),
        };
        self.notice(RpcExtensionUIMethod::SetWidget {
            widget_key: key,
            widget_lines: lines,
            widget_placement: options.and_then(|options| options.placement),
        })
    }

    fn set_title(&self, title: String) -> Result<(), EventError> {
        self.notice(RpcExtensionUIMethod::SetTitle { title })
    }

    fn paste_to_editor(&self, text: String) -> Result<(), EventError> {
        self.set_editor_text(text)
    }

    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        self.notice(RpcExtensionUIMethod::SetEditorText { text })
    }

    fn theme(&self) -> LiveTheme {
        self.theme.clone()
    }

    fn set_theme(&self, _theme: ThemeSelection) -> Result<ThemeSetResult, EventError> {
        Ok(ThemeSetResult {
            success: false,
            error: Some("Theme switching not supported in RPC mode".into()),
        })
    }
}
