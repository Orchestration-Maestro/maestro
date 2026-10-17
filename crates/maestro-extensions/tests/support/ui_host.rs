//! A retained editor and selection host shared by context witnesses.
#![cfg(test)]
use maestro_extensions::types::{ExtensionUIContextPort, ExtensionUIFuture};
use maestro_extensions::{EventError, ExtensionUIContext, ExtensionUIDialogOptions};
use maestro_theme::LiveTheme;
use std::{cell::RefCell, rc::Rc};
/// A host whose editor text and selection answers identify it.
pub struct Named {
    /// Host identity.
    name: &'static str,
    /// Current editor text.
    text: RefCell<String>,
}
impl Named {
    /// An installed context around a fresh named host, and the host itself.
    pub fn install(name: &'static str) -> (ExtensionUIContext, Rc<Self>) {
        let host = Rc::new(Self {
            name,
            text: RefCell::default(),
        });
        (ExtensionUIContext::new(host.clone()), host)
    }
}
impl ExtensionUIContextPort for Named {
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        _: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionUIFuture<Option<String>> {
        let answer = format!("{}:{title}:{}", self.name, options.join(","));
        Box::pin(async move { Ok(Some(answer)) })
    }
    fn set_editor_text(&self, text: String) -> Result<(), EventError> {
        *self.text.borrow_mut() = text;
        Ok(())
    }
    fn get_editor_text(&self) -> Result<String, EventError> {
        Ok(self.text.borrow().clone())
    }
    fn theme(&self) -> LiveTheme {
        LiveTheme::default()
    }
}
