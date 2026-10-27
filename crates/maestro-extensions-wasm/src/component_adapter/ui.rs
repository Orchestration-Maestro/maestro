//! The retained UI capability behind the author's UI port.

use std::rc::Rc;

use super::callbacks::Identity;
use super::event_bus::subscription;
use super::imports::{Dialog, Imports};
use super::signals::Binding;
use crate::types::{ExtensionFuture, ExtensionResult};
use crate::{
    ExtensionUIContext, ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionWidgetOptions,
    NotificationType, Subscription, TerminalInputHandler, Theme, ThemeInfo, ThemeSelection,
    ThemeSetResult, WorkingIndicatorOptions,
};

/// Independently retained UI resource.
pub(super) struct Ui<I: Imports> {
    /// The host the capability belongs to.
    pub(super) imports: I,
    /// The retained host capability, shared with the dialogs it has open.
    pub(super) resource: Rc<I::Ui>,
}

impl<I: Imports> Ui<I> {
    /// Owned copies of the host and its capability for a dialog that outlives this borrow.
    fn owned(&self) -> (I, Rc<I::Ui>) {
        (self.imports.clone(), Rc::clone(&self.resource))
    }
}

/// The host signal following a dialog's caller signal, and the dialog's timeout.
fn follow<I: Imports>(
    imports: &I,
    opts: Option<ExtensionUIDialogOptions>,
) -> ExtensionResult<(Binding<I>, Option<f64>)> {
    let (signal, timeout) = opts.map_or((None, None), |opts| (opts.signal, opts.timeout));
    Ok((Binding::new(imports, signal.as_ref())?, timeout))
}

impl<I: Imports> ExtensionUIContextPort for Ui<I> {
    fn custom_component(
        &self,
        factory: crate::CustomComponentFactory,
        options: Option<crate::CustomUIOptions>,
    ) -> ExtensionFuture<'static, ()> {
        let (imports, resource) = self.owned();
        Box::pin(async move {
            let identity = super::callbacks::Scoped::new(&imports, factory);
            let retained = options
                .map(|options| super::custom::Retained::new(&imports, options))
                .transpose()?;
            let options = retained.as_ref().map(super::custom::Retained::operands);
            imports
                .ui_custom(&resource, &identity.handle, options)
                .await
        })
    }
    fn notify(&self, message: &str, kind: Option<NotificationType>) -> ExtensionResult<()> {
        self.imports.ui_notify(&self.resource, message, kind)
    }
    fn set_status(&self, key: &str, text: Option<&str>) -> ExtensionResult<()> {
        self.imports.ui_set_status(&self.resource, key, text)
    }
    fn set_working_message(&self, message: Option<&str>) -> ExtensionResult<()> {
        self.imports.ui_set_working_message(&self.resource, message)
    }
    fn set_working_visible(&self, visible: bool) -> ExtensionResult<()> {
        self.imports.ui_set_working_visible(&self.resource, visible)
    }
    fn set_working_indicator(
        &self,
        options: Option<WorkingIndicatorOptions>,
    ) -> ExtensionResult<()> {
        self.imports
            .ui_set_working_indicator(&self.resource, options)
    }
    fn set_hidden_thinking_label(&self, label: Option<&str>) -> ExtensionResult<()> {
        self.imports
            .ui_set_hidden_thinking_label(&self.resource, label)
    }
    fn set_widget(
        &self,
        key: &str,
        content: Option<&[String]>,
        options: Option<ExtensionWidgetOptions>,
    ) -> ExtensionResult<()> {
        self.imports
            .ui_set_widget(&self.resource, key, content, options)
    }
    fn set_footer(&self, factory: Option<crate::FooterFactory>) -> ExtensionResult<()> {
        let Some(factory) = factory else {
            return self.imports.ui_set_footer(&self.resource, None);
        };
        let identity = Identity::new(&self.imports);
        identity.stage(factory);
        let result = self
            .imports
            .ui_set_footer(&self.resource, Some(&identity.handle));
        identity.settle(result.is_ok());
        result
    }
    fn set_header(&self, factory: Option<crate::ComponentFactory>) -> ExtensionResult<()> {
        let Some(factory) = factory else {
            return self.imports.ui_set_header(&self.resource, None);
        };
        let identity = Identity::new(&self.imports);
        identity.stage(factory);
        let result = self
            .imports
            .ui_set_header(&self.resource, Some(&identity.handle));
        identity.settle(result.is_ok());
        result
    }
    fn set_widget_factory(
        &self,
        key: &str,
        factory: crate::ComponentFactory,
        options: Option<ExtensionWidgetOptions>,
    ) -> ExtensionResult<()> {
        let identity = Identity::new(&self.imports);
        identity.stage(factory);
        let result =
            self.imports
                .ui_set_widget_factory(&self.resource, key, &identity.handle, options);
        identity.settle(result.is_ok());
        result
    }
    fn set_title(&self, title: &str) -> ExtensionResult<()> {
        self.imports.ui_set_title(&self.resource, title)
    }
    fn paste_to_editor(&self, text: &str) -> ExtensionResult<()> {
        self.imports.ui_paste_to_editor(&self.resource, text)
    }
    fn set_editor_text(&self, text: &str) -> ExtensionResult<()> {
        self.imports.ui_set_editor_text(&self.resource, text)
    }
    fn get_editor_text(&self) -> ExtensionResult<String> {
        self.imports.ui_get_editor_text(&self.resource)
    }
    fn set_editor_component(&self, factory: Option<crate::EditorFactory>) -> ExtensionResult<()> {
        let Some(factory) = factory else {
            return self.imports.ui_set_editor_component(&self.resource, None);
        };
        let identity = Identity::new(&self.imports);
        identity.stage(factory);
        let result = self
            .imports
            .ui_set_editor_component(&self.resource, Some(&identity.handle));
        identity.settle(result.is_ok());
        result
    }
    fn add_autocomplete_provider(
        &self,
        factory: crate::AutocompleteProviderFactory,
    ) -> ExtensionResult<()> {
        let identity = Identity::new(&self.imports);
        identity.stage(factory);
        let result = self
            .imports
            .ui_add_autocomplete_provider(&self.resource, &identity.handle);
        identity.settle(result.is_ok());
        result
    }
    fn get_editor_component(&self) -> ExtensionResult<Option<crate::EditorFactory>> {
        Ok(
            match self.imports.ui_get_editor_component(&self.resource)? {
                None => None,
                Some(Ok(id)) => Some(super::callbacks::find(id)?),
                Some(Err(factory)) => Some(super::foreign_editors::factory(factory)),
            },
        )
    }
    fn theme(&self) -> ExtensionResult<Theme> {
        let theme = self.imports.ui_theme(&self.resource)?;
        Ok(super::theme::theme(self.imports.clone(), theme))
    }
    fn get_all_themes(&self) -> ExtensionResult<Vec<ThemeInfo>> {
        self.imports.ui_get_all_themes(&self.resource)
    }
    fn get_theme(&self, name: &str) -> ExtensionResult<Option<Theme>> {
        let theme = self.imports.ui_get_theme(&self.resource, name)?;
        Ok(theme.map(|theme| super::theme::theme(self.imports.clone(), theme)))
    }
    fn set_theme(&self, theme: ThemeSelection) -> ExtensionResult<ThemeSetResult> {
        match theme {
            ThemeSelection::Name(name) => self.imports.ui_set_theme_name(&self.resource, &name),
            ThemeSelection::Theme(theme) => {
                let retained = theme
                    .host::<super::theme::Retained<I>>()
                    .ok_or("the theme was not supplied by the host")?;
                self.imports.ui_set_theme(&self.resource, &retained.theme)
            }
        }
    }
    fn get_tools_expanded(&self) -> ExtensionResult<bool> {
        self.imports.ui_get_tools_expanded(&self.resource)
    }
    fn set_tools_expanded(&self, expanded: bool) -> ExtensionResult<()> {
        self.imports.ui_set_tools_expanded(&self.resource, expanded)
    }
    fn on_terminal_input(&self, handler: TerminalInputHandler) -> ExtensionResult<Subscription> {
        let identity = Identity::new(&self.imports);
        let remover = self
            .imports
            .ui_on_terminal_input(&self.resource, &identity.handle)?;
        identity.keep(handler);
        Ok(subscription(self.imports.clone(), remover))
    }
    fn select(
        &self,
        title: String,
        options: Vec<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionFuture<'static, Option<String>> {
        let (imports, resource) = self.owned();
        Box::pin(async move {
            let (binding, timeout) = follow(&imports, opts)?;
            let dialog = Dialog {
                signal: binding.signal(),
                timeout,
            };
            let answer = imports.ui_select(&resource, title, options, dialog).await;
            binding.settle(answer)
        })
    }
    fn confirm(
        &self,
        title: String,
        message: String,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionFuture<'static, bool> {
        let (imports, resource) = self.owned();
        Box::pin(async move {
            let (binding, timeout) = follow(&imports, opts)?;
            let dialog = Dialog {
                signal: binding.signal(),
                timeout,
            };
            let answer = imports.ui_confirm(&resource, title, message, dialog).await;
            binding.settle(answer)
        })
    }
    fn input(
        &self,
        title: String,
        placeholder: Option<String>,
        opts: Option<ExtensionUIDialogOptions>,
    ) -> ExtensionFuture<'static, Option<String>> {
        let (imports, resource) = self.owned();
        Box::pin(async move {
            let (binding, timeout) = follow(&imports, opts)?;
            let dialog = Dialog {
                signal: binding.signal(),
                timeout,
            };
            let answer = imports
                .ui_input(&resource, title, placeholder, dialog)
                .await;
            binding.settle(answer)
        })
    }
    fn editor(
        &self,
        title: String,
        prefill: Option<String>,
    ) -> ExtensionFuture<'static, Option<String>> {
        let (imports, resource) = self.owned();
        Box::pin(async move { imports.ui_editor(&resource, title, prefill).await })
    }
}

/// Wraps an independently retained host UI capability.
pub(super) fn ui<I: Imports>(imports: I, resource: I::Ui) -> ExtensionUIContext {
    ExtensionUIContext::new(Rc::new(Ui {
        imports,
        resource: Rc::new(resource),
    }))
}
