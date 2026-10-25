//! Extension shortcut selection, dispatch and ownership through the editor's first
//! interceptor.

mod autocomplete;

use super::{Setup, World, isolated};
use maestro_app::{
    Extension, ExtensionFactory, ExtensionShortcut, Local, ReadonlySessionManager, strip_ansi,
};
use maestro_tui::{Component, tui::InputHandler};
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, rc::Rc, sync::RwLock};

/// The loaded extension owners of `world`, in load order.
pub(super) fn owners(world: &World) -> Vec<Rc<RwLock<Extension<Local>>>> {
    world
        .mode
        .runtime_host
        .session()
        .resource_loader()
        .get_extensions()
        .extensions
        .clone()
}

/// An extension factory that registers nothing until the test adds shortcuts.
pub(super) fn idle() -> ExtensionFactory<Local> {
    Rc::new(|_| Box::pin(std::future::ready(Ok(()))))
}

/// Starts `world` and settles the frame it draws while starting.
async fn started(world: World) -> World {
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// A mode over one inline extension and `further` extensions, each under its path, built
/// before its shortcuts are registered.
fn built(directory: &Path, further: Vec<(String, ExtensionFactory<Local>)>) -> World {
    Setup {
        extension: Some(idle()),
        further,
        ..Setup::default()
    }
    .build(directory)
}

/// Registers `shortcut` on `owner` under the owner's own path.
pub(super) fn register(
    owner: &Rc<RwLock<Extension<Local>>>,
    mut shortcut: ExtensionShortcut<Local>,
) {
    shortcut.extension_path = owner.read().unwrap().path.clone();
    let key = shortcut.shortcut.clone();
    owner
        .write()
        .unwrap()
        .shortcuts
        .insert(key, Rc::new(RwLock::new(shortcut)));
}

/// A shortcut on `key` whose handler records `entry` and then fails with `refused`.
fn shortcut(
    key: &str,
    runs: &Rc<RefCell<Vec<Value>>>,
    entry: Value,
    refused: Option<&str>,
) -> ExtensionShortcut<Local> {
    let runs = Rc::clone(runs);
    let refused = refused.map(str::to_owned);
    ExtensionShortcut {
        shortcut: key.to_owned(),
        description: None,
        handler: Rc::new(move |_context| {
            let runs = Rc::clone(&runs);
            let entry = entry.clone();
            let refused = refused.clone();
            Box::pin(async move {
                runs.borrow_mut().push(entry);
                match refused {
                    Some(message) => Err(message.into()),
                    None => Ok(()),
                }
            })
        }),
        extension_path: String::new(),
    }
}

/// The shortcut diagnostics the session's runner retained, verbatim and in order.
fn diagnostics(world: &World) -> Vec<String> {
    let retained = world
        .mode
        .runtime_host
        .session()
        .extension_runner()
        .get_shortcut_diagnostics();
    let messages: Vec<String> = retained
        .read()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    messages
}

/// Presses `key` at the editor, then finishes the work the handler started and the frames
/// it requested.
fn press(world: &World, key: &str) {
    world.mode.editor.handle_input(key);
    world.runtime.settle().unwrap();
    let errors = world.runtime.drain_futures();
    assert!(errors.is_empty(), "the shortcut work reported {errors:?}");
}

/// The error notices the transcript holds, without their colours or `Error: ` prefix.
fn notices(world: &World) -> Vec<String> {
    world
        .mode
        .chat_container
        .render(80)
        .iter()
        .filter_map(|row| {
            strip_ansi(row)
                .ok()?
                .split_once("Error: ")
                .map(|(_, notice)| notice.trim_end().to_owned())
        })
        .collect()
}

#[test]
fn extension_shortcut_selection_reuses_reserved_and_ordered_conflict_diagnostics() {
    isolated(
        "extension_shortcut_selection_reuses_reserved_and_ordered_conflict_diagnostics",
        async |directory| {
            // An empty registration set installs no interceptor at all.
            let empty = started(built(directory, Vec::new())).await;
            assert!(empty.mode.editor.on_extension_shortcut().is_none());
            assert!(diagnostics(&empty).is_empty());
            empty.mode.editor.handle_input("\x11");
            assert_eq!(empty.runtime.futures(), 0, "no handler was started");

            // An ordinary application action's key stays usable by the extension that
            // claims it, and every later claim keeps the original slot.
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, vec![("/second/ext.js".to_owned(), idle())]);
            let owners = owners(&world);
            register(&owners[0], shortcut("ctrl+r", &runs, json!("first"), None));
            register(&owners[0], shortcut("CTRL+Q", &runs, json!("first"), None));
            register(&owners[1], shortcut("ctrl+q", &runs, json!("second"), None));
            let world = started(world).await;
            assert_eq!(
                diagnostics(&world),
                [
                    "Extension shortcut conflict: 'ctrl+r' is built-in shortcut for \
                     app.session.rename and <inline>. Using <inline>."
                        .to_owned(),
                    "Extension shortcut conflict: 'ctrl+q' registered by both <inline> and \
                     /second/ext.js. Using /second/ext.js."
                        .to_owned(),
                ]
            );
            press(&world, "\x12");
            press(&world, "\x11");
            assert_eq!(
                *runs.borrow(),
                [json!("first"), json!("second")],
                "the ordinary colliding key runs its extension and the later claim keeps the slot"
            );
        },
    );
}

#[test]
fn extension_shortcuts_never_replace_reserved_keys() {
    isolated(
        "extension_shortcuts_never_replace_reserved_keys",
        async |directory| {
            // A configured reserved action claims the key, whatever order the two actions
            // are configured in, so the extension is skipped either way.
            for config in [
                json!({"app.clear": "ctrl+q"}),
                json!({"app.clear": "ctrl+q", "app.session.rename": "ctrl+q"}),
                json!({"app.session.rename": "ctrl+q", "app.clear": "ctrl+q"}),
            ] {
                std::fs::write(directory.join("keybindings.json"), config.to_string()).unwrap();
                let runs = Rc::new(RefCell::new(Vec::new()));
                let world = built(directory, Vec::new());
                let owners = owners(&world);
                register(&owners[0], shortcut("ctrl+q", &runs, json!("ran"), None));
                let world = started(world).await;
                assert_eq!(
                    diagnostics(&world),
                    [
                        "Extension shortcut 'ctrl+q' from <inline> conflicts with built-in \
                         shortcut. Skipping."
                            .to_owned()
                    ],
                    "{config}"
                );
                press(&world, "\x11");
                assert!(runs.borrow().is_empty(), "the reserved key stays blocked");
            }

            // The same key bound to an ordinary action alone leaves it to the extension.
            std::fs::write(
                directory.join("keybindings.json"),
                json!({"app.session.rename": "ctrl+q"}).to_string(),
            )
            .unwrap();
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            register(&owners[0], shortcut("ctrl+q", &runs, json!("ran"), None));
            let world = started(world).await;
            assert_eq!(
                diagnostics(&world),
                [
                    "Extension shortcut conflict: 'ctrl+q' is built-in shortcut for \
                     app.session.rename and <inline>. Using <inline>."
                        .to_owned()
                ]
            );
            press(&world, "\x11");
            assert_eq!(*runs.borrow(), [json!("ran")]);
        },
    );
}

#[test]
fn extension_shortcut_input_consumes_only_the_first_match() {
    isolated(
        "extension_shortcut_input_consumes_only_the_first_match",
        async |directory| {
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            register(
                &owners[0],
                shortcut("ctrl+q", &runs, json!("matched"), None),
            );
            let world = started(world).await;
            let editor = Rc::clone(&world.mode.editor);
            assert!(editor.on_extension_shortcut().is_some());

            editor.set_text("");
            editor.handle_input("\x11");
            assert_eq!(editor.get_text(), "", "the matching input is consumed");
            assert!(
                runs.borrow().is_empty(),
                "the handler starts without blocking input"
            );
            world.runtime.settle().unwrap();
            assert!(world.runtime.drain_futures().is_empty());
            assert_eq!(
                *runs.borrow(),
                [json!("matched")],
                "the matching encoding starts its handler once"
            );

            runs.borrow_mut().clear();
            editor.handle_input("q");
            assert_eq!(editor.get_text(), "q", "an unmatched input edits normally");
            editor.set_text("");
            editor.handle_input("\x1bq");
            world.runtime.settle().unwrap();
            assert!(world.runtime.drain_futures().is_empty());
            assert!(
                runs.borrow().is_empty(),
                "the same key with another modifier reaches no handler"
            );
        },
    );
}

#[test]
fn shortcut_context_rejects_stale_handles_and_uses_the_current_runner() {
    isolated(
        "shortcut_context_rejects_stale_handles_and_uses_the_current_runner",
        async |directory| {
            let log: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            let runs = Rc::clone(&log);
            let handler: ExtensionShortcut<Local> = ExtensionShortcut {
                shortcut: "ctrl+q".to_owned(),
                description: None,
                handler: Rc::new(move |context| {
                    let runs = Rc::clone(&runs);
                    Box::pin(async move {
                        runs.borrow_mut().push(json!({
                            "cwd": context.cwd().ok(),
                            "idle": context.is_idle().ok(),
                            "prompt": context.get_system_prompt().ok().map(|prompt| !prompt.is_empty()),
                            "pending": context.has_pending_messages().ok(),
                        }));
                        Ok(())
                    })
                }),
                extension_path: String::new(),
            };
            register(&owners[0], handler);
            let world = started(world).await;

            let session = world.mode.runtime_host.session();
            let installed = session.extension_runner();
            let retained = installed.create_context();
            session.reload().await.unwrap();
            let current = session.extension_runner();
            assert!(
                !Rc::ptr_eq(&installed, &current),
                "the session reads a rebuilt runner"
            );

            press(&world, "\x11");
            let notices = notices(&world);
            assert!(
                notices.is_empty(),
                "the current runner answers: {notices:?}"
            );
            assert_eq!(
                *log.borrow(),
                [json!({
                    "cwd": session.session_manager().borrow().get_cwd(),
                    "idle": true,
                    "prompt": true,
                    "pending": false,
                })],
                "the next key reads the session's live state"
            );

            installed.invalidate(None);
            assert!(
                retained.is_idle().is_err(),
                "a context kept from the retired runner fails as stale"
            );
        },
    );
}

#[test]
fn extension_shortcut_rejection_keeps_its_error_notice() {
    isolated(
        "extension_shortcut_rejection_keeps_its_error_notice",
        async |directory| {
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            register(
                &owners[0],
                shortcut("ctrl+q", &runs, json!("rejected"), Some("rejected")),
            );
            let world = started(world).await;
            press(&world, "\x11");
            assert_eq!(notices(&world), ["Shortcut handler error: rejected"]);
            let editor = Rc::clone(&world.mode.editor);
            editor.handle_input("x");
            assert_eq!(editor.get_text(), "x", "later input still edits");
        },
    );
}

#[test]
fn empty_shortcut_rebinding_removes_the_previous_input_interceptor() {
    isolated(
        "empty_shortcut_rebinding_removes_the_previous_input_interceptor",
        async |directory| {
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            register(&owners[0], shortcut("ctrl+w", &runs, json!("ran"), None));
            let world = started(world).await;
            let editor = Rc::clone(&world.mode.editor);

            editor.set_text("ab cd");
            press(&world, "\x17");
            assert_eq!(editor.get_text(), "ab cd", "the claimed input is consumed");
            assert_eq!(*runs.borrow(), [json!("ran")]);

            let retired = Rc::downgrade(&runs);
            drop(runs);
            owners[0].write().unwrap().shortcuts.clear();
            world
                .mode
                .setup_extension_shortcuts(&world.mode.runtime_host.session().extension_runner());
            assert!(editor.on_extension_shortcut().is_none());
            assert!(
                retired.upgrade().is_none(),
                "the retired callback is released"
            );

            editor.set_text("ab cd");
            press(&world, "\x17");
            assert_eq!(
                editor.get_text(),
                "ab ",
                "ordinary input reaches the editor once the interceptor is gone"
            );
            assert!(retired.upgrade().is_none());
        },
    );
}

#[test]
fn shortcut_callbacks_release_the_mode_after_work_settles() {
    isolated(
        "shortcut_callbacks_release_the_mode_after_work_settles",
        async |directory| {
            let runs = Rc::new(RefCell::new(Vec::new()));
            let world = built(directory, Vec::new());
            let owners = owners(&world);
            register(&owners[0], shortcut("ctrl+q", &runs, json!("ran"), None));
            let world = started(world).await;

            let World {
                mode,
                terminal: _terminal,
                runtime,
                themes: _themes,
                host: _host,
                loader: _loader,
            } = world;
            let editor = Rc::clone(&mode.editor);
            let weak = Rc::downgrade(&mode);
            editor.handle_input("\x11");
            drop(mode);
            assert!(
                weak.upgrade().is_some(),
                "started work keeps the mode until it settles"
            );
            runtime.settle().unwrap();
            assert!(runtime.drain_futures().is_empty());
            assert!(
                weak.upgrade().is_none(),
                "the installed interceptor does not keep the mode alive"
            );
            assert_eq!(*runs.borrow(), [json!("ran")]);
            editor.handle_input("\x11");
            assert_eq!(
                runtime.futures(),
                0,
                "the retired interceptor starts nothing"
            );
        },
    );
}
