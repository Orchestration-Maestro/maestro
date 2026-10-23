# Extension UI capabilities

`ExtensionUIContext` is an owned handle to the UI host that was current when
it was obtained. A host implements `ExtensionUIContextPort`; every operation
except `theme` has a default that returns the headless result, so a host
overrides only what it supports. The runner starts with a headless fallback
that supplies the live theme given to `ExtensionRunner::new`;
`set_ui_context(Some(..))` installs a host and `set_ui_context(None)` restores
the fallback. `has_ui` reports whether an installed host, rather than a
fallback, is current. It never inspects which operations a host supports.

```rust
use maestro_extensions::types::{ExtensionUIContextPort};
use maestro_extensions::{EventError, ExtensionRunner, ExtensionUIContext};
use maestro_models::Local;
use maestro_theme::LiveTheme;
use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

/// A host that supports only the editor text.
#[derive(Default)]
struct EditorHost {
    /// The current editor text.
    text: RefCell<String>,
}
impl ExtensionUIContextPort for EditorHost {
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

fn retained_ui_owned_host_example() -> Result<(), EventError> {
#   let auth = maestro_models::SharedHandle::new(maestro_credentials::AuthStorage::in_memory(Default::default()));
#   let session = maestro_extensions::runner::ExtensionRunnerSession {
#       cwd: "/work".into(),
#       session_manager: Rc::new(RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#       model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth)?),
#   };
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(Vec::new())),
        maestro_extensions::create_extension_runtime::<maestro_models::Local>(),
        session,
        Rc::new(|line| eprint!("{line}")),
        LiveTheme::default(),
    );
    assert!(!runner.has_ui());
    runner.set_ui_context(Some(ExtensionUIContext::new(Rc::new(EditorHost::default()))));
    let ui = runner.get_ui_context();
    runner.set_ui_context(None);
    ui.set_editor_text("draft".into())?;
    assert_eq!(ui.get_editor_text()?, "draft");
    assert_eq!(runner.get_ui_context().get_editor_text()?, "");
    Ok(())
}
# fn main() -> Result<(), EventError> {
# retained_ui_owned_host_example()
# }
```

A retained handle keeps its host after the runner installs another one. The
handle, its factories and the components they build are local values: they use
`Rc`, may hold terminal objects and stay on the thread that owns the terminal.
A UI handle cannot enter a task that may move to another thread:

```compile_fail,E0277
use maestro_extensions::ExtensionUIContext;

fn retained_ui_cannot_enter_send_task(ui: ExtensionUIContext) {
    tokio::spawn(async move {
        let _ = ui.get_editor_text();
    });
}
```

## Headless results

Without an installed host, dialogs settle with no choice (`select`, `input`,
`editor`, `custom`) or `false` (`confirm`). Notifications, status, working
message, visibility and indicator, hidden-thinking label, widgets, footer,
header, title, paste, editor text, autocomplete and editor factory setters and
tool expansion changes have no effect, and no supplied factory or handler is
called. `on_terminal_input` returns a removal that does nothing.
`get_editor_text` returns an empty string, `get_editor_component` and
`get_theme` return `None`, `get_all_themes` returns an empty list and
`get_tools_expanded` returns `false`. `set_theme` returns
`ThemeSetResult { success: false, error: Some("UI not available") }` for a name
or an instance.

## Hosts and callbacks

Optional arguments are passed through unchanged: an absent value stays absent,
an empty string or list stays present, and millisecond numbers are not
validated. An absent widget placement means above the editor.

Factories, renderers, input handlers, disposers, overlay option resolvers and
overlay handle callbacks receive a `CallbackEmitter`. A host calls them through
`EventBus::invoke_callback` on the extension's bus, so events they emit reach
other extensions' listeners right after the callback returns, as for event
listeners. A host calls a synchronous factory before its setter returns.
Disposal is explicit: the host calls `ExtensionComponent::dispose` when it
removes a component; dropping a handle does not dispose anything.

`theme()` returns a live view: a retained handle reads each later publication,
and reading it before the first publication fails with the theme owner's error.
`LiveTheme::from_theme` passes a prepared instance where a live view is
expected; it stays that instance. `get_theme` returns a prepared instance.

## Custom components

`custom_component` passes the host's close operation to the factory. The close
returns `true` on the first successful close, `false` once already closed, or
the close failure. `ExtensionUIContext::custom` builds on it with a typed
result: `done(value)` commits the value only after the host accepts the first
close, and later calls do not replace it. The returned future settles when the
host's operation settles. A committed value wins over a later failure;
otherwise the host's error is returned, or `None` when the component closed
without a value. `Some(None)` for an optional result stays distinct from no
result. The host keeps the operation alive on its own, so dropping the returned
future neither closes nor disposes the component.

## Message renderers

`ExtensionAPI::register_message_renderer` registers a renderer for a custom
message type, replacing an earlier one in its original position.
`ExtensionRunner::get_message_renderer::<T>` selects the first extension in load
order that registered the type and returns its renderer when that renderer
takes details of type `T`. A different details type on that extension is absent;
later extensions are not consulted. A renderer receives the shared
`CustomMessage<T>`, the `MessageRenderOptions` and the live theme, and returns a
component or `None` to keep the default rendering. Registration follows the
same activity rules as other registrations.
