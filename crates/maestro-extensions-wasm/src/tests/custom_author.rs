//! Custom factories keep arbitrary guest results and their owned asynchronous continuations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactoryOutcome,
    ExtensionComponent, ExtensionResult, ExtensionUIContext, TUI,
};
use std::{cell::Cell, rc::Rc, task::Poll};

/// A component whose input commits its original, nonserializable guest value.
struct View {
    /// The original value.
    value: Rc<Cell<u32>>,
    /// Typed completion owned after the factory returns.
    done: CustomDone<Rc<Cell<u32>>>,
    /// A terminal retained after the factory returns.
    tui: TUI,
}
impl ComponentView for View {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.tui.full_redraws()?;
        Ok(vec![format!("custom {}", self.value.get())])
    }
    fn handle_input(&self, _: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        (self.done)(self.value.clone())?;
        (self.done)(Rc::new(Cell::new(99)))
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Runs both factory forms without serializing the result through the host.
pub async fn run(ui: &ExtensionUIContext, asynchronous: bool) -> ExtensionResult<String> {
    let value = Rc::new(Cell::new(23));
    let original = value.clone();
    let disposed = Rc::new(Cell::new(0));
    let drops = disposed.clone();
    let result = ui
        .custom(
            Rc::new(move |tui, _, keys, done, _| {
                keys.get_user_bindings()?;
                let disposal = drops.clone();
                let built = ExtensionComponent {
                    component: Component::new(View {
                        value: value.clone(),
                        done,
                        tui,
                    }),
                    dispose: Some(Rc::new(move |_| {
                        disposal.set(disposal.get() + 1);
                        Ok(())
                    })),
                    set_expanded: None,
                    width: Some(13.0),
                };
                if !asynchronous {
                    return Ok(CustomFactoryOutcome::Ready(built));
                }
                Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                    yield_once().await;
                    Ok(built)
                })))
            }),
            None,
        )
        .await?;
    assert!(result.is_some_and(|result| Rc::ptr_eq(&result, &original)));
    assert_eq!(disposed.get(), 1);
    Ok(if asynchronous {
        "custom async done"
    } else {
        "custom ready done"
    }
    .into())
}

/// Forces a continuation beyond its first poll without a timer or a borrowed emitter.
async fn yield_once() {
    let mut yielded = false;
    std::future::poll_fn(|context| {
        if yielded {
            return Poll::Ready(());
        }
        yielded = true;
        context.waker().wake_by_ref();
        Poll::Pending
    })
    .await;
}
