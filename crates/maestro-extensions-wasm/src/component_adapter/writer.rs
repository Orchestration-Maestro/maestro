//! The frame writer and its terminal behind the author's facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::callbacks::Identity;
use super::factories::Terminal as Writer;
use super::lists::{WidgetHandler, stage_handler};
use super::widget_imports::{
    TerminalRequest, WidgetImports, WriterAnswer, WriterRequest, WriterTarget,
};
use super::widgets::HostWidget;
use crate::{
    Component, ExtensionFuture, ExtensionResult, HostWidgetHandle as Widget, OverlayHandle,
    OverlayOptionsHandle, Subscription, SubscriptionPort, Terminal, TerminalInputHandler,
    TerminalPort, TextHandler, TuiPort, WidgetNotice, WidgetPort,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The message of an answer of the wrong kind.
const MISMATCH: &str = "extension frame writer result kind mismatch";

impl<I: WidgetImports> Writer<I> {
    /// The frame writer as a container widget, built once.
    fn widget(&self) -> ExtensionResult<Rc<HostWidget<I>>> {
        if let Some(widget) = self.writer.get() {
            return Ok(widget.clone());
        }
        let widget = HostWidget::shared(&self.imports, self.imports.tui_writer(&self.tui)?);
        Ok(self.writer.get_or_init(|| widget).clone())
    }
    /// Runs a request that answers nothing.
    fn act(&self, request: WriterRequest<'_, I>) -> ExtensionResult<()> {
        let widget = self.widget()?;
        self.imports
            .widget_writer(&widget.widget, request)
            .map(drop)
    }
    /// Runs a request that answers a flag.
    fn flag(&self, request: WriterRequest<'_, I>) -> ExtensionResult<bool> {
        let widget = self.widget()?;
        match self.imports.widget_writer(&widget.widget, request)? {
            WriterAnswer::Flag(flag) => Ok(flag),
            _ => Err(MISMATCH.to_owned()),
        }
    }
    /// Runs `request` on `component`'s target: a host widget, an identity this writer lent
    /// it under before while the host still keeps it, or a fresh identity kept once the host
    /// accepts.
    fn targeted<T>(
        &self,
        component: &Component,
        request: impl FnOnce(WriterTarget<'_, I>) -> ExtensionResult<T>,
    ) -> ExtensionResult<T> {
        if let Some(host) = component.widget().and_then(Widget::owner::<HostWidget<I>>) {
            return request(WriterTarget::Widget(&host.widget));
        }
        if let Some(id) = self
            .live(component)?
            .or_else(|| self.writer.get().and_then(|widget| widget.guest(component)))
        {
            return request(WriterTarget::Lent(id));
        }
        let identity = Identity::new(&self.imports);
        identity.stage(component.clone());
        let result = request(WriterTarget::Component(&identity.handle));
        if result.is_ok() {
            let mut lent = self.lent.borrow_mut();
            lent.retain(|(view, _)| view.strong_count() > 0 && !component.is(view));
            lent.push((component.downgrade(), identity.id()));
        }
        identity.settle(result.is_ok());
        result
    }
    /// The identity `component` was lent under, when the host still keeps it; the host
    /// releases a retired target asynchronously, so it is asked.
    fn live(&self, component: &Component) -> ExtensionResult<Option<u32>> {
        let lent = self
            .lent
            .borrow()
            .iter()
            .find_map(|(view, id)| component.is(view).then_some(*id));
        match lent {
            Some(id) if self.flag(WriterRequest::LentLive(id))? => Ok(Some(id)),
            _ => Ok(None),
        }
    }
}

impl<I: WidgetImports> TuiPort for Writer<I> {
    fn full_redraws(&self) -> ExtensionResult<u64> {
        self.imports.tui_full_redraws(&self.tui)
    }
    fn terminal(&self) -> ExtensionResult<Terminal> {
        Ok(Terminal::new(Rc::new(Screen(self.widget()?))))
    }
    fn add_child(&self, component: &Component) -> ExtensionResult<()> {
        self.widget()?.add_child(component)
    }
    fn remove_child(&self, component: &Component) -> ExtensionResult<()> {
        self.widget()?.remove_child(component)
    }
    fn clear(&self) -> ExtensionResult<()> {
        self.widget()?.clear()
    }
    fn invalidate(&self) -> ExtensionResult<()> {
        self.widget()?.invalidate()
    }
    fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
        self.widget()?.render(width)
    }
    fn get_show_hardware_cursor(&self) -> ExtensionResult<bool> {
        self.flag(WriterRequest::GetShowHardwareCursor)
    }
    fn set_show_hardware_cursor(&self, enabled: bool) -> ExtensionResult<()> {
        self.act(WriterRequest::SetShowHardwareCursor(enabled))
    }
    fn get_clear_on_shrink(&self) -> ExtensionResult<bool> {
        self.flag(WriterRequest::GetClearOnShrink)
    }
    fn set_clear_on_shrink(&self, enabled: bool) -> ExtensionResult<()> {
        self.act(WriterRequest::SetClearOnShrink(enabled))
    }
    fn set_focus(&self, component: Option<&Component>) -> ExtensionResult<()> {
        match component {
            Some(component) => self.targeted(component, |target| {
                self.act(WriterRequest::SetFocus(Some(target)))
            }),
            None => self.act(WriterRequest::SetFocus(None)),
        }
    }
    fn show_overlay(
        &self,
        component: &Component,
        options: Option<&OverlayOptionsHandle>,
    ) -> ExtensionResult<OverlayHandle> {
        let options = options
            .map(|options| {
                options
                    .0
                    .host::<super::overlays::Options<I>>()
                    .ok_or_else(|| "overlay options belong to another host".to_owned())
            })
            .transpose()?;
        let widget = self.widget()?;
        self.targeted(component, |target| {
            let options = options.as_ref().map(|options| &options.resource);
            match self
                .imports
                .widget_writer(&widget.widget, WriterRequest::ShowOverlay(target, options))?
            {
                WriterAnswer::Overlay(handle) => Ok(super::overlays::handle(&self.imports, handle)),
                _ => Err(MISMATCH.to_owned()),
            }
        })
    }
    fn hide_overlay(&self) -> ExtensionResult<()> {
        self.act(WriterRequest::HideOverlay)
    }
    fn has_overlay(&self) -> ExtensionResult<bool> {
        self.flag(WriterRequest::HasOverlay)
    }
    fn start(&self) -> ExtensionResult<()> {
        self.act(WriterRequest::Start)
    }
    fn stop(&self) -> ExtensionResult<()> {
        self.act(WriterRequest::Stop)
    }
    fn request_render(&self, force: bool) -> ExtensionResult<()> {
        self.act(WriterRequest::RequestRender(force))
    }
    fn add_input_listener(&self, listener: TerminalInputHandler) -> ExtensionResult<Subscription> {
        let added = self
            .listeners
            .borrow()
            .iter()
            .find_map(|(kept, remover)| Rc::ptr_eq(kept, &listener).then(|| remover.clone()));
        let remover = if let Some(remover) = added {
            remover
        } else {
            let widget = self.widget()?;
            let identity = Identity::new(&self.imports);
            let remover = self
                .imports
                .widget_add_input_listener(&widget.widget, &identity.handle)?;
            identity.keep(listener.clone());
            let remover = super::event_bus::subscription(self.imports.clone(), remover);
            self.listeners
                .borrow_mut()
                .push((listener.clone(), remover.clone()));
            remover
        };
        Ok(Subscription::new(Rc::new(InputRemoval {
            listeners: Rc::downgrade(&self.listeners),
            listener: Rc::downgrade(&listener),
            remover,
        })))
    }
    fn remove_input_listener(&self, listener: &TerminalInputHandler) -> ExtensionResult<()> {
        remove_listener(&self.listeners, Rc::as_ptr(listener))
    }
    fn set_on_debug(&self, callback: Option<WidgetNotice>) -> ExtensionResult<()> {
        let staged =
            callback.map(|callback| stage_handler(&self.imports, WidgetHandler::Notice(callback)));
        let result = self.act(WriterRequest::SetOnDebug(
            staged.as_ref().map(|staged| &staged.handle),
        ));
        if let Some(staged) = staged {
            staged.settle(result.is_ok());
        }
        result
    }
}

/// The callable behind an input listener.
type TerminalInputFn = dyn for<'a> Fn(
    &str,
    crate::CallbackEmitter<'a>,
) -> ExtensionResult<Option<crate::TerminalInputResult>>;

/// Input listeners added through one frame writer, each once, with their host removers.
pub(super) type InputListeners = RefCell<Vec<(TerminalInputHandler, Subscription)>>;

/// Removes `listener` from the frame writer and its host; one that is not present is ignored.
fn remove_listener(
    listeners: &InputListeners,
    listener: *const TerminalInputFn,
) -> ExtensionResult<()> {
    let index = listeners
        .borrow()
        .iter()
        .position(|(kept, _)| std::ptr::addr_eq(Rc::as_ptr(kept), listener));
    let Some(index) = index else {
        return Ok(());
    };
    let removed = listeners.borrow_mut().remove(index);
    removed.1.unsubscribe()
}

/// The removal an addition returns; it keeps the host remover but neither the listener nor
/// the frame writer.
struct InputRemoval {
    /// The frame writer's listeners.
    listeners: Weak<InputListeners>,
    /// The added listener.
    listener: Weak<TerminalInputFn>,
    /// The host's removal, used directly once the frame writer is gone.
    remover: Subscription,
}

impl SubscriptionPort for InputRemoval {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        match self.listeners.upgrade() {
            Some(listeners) => remove_listener(&listeners, self.listener.as_ptr()),
            None => self.remover.unsubscribe(),
        }
    }
}

/// The terminal of a frame writer widget.
struct Screen<I: WidgetImports>(Rc<HostWidget<I>>);

impl<I: WidgetImports> Screen<I> {
    /// Runs a terminal request.
    fn request(&self, request: TerminalRequest<'_>) -> ExtensionResult<WriterAnswer<I>> {
        self.0
            .imports
            .widget_writer(&self.0.widget, WriterRequest::Terminal(request))
    }
    /// Runs a terminal request that answers nothing.
    fn act(&self, request: TerminalRequest<'_>) -> ExtensionResult<()> {
        self.request(request).map(drop)
    }
    /// Runs a terminal request that answers a dimension.
    fn count(&self, request: TerminalRequest<'_>) -> ExtensionResult<u32> {
        match self.request(request)? {
            WriterAnswer::Count(count) => Ok(count),
            _ => Err(MISMATCH.to_owned()),
        }
    }
}

impl<I: WidgetImports> TerminalPort for Screen<I> {
    fn start(&self, on_input: TextHandler, on_resize: WidgetNotice) -> ExtensionResult<()> {
        let imports = &self.0.imports;
        let input = stage_handler(imports, WidgetHandler::Text(on_input));
        let resize = stage_handler(imports, WidgetHandler::Notice(on_resize));
        let result = imports
            .widget_writer(
                &self.0.widget,
                WriterRequest::TerminalStart(&input.handle, &resize.handle),
            )
            .map(drop);
        input.settle(result.is_ok());
        resize.settle(result.is_ok());
        result
    }
    fn stop(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::Stop)
    }
    fn write(&self, data: &str) -> ExtensionResult<()> {
        self.act(TerminalRequest::Write(data))
    }
    fn columns(&self) -> ExtensionResult<u32> {
        self.count(TerminalRequest::Columns)
    }
    fn rows(&self) -> ExtensionResult<u32> {
        self.count(TerminalRequest::Rows)
    }
    fn kitty_protocol_active(&self) -> ExtensionResult<bool> {
        match self.request(TerminalRequest::KittyProtocolActive)? {
            WriterAnswer::Flag(flag) => Ok(flag),
            _ => Err(MISMATCH.to_owned()),
        }
    }
    fn move_by(&self, lines: i64) -> ExtensionResult<()> {
        self.act(TerminalRequest::MoveBy(lines))
    }
    fn hide_cursor(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::HideCursor)
    }
    fn show_cursor(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::ShowCursor)
    }
    fn clear_line(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::ClearLine)
    }
    fn clear_from_cursor(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::ClearFromCursor)
    }
    fn clear_screen(&self) -> ExtensionResult<()> {
        self.act(TerminalRequest::ClearScreen)
    }
    fn set_title(&self, title: &str) -> ExtensionResult<()> {
        self.act(TerminalRequest::SetTitle(title))
    }
    fn set_progress(&self, active: bool) -> ExtensionResult<()> {
        self.act(TerminalRequest::SetProgress(active))
    }
    fn drain_input(
        &self,
        max_ms: Option<f64>,
        idle_ms: Option<f64>,
    ) -> ExtensionFuture<'static, ()> {
        let writer = self.0.clone();
        Box::pin(async move {
            writer
                .imports
                .widget_drain_input(&writer.widget, max_ms, idle_ms)
                .await
        })
    }
}
