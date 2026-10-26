//! Host keybindings managers, keybinding hints and editor styles behind the author's facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use super::widget_imports::{KeyHint, KeybindingsAnswer, KeybindingsRequest, WidgetImports};
use super::widgets::{Builder, host_style};
use crate::{
    EditorTheme, ExtensionResult, KeybindingConflict, KeybindingDefinition, KeybindingsConfig,
    KeybindingsManager, KeybindingsPort, SelectListTheme,
};

/// A host keybindings manager kept by its facade.
pub(super) struct HostKeybindings<I: WidgetImports> {
    /// The host the manager belongs to.
    imports: I,
    /// The host resource.
    pub(super) manager: I::Keybindings,
}

impl<I: WidgetImports> HostKeybindings<I> {
    /// Sends one request.
    fn request(&self, request: KeybindingsRequest<'_>) -> ExtensionResult<KeybindingsAnswer> {
        self.imports.keybindings_request(&self.manager, request)
    }
    /// Sends one request that answers bindings.
    fn config(&self, request: KeybindingsRequest<'_>) -> ExtensionResult<KeybindingsConfig> {
        match self.request(request)? {
            KeybindingsAnswer::Config(config) => Ok(config),
            _ => Err("the host answered no keybindings".to_owned()),
        }
    }
}

/// Editor styles over a lent border style and completion list styles in field order.
pub(super) fn editor_theme<I: WidgetImports>(
    imports: &I,
    (border, styles): (I::Style, Vec<I::Style>),
) -> ExtensionResult<EditorTheme> {
    let mut styles = styles.into_iter().map(|style| host_style(imports, style));
    let mut next = || {
        styles
            .next()
            .ok_or_else(|| "the host lent too few editor styles".to_owned())
    };
    Ok(EditorTheme {
        border_color: host_style(imports, border),
        select_list: SelectListTheme {
            selected_prefix: next()?,
            selected_text: next()?,
            description: next()?,
            scroll_info: next()?,
            no_match: next()?,
        },
    })
}

/// Wraps a lent host manager.
pub(super) fn lend<I: WidgetImports>(imports: &I, manager: I::Keybindings) -> KeybindingsManager {
    KeybindingsManager::lent(Rc::new(HostKeybindings {
        imports: imports.clone(),
        manager,
    }))
}

impl<I: WidgetImports> KeybindingsPort for HostKeybindings<I> {
    fn matches(&self, data: &str, keybinding: &str) -> ExtensionResult<bool> {
        match self.request(KeybindingsRequest::Matches(data, keybinding))? {
            KeybindingsAnswer::Matched(matched) => Ok(matched),
            _ => Err("the host answered no match".to_owned()),
        }
    }
    fn get_keys(&self, keybinding: &str) -> ExtensionResult<Vec<String>> {
        match self.request(KeybindingsRequest::Keys(keybinding))? {
            KeybindingsAnswer::Keys(keys) => Ok(keys),
            _ => Err("the host answered no keys".to_owned()),
        }
    }
    fn get_definition(&self, keybinding: &str) -> ExtensionResult<Option<KeybindingDefinition>> {
        match self.request(KeybindingsRequest::Definition(keybinding))? {
            KeybindingsAnswer::Definition(definition) => Ok(definition),
            _ => Err("the host answered no definition".to_owned()),
        }
    }
    fn get_conflicts(&self) -> ExtensionResult<Vec<KeybindingConflict>> {
        match self.request(KeybindingsRequest::Conflicts)? {
            KeybindingsAnswer::Conflicts(conflicts) => Ok(conflicts),
            _ => Err("the host answered no conflicts".to_owned()),
        }
    }
    fn set_user_bindings(&self, user_bindings: KeybindingsConfig) -> ExtensionResult<()> {
        self.request(KeybindingsRequest::SetUserBindings(&user_bindings))
            .map(drop)
    }
    fn get_user_bindings(&self) -> ExtensionResult<KeybindingsConfig> {
        self.config(KeybindingsRequest::UserBindings)
    }
    fn get_resolved_bindings(&self) -> ExtensionResult<KeybindingsConfig> {
        self.config(KeybindingsRequest::ResolvedBindings)
    }
    fn reload(&self) -> ExtensionResult<()> {
        self.request(KeybindingsRequest::Reload).map(drop)
    }
    fn get_effective_config(&self) -> ExtensionResult<KeybindingsConfig> {
        self.config(KeybindingsRequest::EffectiveConfig)
    }
}

impl<I: WidgetImports> Builder<I> {
    /// A manager of the application's keybindings.
    pub(super) fn application_keybindings(
        &self,
        user_bindings: &KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<KeybindingsManager> {
        let manager = self.0.new_keybindings(user_bindings, config_path)?;
        Ok(lend(&self.0, manager))
    }

    /// The live theme's editor styles.
    pub(super) fn lent_editor_theme(&self) -> ExtensionResult<EditorTheme> {
        editor_theme(&self.0, self.0.get_editor_theme()?)
    }

    /// A keybinding's keys, or a hint.
    pub(super) fn hint(&self, hint: KeyHint<'_>) -> ExtensionResult<String> {
        self.0.key_hint(hint)
    }
}
