# Authored editor and completion contracts

The guest library defines `EditorComponent` and `AutocompleteProvider` for authored
editors and completion providers. An editor factory installs an authored editor in
place of the host's editor, and a provider factory stacks an authored provider on the
host's provider. The host-backed `Editor` and `CustomEditor` facades implement the editor
traits.

## Editors

Implement `ComponentView` for rendering and invalidation, then `EditorComponent`
for text access, input, submission/change callback setters and a change-callback
getter. Its required
`handle_input` is separate from `ComponentView`'s optional input handler. When an
editor is also used as an ordinary component, delegate that handler with
`EditorComponent::handle_input(self, data, emitter)`.

Every operation is synchronous and returns `ExtensionResult`. `CallbackEmitter`
is `Copy`: reuse it across several editor operations in one callback. Its copies
borrow the same frame and cannot outlive that invocation. Retain callbacks and
editor state, not emitters. Callbacks run after releasing any mutable borrow of editor state, so a
handler can call back into the same editor.

History, cursor insertion, expanded text, completion-provider installation,
border color and padding/completion-row setters are optional. Their defaults
return `Ok(None)`. `Some(())` means an operation ran; `None` means it is absent.
An expanded-text reader that gets `None` can fall back to `get_text`. An error is
neither absence nor a successful no-op.

## Completion providers

`AutocompleteProvider::get_suggestions` returns an asynchronous `ExtensionFuture`.
It receives the edited lines, a `CursorPosition` and `CompletionOptions`. Cursor
columns are UTF-8 byte offsets. The options borrow an `AbortSignal`; clone the
signal when keeping it independently. `force: None`, `Some(false)` and
`Some(true)` remain distinct. Providers choose how to react to cancellation.

`AutocompleteSuggestions` holds candidates and the prefix they replace.
`Ok(None)` means no suggestions; `Ok(Some(...))` with an empty candidate list is
a separate result. Candidate values, labels, descriptions and prefixes remain
literal, including empty strings and newlines.

`apply_completion` is synchronous and returns `CompletionResult`, with the edited
lines and cursor. `should_trigger_file_completion` is also synchronous. Its
default `Ok(None)` leaves the decision to the caller; `Some(false)` and
`Some(true)` are explicit decisions. Provider errors remain errors.

A wrapper can retain an `Rc<dyn AutocompleteProvider>` and delegate requests to
it.

`ExtensionUIContext::add_autocomplete_provider(factory)` stacks an
`AutocompleteProviderFactory` on the host's current provider. The factory receives that
provider and returns the provider the host uses from then on. The host keeps the factory
and runs it again whenever it rebuilds its provider stack, so each factory receives the
provider the factories before it returned. The host runs the factory before the request
returns, and the factory's error is the request's error.

A provider the host lends to a factory or to an editor forwards each request to the
host. A suggestion request keeps the caller's own signal when the host lent that signal,
so a provider that keeps the signal sees a later cancellation. Applying a completion and
the file-completion decision are synchronous, and the host provider may call back into
the extension before it answers. An authored provider's failure in an operation whose
native signature cannot return an error is reported by the provider's next suggestion
request.

The host's `Editor` and `CustomEditor` install a provider with
`set_autocomplete_provider`; an authored editor without the operation answers `None`.

## Editor factories

`ExtensionUIContext::set_editor_component(Some(factory))` replaces the host's editor
with the editor `factory` builds. The host runs the factory before the request
returns, and its error is the request's error. `None` restores the host's own editor.
`get_editor_component()` returns the factory that is current, the same `Rc` the
extension supplied, or `None` while the host's own editor is in use. A factory that
another extension set is reported as an error.

An `EditorFactory` receives the terminal, the editor styles, the keybindings manager
and a borrowed emitter. It returns an `EditorFactoryOutcome`:

- `Editor` holds an editor with only the editor operations.
- `CustomEditor` holds a `CustomEditorComponent`, an editor built on the application's
  `CustomEditor`. Its `custom_editor` names that custom editor. The host shares the
  custom editor's application actions and its interrupt, exit, image-paste and
  shortcut callbacks. Input the authored editor passes on to the custom editor runs
  those actions, and the custom editor's focus is the authored editor's focus.

`Editor` and `CustomEditor` implement the matching traits. A factory can return one as
it is, or wrap one and delegate to it, as a modal editor does for the keys it does not
keep.

The host calls the authored editor through its operations. It renders and delivers
input through the editor's component, and the input reaches
`EditorComponent::handle_input`. Submission and change callbacks the host installs
are host callbacks lent to the editor. A change callback the editor returns can be
called by the host. The host keeps the submission callback it installed; reading it
does not enter the editor. Optional operations keep their absence, so the host skips
a border, padding or expansion the editor lacks. When an operation whose native
signature cannot return an error fails, the editor's next rendering reports that
failure.

## Factory component capabilities

`ExtensionComponent` returns the component, optional `dispose` and `set_expanded`
callbacks, and an optional `width`. Set unused fields to `None`.
`ComponentExpansionHandler` receives a boolean and a borrowed `CallbackEmitter`.
It runs synchronously, can run repeatedly after the factory returns, and returns
its original error to the host. A host can also call it before its UI setter
returns. Cloned native handles share the callback; removing the shown component
does not invalidate an independently retained handle.

The width crosses the boundary unchanged, including absent, zero and fractional
values. It is an implicit overlay width for a host that supports custom overlays;
this record does not install an overlay or choose its layout.

| Protocol value | Native counterpart | Ownership |
| --- | --- | --- |
| `factory-component` | `ExtensionComponent` | Transfers the component, optional disposal and expansion callback through the existing callback owner; carries width by value. |
| `set-expanded` callback | `UIHostCall<bool, ()>` | Retained independently; each call borrows its own emission frame. Its final native handle releases the authored capture. |
| `set-editor-component`, `get-editor-component` | `ExtensionUIContextPort::set_editor_component`, `get_editor_component` | The host retains the factory identity while the factory is installed, and answers it back by identity. |
| `invoke-editor-factory`, `editor-built` | `EditorFactory`, `EditorFactoryOutcome` | Lends the terminal, styles and keybindings for the call; transfers the editor's component and editor identities, and a shared handle of the custom editor it extends. |
| `invoke-editor` | `EditorComponent` | One request per editor operation, each with its own emission frame. |
| `text-callback` | `TextCallback` | A host callback lent to the editor; dropping it keeps the host's callback. |
| `widget.border-color`, `widget.share` | `Editor::border_color`, the widget itself | Lends the host editor's border style; shares the widget handle with the host. |
| `add-autocomplete-provider`, `invoke-provider-factory` | `ExtensionUIContextPort::add_autocomplete_provider`, `AutocompleteProviderFactory` | The host retains the factory for later rebuilds; each call lends the current provider and transfers the returned provider's identity. |
| `invoke-provider` | `AutocompleteProvider` | One request per provider operation; a suggestion request owns its lent signal. |
| `completion-provider` | `AutocompleteProvider` | A host provider lent to a factory or editor; dropping it keeps the host's provider. |
| `widget.set-autocomplete-provider` | `Editor::set_autocomplete_provider` | Transfers an authored provider to the host editor. |

Disposal, component removal and final callback release remain separate effects.
Neither calling expansion nor dropping its handle invokes disposal.

### Retained frame writer

A factory's `TUI::full_redraws` reads the original host frame writer's count on
every call. Clones retain the same host capability after the factory returns;
releasing the last clone releases that capability, not the host terminal.
