//! Recording command operations shared by the command-context witnesses.
#![cfg(test)]
use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContextActions,
    types::{ForkOptions, NewSessionCommandOptions, SessionChangeResult, SwitchSessionOptions},
};
use maestro_session::session_manager::NavigateTreeOptions;
use std::{cell::RefCell, rc::Rc};

/// One invocation with the owned arguments received by the application.
pub enum Call {
    /// Wait for idle.
    Wait,
    /// New session and its supplied options.
    New(Option<NewSessionCommandOptions>),
    /// Fork entry and its supplied options.
    Fork(String, Option<ForkOptions>),
    /// Navigation target and its supplied options.
    Navigate(String, Option<NavigateTreeOptions>),
    /// Session path and its supplied options.
    Switch(String, Option<SwitchSessionOptions>),
    /// Reload resources.
    Reload,
}
/// Ordered invocation recorder.
#[derive(Default, Clone)]
pub struct Recorder(pub Rc<RefCell<Vec<Call>>>);
impl Recorder {
    /// Operations that record synchronously and resolve to the supplied result.
    pub fn operations(&self, cancelled: bool) -> CommandContextOperations {
        let (wait, new, fork, nav, switch, reload) = (
            self.0.clone(),
            self.0.clone(),
            self.0.clone(),
            self.0.clone(),
            self.0.clone(),
            self.0.clone(),
        );
        CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                wait.borrow_mut().push(Call::Wait);
                Box::pin(async { Ok(()) })
            }),
            new_session: Rc::new(move |options| {
                new.borrow_mut().push(Call::New(options));
                Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
            }),
            fork: Rc::new(move |id, options| {
                fork.borrow_mut().push(Call::Fork(id, options));
                Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
            }),
            navigate_tree: Rc::new(move |id, options| {
                nav.borrow_mut().push(Call::Navigate(id, options));
                Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
            }),
            switch_session: Rc::new(move |path, options| {
                switch.borrow_mut().push(Call::Switch(path, options));
                Box::pin(async move { Ok(SessionChangeResult { cancelled }) })
            }),
            reload: Rc::new(move || {
                reload.borrow_mut().push(Call::Reload);
                Box::pin(async { Ok(()) })
            }),
        }
    }
    /// The same operations as a bound set.
    pub fn actions(&self, cancelled: bool) -> ExtensionCommandContextActions {
        ExtensionCommandContextActions::new(self.operations(cancelled))
    }
    /// Takes just the calls produced by the preceding invocation group.
    pub fn take(&self) -> Vec<Call> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}
