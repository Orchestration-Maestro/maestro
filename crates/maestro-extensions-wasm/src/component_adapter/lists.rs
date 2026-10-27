//! Host selection lists, settings lists and loaders, and the authored callbacks they run.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use super::callbacks::{self, Identity};
use super::renderers::with_emitter;
use super::widget_imports::{
    HandlerSlot, RowStyleRef, SettingRef, SettingsStyles, WidgetAnswer, WidgetEvent, WidgetImports,
};
use super::widgets::{Builder, Staged, stage_style, style_ref};
use crate::{
    ExtensionResult, RowStyle, SelectHandler, SelectItem, SelectListLayoutOptions, SelectListTheme,
    SettingChange, SettingItem, SettingsListOptions, SettingsListTheme, Submenu, SubmenuDone,
    SubmenuDonePort, TUI, Theme, TruncatePrimary, WidgetNotice,
};

/// An authored widget callback, by the shape the host calls it with.
pub(crate) enum WidgetHandler {
    /// Receives a choice.
    Select(SelectHandler),
    /// Receives a notice.
    Notice(WidgetNotice),
    /// Receives a setting's change.
    Change(SettingChange),
    /// Truncates primary text.
    Truncate(TruncatePrimary),
    /// Styles a settings row.
    Row(RowStyle),
    /// Builds a submenu.
    Submenu(Submenu),
    /// Receives an editor's text.
    Text(crate::TextHandler),
    /// Intercepts an editor's input.
    Shortcut(crate::EditorShortcut),
    /// Decides whether an overlay shows.
    Visible(crate::OverlayVisible),
    /// Receives a shown overlay's handle.
    OverlayHandle(crate::OverlayHandleCallback),
    /// Resolves an overlay's placement.
    OverlayOptions(crate::OverlayOptionsResolver),
}

/// A host row style kept by its facade.
pub(super) struct HostRowStyle<I: WidgetImports> {
    /// The host resource.
    style: I::RowStyle,
}

/// The host's submenu closer kept by its facade.
struct HostDone<I: WidgetImports> {
    /// The host the closer belongs to.
    imports: I,
    /// The host resource.
    done: I::SubmenuDone,
}

impl<I: WidgetImports> SubmenuDonePort for HostDone<I> {
    fn done(&self, value: Option<&str>) -> ExtensionResult<()> {
        self.imports.submenu_done(&self.done, value)
    }
}

/// `handler` staged under a fresh identity for one request.
pub(super) fn stage_handler<I: super::Imports>(imports: &I, handler: WidgetHandler) -> Identity<I> {
    let identity = Identity::new(imports);
    identity.stage(Rc::new(handler));
    identity
}

/// The host row style behind `style`, or `style` staged under a fresh identity.
fn stage_row<I: WidgetImports>(imports: &I, style: &RowStyle) -> Staged<I, HostRowStyle<I>> {
    if let Some(host) = style.0.host::<HostRowStyle<I>>() {
        return Staged::Host(host);
    }
    Staged::Authored(stage_handler(imports, WidgetHandler::Row(style.clone())))
}

/// The operand naming a staged row style.
fn row_ref<I: WidgetImports>(staged: &Staged<I, HostRowStyle<I>>) -> RowStyleRef<'_, I> {
    match staged {
        Staged::Host(host) => RowStyleRef::Host(&host.style),
        Staged::Authored(identity) => RowStyleRef::Authored(&identity.handle),
    }
}

impl<I: WidgetImports> Builder<I> {
    /// Builds a selection list, staging its authored styles and truncation.
    pub(super) fn build_select_list(
        &self,
        items: &[SelectItem],
        max_visible: u32,
        theme: &SelectListTheme,
        layout: &SelectListLayoutOptions,
    ) -> ExtensionResult<I::Widget> {
        let imports = &self.0;
        let styles: Vec<_> = [
            &theme.selected_prefix,
            &theme.selected_text,
            &theme.description,
            &theme.scroll_info,
            &theme.no_match,
        ]
        .into_iter()
        .map(|style| stage_style(imports, style))
        .collect();
        let truncate = layout
            .truncate_primary
            .as_ref()
            .map(|truncate| stage_handler(imports, WidgetHandler::Truncate(truncate.clone())));
        let layout_ref = (
            layout.min_primary_column_width,
            layout.max_primary_column_width,
            truncate.as_ref().map(|identity| &identity.handle),
        );
        let result = imports.new_select_list(
            items,
            max_visible,
            styles.iter().map(style_ref).collect(),
            layout_ref,
        );
        let accepted = result.is_ok();
        for staged in styles {
            staged.settle(accepted);
        }
        if let Some(identity) = truncate {
            identity.settle(accepted);
        }
        result
    }

    /// Builds a settings list, staging its authored styles, submenus and callbacks.
    pub(super) fn build_settings_list(
        &self,
        (items, max_visible): (&[SettingItem], u32),
        theme: &SettingsListTheme,
        (on_change, on_cancel): (SettingChange, WidgetNotice),
        options: SettingsListOptions,
    ) -> ExtensionResult<I::Widget> {
        let imports = &self.0;
        let submenus: Vec<_> = items
            .iter()
            .map(|item| {
                let submenu = item.submenu.clone()?;
                Some(stage_handler(imports, WidgetHandler::Submenu(submenu)))
            })
            .collect();
        let settings: Vec<_> = items
            .iter()
            .zip(&submenus)
            .map(|(item, submenu)| SettingRef {
                item,
                submenu: submenu.as_ref().map(|identity| &identity.handle),
            })
            .collect();
        let rows = [
            stage_row(imports, &theme.label),
            stage_row(imports, &theme.value),
        ];
        let styles = [
            stage_style(imports, &theme.description),
            stage_style(imports, &theme.hint),
        ];
        let change = stage_handler(imports, WidgetHandler::Change(on_change));
        let cancel = stage_handler(imports, WidgetHandler::Notice(on_cancel));
        let theme_ref = SettingsStyles {
            label: row_ref(&rows[0]),
            value: row_ref(&rows[1]),
            description: style_ref(&styles[0]),
            cursor: &theme.cursor,
            hint: style_ref(&styles[1]),
        };
        let callbacks = (&change.handle, &cancel.handle);
        let result = imports.new_settings_list(
            (&settings, max_visible),
            theme_ref,
            callbacks,
            options.enable_search,
        );
        let accepted = result.is_ok();
        drop(settings);
        for identity in submenus.into_iter().flatten().chain([change, cancel]) {
            identity.settle(accepted);
        }
        for staged in rows {
            staged.settle(accepted);
        }
        for staged in styles {
            staged.settle(accepted);
        }
        result
    }

    /// Builds a bordered loader on the host terminal and theme behind the facades.
    pub(super) fn build_bordered_loader(
        &self,
        tui: &TUI,
        theme: &Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<I::Widget> {
        let tui = tui
            .host::<super::factories::Terminal<I>>()
            .ok_or("a loader needs a terminal the host lent")?;
        let theme = theme
            .host::<super::theme::Retained<I>>()
            .ok_or("a loader needs a theme the host supplied")?;
        self.0
            .new_bordered_loader(&tui.tui, &theme.theme, message, cancellable)
    }

    /// The live theme's selection list styles.
    pub(super) fn lent_select_list_theme(&self) -> ExtensionResult<SelectListTheme> {
        let styles = self.0.get_select_list_theme()?;
        let mut styles = styles
            .into_iter()
            .map(|style| super::widgets::host_style(&self.0, style));
        let mut next = || {
            styles
                .next()
                .ok_or_else(|| "the host lent too few selection list styles".to_owned())
        };
        Ok(SelectListTheme {
            selected_prefix: next()?,
            selected_text: next()?,
            description: next()?,
            scroll_info: next()?,
            no_match: next()?,
        })
    }

    /// The live theme's settings list styles.
    pub(super) fn lent_settings_list_theme(&self) -> ExtensionResult<SettingsListTheme> {
        let (label, value, description, cursor, hint) = self.0.get_settings_list_theme()?;
        let row = |style: I::RowStyle| {
            let host = Rc::new(HostRowStyle::<I> { style });
            let (imports, owner) = (self.0.clone(), host.clone());
            RowStyle(crate::Supplied::lent(
                Rc::new(move |text: &str, selected: bool| {
                    imports.row_style_apply(&owner.style, text, selected)
                }),
                host,
            ))
        };
        Ok(SettingsListTheme {
            label: row(label),
            value: row(value),
            description: super::widgets::host_style(&self.0, description),
            cursor,
            hint: super::widgets::host_style(&self.0, hint),
        })
    }
}

/// Replaces or removes one handler of `widget`, keeping a new handler when the host accepts.
pub(super) fn set_handler<I: WidgetImports>(
    imports: &I,
    widget: &I::Widget,
    slot: HandlerSlot,
    handler: Option<WidgetHandler>,
) -> ExtensionResult<()> {
    let staged = handler.map(|handler| stage_handler(imports, handler));
    let result = imports.widget_set_handler(
        widget,
        slot,
        staged.as_ref().map(|identity| &identity.handle),
    );
    if let Some(identity) = staged {
        identity.settle(result.is_ok());
    }
    result
}

/// Runs the authored widget callback under `handler` with a borrowed emitter.
///
/// # Errors
/// Returns a message when the callback does not take `event`, or the callback's failure.
pub(crate) fn invoke_widget<I: WidgetImports>(
    imports: &I,
    handler: u32,
    event: WidgetEvent<I>,
    emitter: &I::Emitter,
) -> ExtensionResult<WidgetAnswer> {
    let handler = callbacks::find::<Rc<WidgetHandler>>(handler)?;
    with_emitter(imports, emitter, |emitter| match (&*handler, event) {
        (
            WidgetHandler::Select(handler),
            WidgetEvent::Select(item) | WidgetEvent::SelectionChange(item),
        ) => handler(&item, emitter).map(|()| WidgetAnswer::Done),
        (
            WidgetHandler::Notice(handler),
            WidgetEvent::Cancel | WidgetEvent::Abort | WidgetEvent::Notice,
        ) => handler(emitter).map(|()| WidgetAnswer::Done),
        (WidgetHandler::Text(handler), WidgetEvent::Text(text)) => {
            handler(&text, emitter).map(|()| WidgetAnswer::Done)
        }
        (WidgetHandler::Shortcut(handler), WidgetEvent::Shortcut(data)) => {
            handler(&data, emitter).map(WidgetAnswer::Consumed)
        }
        (WidgetHandler::Change(handler), WidgetEvent::Change(id, value)) => {
            handler(&id, &value, emitter).map(|()| WidgetAnswer::Done)
        }
        (WidgetHandler::Truncate(truncate), WidgetEvent::Truncate(context)) => {
            truncate(&context, emitter).map(WidgetAnswer::Text)
        }
        (WidgetHandler::Row(style), WidgetEvent::Row(text, selected)) => {
            style.apply(&text, selected).map(WidgetAnswer::Text)
        }
        (WidgetHandler::Visible(visible), WidgetEvent::Visible(columns, rows)) => {
            visible(columns, rows).map(WidgetAnswer::Visible)
        }
        (WidgetHandler::OverlayHandle(receive), WidgetEvent::OverlayHandle(handle)) => {
            receive(super::overlays::handle(imports, handle), emitter).map(|()| WidgetAnswer::Done)
        }
        (WidgetHandler::OverlayOptions(resolve), WidgetEvent::OverlayOptions) => {
            resolve(emitter).map(WidgetAnswer::OverlayOptions)
        }
        (WidgetHandler::Submenu(submenu), WidgetEvent::Submenu(value, done)) => {
            let done = SubmenuDone::new(Rc::new(HostDone {
                imports: imports.clone(),
                done,
            }));
            submenu(&value, done, emitter).map(WidgetAnswer::Component)
        }
        _ => Err("the widget callback does not take this event".to_owned()),
    })
}
