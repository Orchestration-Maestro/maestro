//! Supplied-file notifications through retained native watches and callback emitters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionResult, fs::FsWatcher, io::NativeIo,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::{Poll, Waker},
};

/// Original watch and the observable effect of its callback's native bus emission.
#[derive(Default)]
struct Slot {
    /// Original native registration, retained across commands.
    watch: RefCell<Option<Rc<dyn FsWatcher>>>,
    /// Last delivered payload, without assuming native event counts.
    effect: RefCell<Option<Value>>,
    /// Wakes the consumer when a callback delivers its effect.
    waiting: RefCell<Option<Waker>>,
    /// Release observations do not themselves retain the native registration.
    released: Rc<Released>,
}

/// Callback capture destruction, independently observable from native notification counts.
#[derive(Default)]
struct Released {
    /// Number of callback captures released by their actual owners.
    count: Cell<u64>,
    /// Consumer waiting for release, with no timer or polling watchdog.
    waiting: RefCell<Option<Waker>>,
}
/// Reports when the host releases the retained callback's last capture.
struct OnDrop(Rc<Released>);
impl Drop for OnDrop {
    fn drop(&mut self) {
        self.0.count.set(self.0.count.get() + 1);
        if let Some(waker) = self.0.waiting.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// Registers file operations independently of the callback that creates each watch.
pub(super) fn register(api: &ExtensionAPI, io: Rc<dyn NativeIo>) -> ExtensionResult<()> {
    let slot = Rc::new(Slot::default());
    let weak = Rc::downgrade(&slot);
    api.events().on(
        "native-file-change",
        Rc::new(move |data, _| {
            if let Some(slot) = weak.upgrade() {
                *slot.effect.borrow_mut() = Some(data);
                if let Some(waker) = slot.waiting.borrow_mut().take() {
                    waker.wake();
                }
            }
            Ok(None)
        }),
    )?;
    api.register_command(
        "native-watch",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, context| {
                let (slot, io) = (slot.clone(), io.clone());
                Box::pin(async move {
                    let reply = match execute(&*io, &slot, &args).await {
                        Ok(value) => json!({"value":value}),
                        Err(error) => json!({"error":error}),
                    };
                    context.ui()?.set_title(&format!("NATIVE-IO:{reply}"))
                })
            }),
        },
    )
}

/// Uses the original registration and awaits an actual callback effect.
async fn execute(io: &dyn NativeIo, slot: &Rc<Slot>, args: &str) -> ExtensionResult<Value> {
    let data: Value = serde_json::from_str(args).map_err(|error| error.to_string())?;
    match data["op"].as_str() {
        Some("watch") => {
            open(io, slot, &data)?;
            Ok(Value::Null)
        }
        Some("next") => {
            std::future::poll_fn(|cx| {
                if let Some(effect) = slot.effect.borrow_mut().take() {
                    Poll::Ready(Ok(effect))
                } else {
                    *slot.waiting.borrow_mut() = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await
        }
        Some("drop") => {
            slot.watch.borrow_mut().take();
            Ok(Value::Null)
        }
        Some("released") => {
            let target = data["count"].as_u64().ok_or("missing release count")?;
            std::future::poll_fn(|cx| {
                if slot.released.count.get() >= target {
                    Poll::Ready(Ok(json!(slot.released.count.get())))
                } else {
                    *slot.released.waiting.borrow_mut() = Some(cx.waker().clone());
                    Poll::Pending
                }
            })
            .await
        }
        Some("close") => {
            slot.watch
                .borrow()
                .as_ref()
                .ok_or("no native watch")?
                .close()?;
            Ok(Value::Null)
        }
        _ => Err("unknown native watch operation".into()),
    }
}

/// Retains only a weak reference to the slot so dropping its watch can release the callback.
fn open(io: &dyn NativeIo, slot: &Rc<Slot>, data: &Value) -> ExtensionResult<()> {
    let path = data["path"]
        .as_str()
        .ok_or("missing watch path")?
        .to_owned();
    let payload = json!({"path":path});
    let close = data["self_close"].as_bool().unwrap_or(false);
    let (weak, released) = (Rc::downgrade(slot), OnDrop(slot.released.clone()));
    *slot.watch.borrow_mut() = Some(io.watch(
        path,
        Rc::new(move |emitter| {
            let _ = &released;
            emitter.emit("native-file-change", payload.clone())?;
            if close && let Some(slot) = weak.upgrade() {
                let watch = slot.watch.borrow().clone();
                if let Some(watch) = watch {
                    watch.close()?;
                }
            }
            Ok(())
        }),
    )?);
    Ok(())
}
