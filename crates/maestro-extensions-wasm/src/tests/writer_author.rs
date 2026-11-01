//! A widget that drives the frame writer and terminal its factory received.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionComponent, ExtensionResult,
    ExtensionUIContext, OverlayHandle, OverlayOptions, OverlayOptionsHandle, TUI,
    TerminalInputHandler, TerminalInputResult,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

/// Fixed rows.
struct Rows(Vec<String>);
impl ComponentView for Rows {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(self.0.clone())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A view that keeps a token, so a test sees when every owner released it.
struct Token(Rc<()>);
impl ComponentView for Token {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("token held {}", Rc::strong_count(&self.0))])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Rows a test reads back.
type Log = Rc<RefCell<Vec<String>>>;

/// The shown widget: input names a frame writer request and its rows are the answers.
struct Driver {
    /// The frame writer the factory received.
    tui: TUI,
    /// The answers, in order.
    log: Log,
    /// An authored child.
    child: Component,
    /// An authored overlay and focus target.
    target: Component,
    /// The shown overlay's handle.
    overlay: RefCell<Option<OverlayHandle>>,
    /// An input listener that consumes `x`.
    listener: TerminalInputHandler,
    /// The token of the last retired focus and overlay target.
    retired: RefCell<Weak<()>>,
}
impl ComponentView for Driver {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(self.log.borrow().clone())
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let answer = self.run(data)?;
        self.log.borrow_mut().push(answer);
        Ok(())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
impl Driver {
    /// Runs the request `command` names and describes its answer.
    fn run(&self, command: &str) -> ExtensionResult<String> {
        let tui = &self.tui;
        Ok(match command {
            "children" => {
                tui.add_child(&self.child)?;
                tui.add_child(&self.child)?;
                tui.remove_child(&self.child)?;
                tui.invalidate()?;
                format!("rows {:?}", tui.render(20)?)
            }
            "clear" => {
                tui.clear()?;
                format!("rows {:?}", tui.render(20)?)
            }
            "settings" => self.settings()?,
            "overlay" => {
                let handle = tui.show_overlay(&self.target, None)?;
                let answer = format!(
                    "overlay {} focused {}",
                    tui.has_overlay()?,
                    handle.is_focused()?
                );
                self.overlay.replace(Some(handle));
                answer
            }
            "focus" => self.focus()?,
            "hide" => {
                tui.hide_overlay()?;
                format!("overlay {}", tui.has_overlay()?)
            }
            "listen" => {
                tui.add_input_listener(self.listener.clone())?;
                tui.add_input_listener(self.listener.clone())?;
                "listening".into()
            }
            "unlisten" => {
                tui.remove_input_listener(&self.listener)?;
                "not listening".into()
            }
            "debug" => {
                let log = self.log.clone();
                tui.set_on_debug(Some(Rc::new(move |_| {
                    log.borrow_mut().push("debug key".into());
                    Ok(())
                })))?;
                "debug set".into()
            }
            "terminal" => self.terminal()?,
            "start" => tui.start().map(|()| "started".into())?,
            "stop" => tui.stop().map(|()| "stopped".into())?,
            "render" => tui
                .request_render(true)
                .map(|()| "render requested".into())?,
            _ => return self.release(command),
        })
    }
    /// Runs a request on subscriptions, retired targets or overlay visibility.
    fn release(&self, command: &str) -> ExtensionResult<String> {
        match command {
            "resubscribe" => self.resubscribe(),
            "retire" => self.retire(),
            "released" => Ok(format!(
                "released {}",
                self.retired.borrow().strong_count() == 0
            )),
            "hidden" => self.hidden(),
            _ => Err(format!("unknown request {command}")),
        }
    }
    /// Turns both frame writer flags on and reads them back.
    fn settings(&self) -> ExtensionResult<String> {
        self.tui.set_show_hardware_cursor(true)?;
        self.tui.set_clear_on_shrink(true)?;
        Ok(format!(
            "cursor {} shrink {}",
            self.tui.get_show_hardware_cursor()?,
            self.tui.get_clear_on_shrink()?
        ))
    }
    /// Moves focus away from the overlay and back.
    fn focus(&self) -> ExtensionResult<String> {
        let handle = self.overlay.borrow().clone().ok_or("no overlay")?;
        self.tui.set_focus(None)?;
        let away = handle.is_focused()?;
        self.tui.set_focus(Some(&self.target))?;
        Ok(format!("focus {away} {}", handle.is_focused()?))
    }
    /// Removes the listener through the subscription its addition returned, which also stops
    /// retaining it, then adds it again.
    fn resubscribe(&self) -> ExtensionResult<String> {
        let subscription = self.tui.add_input_listener(self.listener.clone())?;
        let held = Rc::strong_count(&self.listener);
        subscription.unsubscribe()?;
        let released = Rc::strong_count(&self.listener) < held;
        self.tui.add_input_listener(self.listener.clone())?;
        Ok(format!("resubscribed released {released}"))
    }
    /// Focuses, blurs and refocuses a fresh component within one call, then shows and hides
    /// it as an overlay, keeping only a weak view of its token.
    fn retire(&self) -> ExtensionResult<String> {
        let token = Rc::new(());
        let target = Component::new(Token(token.clone()));
        self.tui.set_focus(Some(&target))?;
        self.tui.set_focus(None)?;
        self.tui.set_focus(Some(&target))?;
        self.tui.set_focus(None)?;
        self.tui.show_overlay(&target, None)?.hide()?;
        self.retired.replace(Rc::downgrade(&token));
        Ok("retired".into())
    }
    /// Shows the target as an overlay whose visibility check fails, then hides it.
    fn hidden(&self) -> ExtensionResult<String> {
        let options = OverlayOptionsHandle::new(OverlayOptions::default())?;
        options.set_visible(Some(Rc::new(|columns, rows| {
            Err(format!("direct overlay failed at {columns}x{rows}"))
        })))?;
        let overlay = self.tui.show_overlay(&self.target, Some(&options))?;
        let shown = self.tui.has_overlay()?;
        overlay.hide()?;
        Ok(format!("hidden overlay {shown}"))
    }
    /// Drives every terminal operation once.
    fn terminal(&self) -> ExtensionResult<String> {
        let terminal = self.tui.terminal()?;
        terminal.write("literal")?;
        terminal.move_by(-2)?;
        terminal.hide_cursor()?;
        terminal.show_cursor()?;
        terminal.clear_line()?;
        terminal.clear_from_cursor()?;
        terminal.clear_screen()?;
        terminal.set_title("title")?;
        terminal.set_progress(true)?;
        Ok(format!(
            "terminal {}x{} kitty {}",
            terminal.columns()?,
            terminal.rows()?,
            terminal.kitty_protocol_active()?
        ))
    }
}

thread_local! {
    /// The frame writer the driver widget's factory received last.
    static WRITER: RefCell<Option<TUI>> = const { RefCell::new(None) };
}

/// Discards the pending input of the last driver's terminal, within 250 milliseconds.
pub async fn drain() -> ExtensionResult<String> {
    let tui = WRITER
        .with_borrow(Clone::clone)
        .ok_or("no frame writer yet")?;
    tui.terminal()?.drain_input(Some(250.0), None).await?;
    Ok("drained".into())
}

/// Shows the driver widget.
pub fn install(ui: &ExtensionUIContext) -> ExtensionResult<()> {
    ui.set_widget_factory(
        "writer",
        Rc::new(|tui, _, _| {
            WRITER.with_borrow_mut(|writer| *writer = Some(tui.clone()));
            let log: Log = Rc::default();
            let heard = log.clone();
            let listener: TerminalInputHandler = Rc::new(move |data, _| {
                heard.borrow_mut().push(format!("listener {data}"));
                Ok(Some(TerminalInputResult {
                    consume: Some(data == "x"),
                    data: None,
                }))
            });
            Ok(ExtensionComponent {
                component: Component::new(Driver {
                    tui,
                    log,
                    child: Component::new(Rows(vec!["child".into()])),
                    target: Component::new(Rows(vec!["target".into()])),
                    overlay: RefCell::default(),
                    listener,
                    retired: RefCell::default(),
                }),
                dispose: None,
                set_expanded: None,
                width: None,
            })
        }),
        None,
    )
}
