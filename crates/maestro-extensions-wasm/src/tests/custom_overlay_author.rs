//! A custom overlay that keeps its handle and a visibility check that can fail.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactoryOutcome, CustomUIOptions,
    ExtensionComponent, ExtensionResult, ExtensionUIContext, OverlayAnchor, OverlayHandle,
    OverlayOptions, OverlayOptionsHandle, OverlayOptionsSource, SizeValue,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The overlay's view: it reports its handle's state and drives it from input.
struct View {
    /// The handle the host handed over after showing the overlay.
    handle: Rc<RefCell<Option<OverlayHandle>>>,
    /// Whether the visibility check fails.
    failing: Rc<Cell<bool>>,
    /// Typed completion.
    done: CustomDone<&'static str>,
}
impl ComponentView for View {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let handle = self.handle.borrow();
        let handle = handle.as_ref().ok_or("no handle yet")?;
        Ok(vec![format!(
            "overlay focused={} hidden={}",
            handle.is_focused()?,
            handle.is_hidden()?
        )])
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let handle = self.handle.borrow().clone().ok_or("no handle yet")?;
        match data {
            "hide" => handle.set_hidden(true),
            "show" => handle.set_hidden(false),
            "unfocus" => handle.unfocus(),
            "focus" => handle.focus(),
            "fail" => {
                self.failing.set(true);
                Ok(())
            }
            _ => (self.done)("overlay done"),
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Shows the overlay with static or resolved placement and returns its committed result.
pub async fn run(ui: &ExtensionUIContext, resolved: bool) -> ExtensionResult<String> {
    let options = OverlayOptionsHandle::new(OverlayOptions {
        width: Some(SizeValue::Cells(30)),
        anchor: Some(OverlayAnchor::TopLeft),
        ..OverlayOptions::default()
    })?;
    let failing = Rc::new(Cell::new(false));
    let fails = failing.clone();
    options.set_visible(Some(Rc::new(move |columns, rows| {
        if fails.get() {
            return Err(format!("visibility failed at {columns}x{rows}"));
        }
        Ok(true)
    })))?;
    let handle = Rc::new(RefCell::new(None));
    let received = handle.clone();
    let overlay_options = if resolved {
        OverlayOptionsSource::Resolver(Rc::new(move |_| Ok(options.clone())))
    } else {
        OverlayOptionsSource::Static(options)
    };
    let shown = handle.clone();
    let result = ui
        .custom(
            Rc::new(move |_, _, _, done, _| {
                Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                    component: Component::new(View {
                        handle: shown.clone(),
                        failing: failing.clone(),
                        done,
                    }),
                    dispose: None,
                    set_expanded: None,
                    width: None,
                }))
            }),
            Some(CustomUIOptions {
                overlay: Some(true),
                overlay_options: Some(overlay_options),
                on_handle: Some(Rc::new(move |handle, _| {
                    received.replace(Some(handle));
                    Ok(())
                })),
            }),
        )
        .await?;
    drop(handle.take());
    Ok(result.unwrap_or("overlay closed").into())
}
