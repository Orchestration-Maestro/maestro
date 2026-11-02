# Extension UI capabilities

`ExtensionUIContext` is an owned handle to the UI host that was current when
it was obtained. A host implements `ExtensionUIContextPort`; every operation
except `theme` has a default that returns the headless result, so a host
overrides only what it supports. The runner starts with a headless fallback
that supplies the live theme given to `ExtensionRunner::new`;
`set_ui_context(Some(..))` installs a host and `set_ui_context(None)` restores
the fallback. `has_ui` reports whether an installed host, rather than a
fallback, is current. It never inspects which operations a host supports.

`set_ui_context_binding` takes an `ExtensionUIContextBinding`, which says how
the runner keeps the host. `Owned`, built by `From<ExtensionUIContext>` and used
by `set_ui_context`, keeps the host alive. `ExtensionUIContextBinding::borrowed`
keeps only a weak reference, for a host that itself owns the session and its
runner. A context obtained through a borrowed binding is an owned handle that
keeps the host alive. Once the host is gone, the runner answers with its
headless fallback and `has_ui` is false.

Event and command contexts read the runner's current host each time `ui()` is
called, so they follow later installs. A replacement-session context reads the
replacement runner's host, not its creator's. Each handle that `ui()` returns
keeps its own host. When the last handle to a borrowed host is dropped and
nothing else holds that host, the borrowed binding no longer reaches it.

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
#   struct NoCommands;
#   impl maestro_credentials::ConfigValueOperations for NoCommands {
#       fn execute(&self, _command: &str) -> Option<Vec<u8>> {
#           None
#       }
#   }
#   let no_commands = maestro_models::SharedHandle::new(NoCommands);
#   let session = maestro_extensions::runner::ExtensionRunnerSession {
#       cwd: "/work".into(),
#       session_manager: Rc::new(RefCell::new(maestro_session::SessionManager::in_memory(None)?)),
#       model_registry: maestro_models::SharedHandle::new(maestro_catalog::ModelRegistry::in_memory(auth, no_commands)?),
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

Every UI callback is bound to the extension facade that authored it:

```rust
use maestro_extensions::types::{TerminalInputHandler, UIHostCall};
use maestro_extensions::ExtensionAPI;
use maestro_models::Local;
use maestro_tui::tui::InputListenerResult;

fn bind(api: &ExtensionAPI<Local>) {
    let handler: TerminalInputHandler = api.ui_callback(|data: String, events| {
        events.emit("received", serde_json::json!("input"));
        Ok(InputListenerResult::Replace(data))
    });
    let call: UIHostCall<String, InputListenerResult> =
        api.ui_callback(|data: String, _| Ok(InputListenerResult::Replace(data)));
    assert!(!call.ptr_eq(&handler));
    assert!(matches!(
        call.call("x".into()),
        Ok(InputListenerResult::Replace(data)) if data == "x"
    ));
}
```

`ExtensionAPI::ui_callback` keeps the facade's event capability together with
the callback, so a host calls `UIHostCall::call(operands)` and never obtains a
bus or builds an emitter. Events the callback emits reach other extensions'
listeners right after it returns, as for event listeners; listeners of the
authoring extension run inside the call. The default thread-safe facade and a
local facade both bind the same handle, which stays local: `Rc` values and
terminal objects are still the callbacks' operands and results. `call`
requires neither its operands nor its result to be `Clone`.

A handle is one allocation: `clone`, `ptr_eq`, `PartialEq` and `Hash` all follow
that allocation, so a host can key a cache or a listener set by the callback it
was given instead of wrapping every registration in a new identity. A host calls
a synchronous factory before its setter returns. Disposal is explicit: the host
calls `ExtensionComponent::dispose` when it removes a component; dropping a
handle does not dispose anything.

`theme()` returns a live view: a retained handle reads each later publication,
and reading it before the first publication fails with the theme owner's error.
`LiveTheme::from_theme` passes a prepared instance where a live view is
expected; it stays that instance. `get_theme` returns a prepared instance.

## Components and editors

An `ExtensionComponent` carries the built component and three optional parts: the
disposal the host calls when it removes the component, `set_expanded` for a header
that follows tool expansion, and `width`, which an overlay uses when no overlay
options are supplied. A plain component leaves them `None`.

Editor and custom-component factories receive `ExtensionKeybindings`, retaining the
application manager and its reload after the factory returns. Pass its `manager`
field only when constructing a toolkit `CustomEditor`.

An editor factory returns an `EditorFactoryOutcome`: `Editor` for a plain
`EditorComponent`, or `CustomEditor` for an editor that also implements
`CustomEditorComponent`. That capability exposes the live `ActionHandlers` map and
the interrupt, empty-editor exit, image-paste and shortcut callbacks, so a host can
share its application actions with the replacement. `editor()` returns the editor
through its editor operations either way, and `as_custom()` returns the capability
only when it is there.

`EditorComponent` keeps its optional operations; an unsupported one returns `None`.
The `try_` forms of text, history, insertion, expanded text, completion provider,
padding and completion height call the plain operation by default. An editor backed
by an extension component overrides them to return its original failure, and a
wrapping editor forwards them rather than calling the plain operations.

## Host widgets

`ExtensionWidgets` builds the widgets whose look depends on the host: Markdown laid out
for the host terminal, the live theme's Markdown styles with the host's code
highlighting, and the application's dynamic border. Every mode supplies one, so an
extension can build widgets without an interactive UI; only showing them goes through
the UI context. Text, boxes, containers and spacers need nothing from the host and come
from the toolkit directly. `maestro_extensions::widgets` re-exports those toolkit
widgets beside the capability, and the chat crate's `ExtensionWidgetHost` implements it.
The capability also lends the live theme's selection and settings list styles and builds the
application's bordered loader as an `ExtensionLoader`, whose signal, abort notification,
disposal and rows the extension keeps. It builds the application's keybindings managers as
`ExtensionKeybindings`. Its `reload()` uses the supplied application reload; a pathless
manager keeps its runtime overrides. `get_effective_config()` returns an owned snapshot
of resolved known actions, not raw user overrides. Clones share the manager and its reload;
ordinary toolkit operations such as `get_keys()` and `set_user_bindings()` remain available.
`create_keybindings(agent_dir)` loads through the host's existing application manager.
The capability formats keybinding hints and editor styles with the live theme. It builds
the application's custom editor as an `ExtensionEditor`, whose toolkit editor answers
the editor requests.

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
The [message renderer example](../../examples/extensions/message_renderer.md)
registers one from a component and sends the messages it draws.
