//! A retained terminal view driven by the same input interface as the interactive host.
use maestro_app::{
    CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, EventError,
    ExtensionUIContextPort, ExtensionUIFuture,
};
use std::{cell::RefCell, rc::Rc};
use tokio::sync::oneshot;

/// One custom view kept open until the test sends its closing key.
pub(super) struct Ui {
    /// Controlled frame writer, never a real terminal.
    tui: maestro_tui::TUI,
    /// The actual authored view exposed by the custom factory.
    view: Rc<RefCell<Option<maestro_tui::tui::ComponentHandle>>>,
    /// Announces that input can be delivered without guessing at readiness.
    ready: RefCell<Option<oneshot::Sender<()>>>,
}
impl Ui {
    /// Prepares the existing recording terminal and manual runtime.
    pub fn new() -> (Rc<Self>, oneshot::Receiver<()>) {
        let terminal = crate::tools_example::recording_terminal::RecordingTerminal::new(80, 24);
        let tui = maestro_tui::TUI::new(
            terminal.handle(),
            crate::tools_example::manual_runtime::ManualRuntime::new().handle(),
            maestro_tui::TerminalImage::default(),
            Some(false),
        );
        let (ready, opened) = oneshot::channel();
        (
            Rc::new(Self {
                tui,
                view: Rc::default(),
                ready: RefCell::new(Some(ready)),
            }),
            opened,
        )
    }
    /// Delivers a literal key through the retained component's input handler.
    pub fn input(&self, key: &str) {
        let view = self.view.borrow().as_ref().unwrap().clone();
        view.input_handler().unwrap().try_handle_input(key).unwrap();
    }
    /// Reads the authored component at the requested terminal width.
    pub fn render(&self, width: usize) -> Vec<String> {
        self.view
            .borrow()
            .as_ref()
            .unwrap()
            .try_render(width)
            .unwrap()
    }
}
/// A real prepared theme, shared with the custom-message and tool renderers.
pub(super) fn theme() -> maestro_theme::LiveTheme {
    let theme = maestro_theme::load_theme_from_path(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-theme/assets/theme/dark.json"
        ),
        Some(maestro_theme::ColorMode::Truecolor),
    )
    .unwrap();
    maestro_theme::LiveTheme::from_theme(Rc::new(theme))
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        theme()
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (closed, closing) = oneshot::channel();
        let closed = RefCell::new(Some(closed));
        let built = factory.call((
            self.tui.clone(),
            theme(),
            maestro_app::ExtensionKeybindings {
                manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
                reload: None,
            },
            Rc::new(move || {
                Ok(closed
                    .borrow_mut()
                    .take()
                    .is_some_and(|close| close.send(()).is_ok()))
            }),
        ));
        let view = self.view.clone();
        let ready = self.ready.borrow_mut().take();
        Box::pin(async move {
            let built = match built? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(built) => built.await?,
            };
            view.replace(Some(built.component.clone()));
            if let Some(ready) = ready {
                let _ = ready.send(());
            }
            closing
                .await
                .map_err(|error| EventError::from(error.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            view.take();
            Ok(())
        })
    }
}
