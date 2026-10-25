//! Extension dialogs that take the editor's place until answered, cancelled, timed out
//! or aborted.
use crate::InteractiveMode;
use crate::components::{
    extension_editor::{ExtensionEditorComponent, ExtensionEditorOptions},
    extension_input::{ExtensionInputComponent, ExtensionInputOptions},
    extension_selector::{ExtensionSelectorComponent, ExtensionSelectorOptions},
};
use maestro_app::{
    Cancellation, ExtensionUIDialogOptions, ExtensionUIFuture, MissingSessionCwdError,
    format_missing_session_cwd_prompt,
};
use maestro_theme::ThemeError;
use maestro_tui::{Component, EditorOptions, TUI, get_keybindings};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use tokio::sync::oneshot;

/// Delivers a dialog's answer, or `None`; only the first delivery counts.
type Settle = Rc<dyn Fn(Option<String>)>;

/// Stops a dialog component's countdown before the editor returns.
type Dispose = Box<dyn Fn()>;

/// A dialog retained until it settles, even while another panel hides it.
struct Dialog {
    /// The component whose settlement this is.
    component: Rc<dyn Component>,
    /// Cancels or answers the component once.
    settle: Settle,
}

/// The latest selector, text input and multi-line editor dialog.
#[derive(Default)]
pub(in super::super) struct DialogSlots {
    /// The latest selector, including confirmations.
    selector: RefCell<Option<Dialog>>,
    /// The latest text input.
    input: RefCell<Option<Dialog>>,
    /// The latest multi-line editor.
    editor: RefCell<Option<Dialog>>,
}

impl DialogSlots {
    /// Cancels each kept dialog that has not settled, as its own cancellation does, so it
    /// answers `None`.
    pub(in super::super) fn cancel_all(&self) {
        for slot in [&self.selector, &self.input, &self.editor] {
            if let Some(dialog) = slot.take() {
                (dialog.settle)(None);
            }
        }
    }

    /// Releases only the slot belonging to `component`, outside the slot's borrow.
    fn release(&self, component: &Rc<dyn Component>) {
        for slot in [&self.selector, &self.input, &self.editor] {
            let matches = slot
                .borrow()
                .as_ref()
                .is_some_and(|dialog| Rc::ptr_eq(&dialog.component, component));
            if matches {
                drop(slot.take());
            }
        }
    }
}

/// A shown dialog that has not settled yet.
struct Open {
    /// Receives the answer.
    answer: oneshot::Sender<Option<String>>,
    /// Ends the abort observer when dropped.
    observed: oneshot::Sender<()>,
    /// Stops the component's countdown, when it has one.
    dispose: Option<Dispose>,
    /// Identifies this dialog without a component-to-settlement ownership cycle.
    component: Option<Weak<dyn Component>>,
}

impl InteractiveMode {
    /// Shows a selector over `options` in place of the editor; resolves with the chosen
    /// option, or `None` on cancellation, timeout or abort.
    pub(in crate::interactive_mode) fn show_extension_selector(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let ExtensionUIDialogOptions { signal, timeout } = opts.unwrap_or_default();
        self.show_dialog(signal, &self.dialogs.selector, |settle| {
            let cancel = Rc::clone(&settle);
            let selector = Rc::new(ExtensionSelectorComponent::new(
                title,
                options,
                Rc::new(move |option: &str| settle(Some(option.to_owned()))),
                Rc::new(move || cancel(None)),
                ExtensionSelectorOptions {
                    tui: Some(TUI::clone(&self.ui)),
                    timeout,
                    runtime: Rc::clone(&self.tool_environment.runtime),
                    theme: self.themes.theme(),
                },
            )?);
            let dispose = disposer(&selector, ExtensionSelectorComponent::dispose);
            Ok((selector, Some(dispose)))
        })
    }

    /// Asks `title` and `message` with the options `Yes` and `No`; only `Yes` is true.
    pub(super) fn show_extension_confirm(
        &self,
        title: &str,
        message: &str,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<bool> {
        let answer = self.show_extension_selector(
            format!("{title}\n{message}"),
            vec!["Yes".to_owned(), "No".to_owned()],
            opts,
        );
        Box::pin(async move { Ok(answer.await?.as_deref() == Some("Yes")) })
    }

    /// Asks whether to continue in the offered directory when the session's own
    /// directory no longer exists; resolves with that directory, unchanged, or `None`.
    pub fn prompt_for_missing_session_cwd(
        &self,
        error: &MissingSessionCwdError,
    ) -> ExtensionUIFuture<Option<String>> {
        let fallback = error.issue.fallback_cwd.clone();
        let confirmed = self.show_extension_confirm(
            "Session cwd not found",
            &format_missing_session_cwd_prompt(&error.issue),
            None,
        );
        Box::pin(async move { Ok(confirmed.await?.then_some(fallback)) })
    }

    /// Shows a text input in place of the editor; resolves with the submitted text, or
    /// `None` on cancellation, timeout or abort.
    pub(super) fn show_extension_input(
        &self,
        title: String,
        placeholder: Option<&str>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let ExtensionUIDialogOptions { signal, timeout } = opts.unwrap_or_default();
        self.show_dialog(signal, &self.dialogs.input, |settle| {
            let cancel = Rc::clone(&settle);
            let input = Rc::new(ExtensionInputComponent::new(
                title,
                placeholder,
                Rc::new(move |value: &str| settle(Some(value.to_owned()))),
                Rc::new(move || cancel(None)),
                ExtensionInputOptions {
                    tui: Some(TUI::clone(&self.ui)),
                    timeout,
                    runtime: Rc::clone(&self.tool_environment.runtime),
                    theme: self.themes.theme(),
                },
            )?);
            let dispose = disposer(&input, ExtensionInputComponent::dispose);
            Ok((input, Some(dispose)))
        })
    }

    /// Shows a multiline editor in place of the editor; resolves with the submitted
    /// text, or `None` on cancellation.
    pub(in crate::interactive_mode) fn show_extension_editor(
        &self,
        title: String,
        prefill: Option<String>,
    ) -> ExtensionUIFuture<Option<String>> {
        self.show_dialog(None, &self.dialogs.editor, |settle| {
            let cancel = Rc::clone(&settle);
            let editor = ExtensionEditorComponent::new(
                &self.ui,
                get_keybindings(),
                title,
                ExtensionEditorOptions {
                    prefill,
                    on_submit: Rc::new(move |value: &str| settle(Some(value.to_owned()))),
                    on_cancel: Rc::new(move || cancel(None)),
                    editor_options: EditorOptions::default(),
                    theme: self.themes.theme(),
                    runtime: Rc::clone(&self.tool_environment.runtime),
                },
            )?;
            Ok((Rc::new(editor), None))
        })
    }

    /// Builds a dialog around its settlement, shows it in place of the editor before
    /// returning and, given a signal, settles it with `None` on abort.
    ///
    /// An already-aborted signal settles at once without showing anything. Settling
    /// stops the countdown and abort observer, restoring the editor only when this
    /// dialog is still shown. The slot retains the component independently of the
    /// returned future; replacing the same kind cancels its previous dialog.
    fn show_dialog(
        &self,
        signal: Option<Cancellation>,
        slot: &RefCell<Option<Dialog>>,
        build: impl FnOnce(Settle) -> Result<(Rc<dyn Component>, Option<Dispose>), ThemeError>,
    ) -> ExtensionUIFuture<Option<String>> {
        if signal.as_ref().is_some_and(Cancellation::is_aborted) {
            return Box::pin(std::future::ready(Ok(None)));
        }
        let (answer, answered) = oneshot::channel();
        let (observed, finished) = oneshot::channel();
        let open = Rc::new(RefCell::new(Some(Open {
            answer,
            observed,
            dispose: None,
            component: None,
        })));
        let settle = self.settlement(Rc::clone(&open));
        let (component, dispose) = match build(Rc::clone(&settle)) {
            Ok(shown) => shown,
            Err(error) => return Box::pin(std::future::ready(Err(error.into()))),
        };
        if let Some(open) = open.borrow_mut().as_mut() {
            open.dispose = dispose;
            open.component = Some(Rc::downgrade(&component));
        }
        let previous = slot.replace(Some(Dialog {
            component: Rc::clone(&component),
            settle: Rc::clone(&settle),
        }));
        if let Some(previous) = previous {
            (previous.settle)(None);
        }
        self.show_panel(Rc::clone(&component), component);
        if let Some(signal) = signal {
            let cancelled = signal.cancelled();
            self.tool_environment
                .runtime
                .spawn_local(Box::pin(async move {
                    tokio::select! {
                        () = cancelled => settle(None),
                        _ = finished => {}
                    }
                    Ok(())
                }));
        }
        Box::pin(async move { Ok(answered.await.ok().flatten()) })
    }

    /// The first-wins settlement of `open`, keeping this mode only weakly.
    fn settlement(&self, open: Rc<RefCell<Option<Open>>>) -> Settle {
        let mode = self.this.clone();
        Rc::new(move |value| {
            let Some(Open {
                answer,
                observed,
                dispose,
                component,
            }) = open.borrow_mut().take()
            else {
                return;
            };
            if let Some(dispose) = dispose {
                dispose();
            }
            if let Some(mode) = mode.upgrade()
                && let Some(component) = component.and_then(|component| component.upgrade())
            {
                mode.hide_extension_dialog(&component);
            }
            drop(observed);
            drop(answer.send(value));
        })
    }

    /// Releases this dialog's slot, restoring the editor only if this component is shown.
    fn hide_extension_dialog(&self, component: &Rc<dyn Component>) {
        self.dialogs.release(component);
        let shown = self
            .editor_container
            .children()
            .borrow()
            .iter()
            .any(|child| Rc::ptr_eq(child, component));
        if !shown {
            return;
        }
        self.restore_editor();
        self.ui.request_render(false);
    }
}

/// Stops the countdown of `component` while it is still shown, without keeping it alive.
fn disposer<T: 'static>(component: &Rc<T>, dispose: fn(&T)) -> Dispose {
    let shown = Rc::downgrade(component);
    Box::new(move || {
        if let Some(component) = shown.upgrade() {
            dispose(&component);
        }
    })
}
