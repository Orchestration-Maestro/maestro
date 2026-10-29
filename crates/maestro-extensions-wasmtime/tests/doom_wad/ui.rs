//! A custom view captures the actual engine's WAD before closing through input.
use maestro_extensions::{
    EventError,
    types::{
        CustomComponentFactory, CustomFactoryOutcome, CustomUIOptions, ExtensionUIContextPort,
        ExtensionUIFuture, NotificationType,
    },
    widgets::ExtensionKeybindings,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// Visible notices and the data that reached the composed engine.
pub struct Ui {
    /// Real terminal supplied to custom factories.
    pub tui: maestro_tui::TUI,
    /// Controlled temporary parent of the engine's private root.
    pub temporary: PathBuf,
    /// All user-facing command notices.
    pub notices: RefCell<Vec<String>>,
    /// Failure to exercise through the retained custom view.
    pub failure: Option<String>,
    /// Completes when the background tick reports its failure.
    pub failed: Rc<tokio::sync::Notify>,
    /// Bytes the engine copied during initialization.
    pub wad: Rc<RefCell<Option<Vec<u8>>>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, _: Option<NotificationType>) -> Result<(), EventError> {
        if message.starts_with("DOOM stopped:") {
            self.failed.notify_one();
        }
        self.notices.borrow_mut().push(message);
        Ok(())
    }
    fn custom_component(
        &self,
        factory: CustomComponentFactory,
        _: Option<CustomUIOptions>,
    ) -> ExtensionUIFuture<()> {
        let (send, receive) = tokio::sync::oneshot::channel();
        let send = RefCell::new(Some(send));
        let close = Rc::new(move || {
            Ok(send
                .borrow_mut()
                .take()
                .is_some_and(|send| send.send(()).is_ok()))
        });
        let keys = ExtensionKeybindings {
            manager: maestro_tui::KeybindingsManager::new(Vec::new(), Vec::new()),
            reload: None,
        };
        let building = factory.call((self.tui.clone(), self.theme(), keys, close));
        let (temporary, wad) = (self.temporary.clone(), self.wad.clone());
        let failure = self.failure.clone();
        let failed = self.failed.clone();
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            let bytes = observe(temporary);
            wad.replace(Some(bytes.clone()));
            exercise(&built, failure.as_deref(), failed, receive, &bytes).await?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}

/// Reads the private file the controlled engine actually initialized.
fn observe(temporary: PathBuf) -> Vec<u8> {
    let roots: Vec<_> = std::fs::read_dir(temporary)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("doom1.wad").is_file())
        .collect();
    assert_eq!(roots.len(), 1);
    std::fs::read(roots[0].join("doom1.wad")).unwrap()
}

/// Drives input or failure through the native retained component.
async fn exercise(
    built: &maestro_extensions::types::ExtensionComponent,
    failure: Option<&str>,
    failed: Rc<tokio::sync::Notify>,
    mut receive: tokio::sync::oneshot::Receiver<()>,
    bytes: &[u8],
) -> Result<(), EventError> {
    let rows = built.component.try_render(8);
    if failure == Some("bad-frame") {
        let error = rows.unwrap_err().to_string();
        assert!(error.contains("framebuffer bytes"), "{error}");
        assert_eq!(
            receive.try_recv(),
            Ok(()),
            "malformed frame must close the view"
        );
        return Ok(());
    }
    let rows = rows.unwrap();
    assert_eq!(rows.len(), 11);
    if bytes == b"small-frame" {
        assert!(rows[0].starts_with("\x1b[38;2;0;0;0m\x1b[48;2;0;10;0m▀"));
        assert!(rows[1].starts_with("\x1b[38;2;0;20;0m\x1b[48;2;0;30;0m▀"));
    }
    if failure == Some("tick-error") {
        failed.notified().await;
        assert_eq!(
            receive.try_recv(),
            Ok(()),
            "tick failure must close the view"
        );
    } else {
        built
            .component
            .input_handler()
            .unwrap()
            .try_handle_input("q")
            .map_err(|e| EventError::from(e.to_string()))?;
        receive.await.map_err(|e| EventError::from(e.to_string()))?;
    }
    Ok(())
}
