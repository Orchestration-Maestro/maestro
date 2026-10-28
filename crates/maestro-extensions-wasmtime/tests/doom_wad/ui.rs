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
    /// Bytes the engine copied during initialization.
    pub wad: Rc<RefCell<Option<Vec<u8>>>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
    fn notify(&self, message: String, _: Option<NotificationType>) -> Result<(), EventError> {
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
        Box::pin(async move {
            let built = match building? {
                CustomFactoryOutcome::Ready(built) => built,
                CustomFactoryOutcome::Async(building) => building.await?,
            };
            let roots: Vec<_> = std::fs::read_dir(temporary)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.join("doom1.wad").is_file())
                .collect();
            assert_eq!(roots.len(), 1);
            wad.replace(Some(std::fs::read(roots[0].join("doom1.wad")).unwrap()));
            built
                .component
                .input_handler()
                .unwrap()
                .try_handle_input("q")
                .map_err(|e| EventError::from(e.to_string()))?;
            receive.await.map_err(|e| EventError::from(e.to_string()))?;
            if let Some(dispose) = built.dispose {
                dispose.call(())?;
            }
            Ok(())
        })
    }
}
