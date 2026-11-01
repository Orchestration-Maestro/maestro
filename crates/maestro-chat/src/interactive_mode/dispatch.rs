//! The application-owned startup presentation interface.
use super::InteractiveMode;
use maestro_app::presentation_data::mode_dispatch::InteractiveModeRunner;
use maestro_tui::tui::LocalFuture;
use std::{io, rc::Rc};
impl InteractiveModeRunner for InteractiveMode {
    fn init(self: Rc<Self>) -> LocalFuture {
        Box::pin(async move { InteractiveMode::init(&self).await })
    }
    fn run(self: Rc<Self>) -> LocalFuture {
        Box::pin(async move { InteractiveMode::run(&self).await })
    }
    fn stop(&self) -> io::Result<()> {
        InteractiveMode::stop(self)
    }
}
