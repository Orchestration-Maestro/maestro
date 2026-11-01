//! Reload through the installed submission route and the retained terminal presentation.
use super::actions::shows;
use super::extension_editors::{Knobs, probe_factory};
use super::extension_ui::port;
use super::extension_widgets::{with_configured_factories, with_factories};
use super::navigation::{append_user, resume::saved};
use super::{Setup, World, admitted, isolated, isolated_output, start, started, submit};
use maestro_app::{
    AgentSessionEvent, ComponentFactory, ContextFile, DiagnosticErrorInfo, ExtensionAPI,
    ExtensionCommandContext, ExtensionComponent, ExtensionFactory, ExtensionUIFuture, ForkOptions,
    ForkPosition, Local, NavigateTreeOptions, NewSessionCommandOptions, Presence, PromptOptions,
    ReadonlySessionManager, SessionBeforeForkEvent, SessionBeforeForkResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionBeforeTreeEvent,
    SessionBeforeTreeResult, SettingsManager, StopReason, SwitchSessionOptions, WidgetContent,
};
use maestro_tui::{Cancellation, Component, EditorComponent, Focusable, Text};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    io,
    path::Path,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[test]
fn reload_submit_routes_exact_command() {
    isolated("reload_submit_routes_exact_command", async |directory| {
        let world = started(directory).await;
        for text in ["/reload extra", "/RELOAD", "/reload\u{85}"] {
            let mut input = std::pin::pin!(world.mode.get_user_input());
            let mut context = Context::from_waker(Waker::noop());
            assert!(input.as_mut().poll(&mut context).is_pending());
            submit(&world.mode, text);
            assert_eq!(input.as_mut().poll(&mut context), Poll::Ready(text.into()));
        }
        for text in ["/reload", "\u{feff}/reload\u{feff}"] {
            world.mode.editor.set_text("draft");
            let mut input = std::pin::pin!(world.mode.get_user_input());
            let mut context = Context::from_waker(Waker::noop());
            assert!(input.as_mut().poll(&mut context).is_pending());
            submit(&world.mode, text);
            assert_eq!(world.mode.editor.get_text(), "");
            assert!(input.as_mut().poll(&mut context).is_pending());
            assert!(admitted(&world.runtime).await.is_empty());
            assert!(
                world
                    .mode
                    .chat_container
                    .render(120)
                    .join("\n")
                    .contains("Reloaded keybindings")
            );
        }
    });
}

/// Whether the area presents `editor` with focus.
fn restored(world: &super::World, editor: &Rc<dyn EditorComponent<Signal = Cancellation>>) -> bool {
    let children = world.mode.editor_container.children();
    let children = children.borrow();
    children.len() == 1
        && std::ptr::addr_eq(Rc::as_ptr(&children[0]), Rc::as_ptr(editor))
        && editor
            .focusable()
            .is_some_and(|focus| focus.focus_flag().get())
}

#[test]
fn reload_paints_box_before_awaiting_application() {
    isolated(
        "reload_paints_box_before_awaiting_application",
        async |directory| {
            for pending in [0, 262_145] {
                painted_reload(directory, pending).await;
            }
        },
    );
}

#[test]
fn reload_refuses_only_streaming_and_compaction() {
    isolated(
        "reload_refuses_only_streaming_and_compaction",
        async |directory| {
            let world = started(directory).await;
            super::with_held_response(&world, async |_| {
                world.loader.calls.borrow_mut().clear();
                submit(&world.mode, "/reload");
                assert!(admitted(&world.runtime).await.is_empty());
                let rows = world.mode.chat_container.render(120).join("\n");
                assert!(
                    rows.contains("Wait for the current response to finish before reloading."),
                    "{rows}"
                );
                assert!(world.loader.calls.borrow().is_empty());
            })
            .await;
            let compact = started(directory).await;
            let runtime = compact.runtime.clone();
            super::queues::during_compaction(&compact, move |mode| {
                submit(mode, "/reload");
                let mut work = runtime.take_futures().pop().unwrap();
                let mut context = Context::from_waker(Waker::noop());
                assert!(matches!(
                    work.as_mut().poll(&mut context),
                    Poll::Ready(Ok(()))
                ));
                let rows = mode.chat_container.render(120).join("\n");
                assert!(
                    rows.contains("Wait for compaction to finish before reloading."),
                    "{rows}"
                );
            })
            .await;
            assert!(!world.loader.calls.borrow().contains(&"reload"));
            reload_during_shell(directory).await;
        },
    );
}

/// Runs reload through the installed callback and checks that its work settles normally.
async fn reload(world: &World) {
    submit(&world.mode, "/reload");
    assert!(admitted(&world.runtime).await.is_empty());
}

#[test]
fn reload_failure_restores_post_reset_editor() {
    isolated(
        "reload_failure_restores_post_reset_editor",
        async |directory| {
            let knobs = Rc::new(Knobs::default());
            let k = knobs.clone();
            let (world, mut factories) = with_factories(directory, move |api| {
                vec![probe_factory(api, &k, false, None)]
            })
            .await;
            port(&world.mode)
                .set_editor_component(factories.pop())
                .unwrap();
            let old = knobs.editor();
            assert!(old.focus_flag().get());
            world.loader.on_reload.replace(Some(Box::new(|| {
                Box::pin(async { Err(io::Error::other("application failed")) })
            })));
            reload(&world).await;
            let default: Rc<dyn EditorComponent<Signal = Cancellation>> = world.mode.editor.clone();
            assert!(restored(&world, &default));
            assert!(!old.focus_flag().get());
            let rows = world.mode.chat_container.render(120).join("\n");
            assert!(rows.contains("Reload failed: application failed"), "{rows}");
            assert!(!rows.contains("Reloaded keybindings"));
        },
    );
}

#[test]
fn reload_initial_draw_failure_restores_captured_editor() {
    isolated(
        "reload_initial_draw_failure_restores_captured_editor",
        async |directory| {
            let world = started(directory).await;
            world.loader.calls.borrow_mut().clear();
            world.terminal.clear_writes();
            world
                .terminal
                .set_hide_error(Some(io::ErrorKind::BrokenPipe));
            submit(&world.mode, "/reload");
            let errors = admitted(&world.runtime).await;
            world.terminal.set_hide_error(None);
            let editor: Rc<dyn EditorComponent<Signal = Cancellation>> = world.mode.editor.clone();
            assert!(restored(&world, &editor));
            assert!(errors.is_empty(), "{errors:?}");
            assert!(!world.loader.calls.borrow().contains(&"reload"));
            let output = world.terminal.writes().join("");
            assert!(output.contains("Reloading keybindings"), "{output}");
            let rows = world.mode.chat_container.render(160).join("\n");
            assert!(rows.contains("Reload failed: cursor failure"), "{rows}");
            assert!(!rows.contains("Reloaded keybindings"), "{rows}");
            world.terminal.send_input("resumed draft");
            assert_eq!(world.mode.editor.get_text(), "resumed draft");
        },
    );
}

#[test]
fn reload_preparation_failure_keeps_its_catch_boundary() {
    isolated(
        "reload_preparation_failure_keeps_its_catch_boundary",
        async |directory| {
            let (world, mut widgets) =
                with_factories(directory, |api| vec![failing_widget(api)]).await;
            port(&world.mode)
                .set_widget("failing".into(), widgets.pop(), None)
                .unwrap();
            world.terminal.clear_writes();
            world.loader.calls.borrow_mut().clear();
            submit(&world.mode, "/reload");
            let errors = admitted(&world.runtime).await;
            assert_eq!(errors.len(), 1);
            assert_eq!(
                errors[0].downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::PermissionDenied
            );
            assert!(!world.loader.calls.borrow().contains(&"reload"));
            let rows = world.mode.chat_container.render(120).join("\n");
            assert!(!rows.contains("Reload failed"));
            assert!(
                !world
                    .terminal
                    .writes()
                    .join("")
                    .contains("Reloading keybindings")
            );
            assert!(!world.mode.extension_widgets_above.is_empty());
        },
    );
}

#[test]
fn reload_reset_settles_abandoned_dialogs_before_loading() {
    isolated(
        "reload_reset_settles_abandoned_dialogs_before_loading",
        async |directory| {
            for kind in ["select", "input", "editor", "none"] {
                settled_dialog_reload(directory, kind).await;
            }
        },
    );
}

#[test]
fn reload_quiet_consumer_shows_diagnostics_and_model_errors() {
    isolated(
        "reload_quiet_consumer_shows_diagnostics_and_model_errors",
        async |directory| {
            for broken in [true, false] {
                quiet_reload(directory, broken).await;
            }
        },
    );
}

#[test]
fn reload_failure_restores_captured_editor_without_rollback() {
    isolated(
        "reload_failure_restores_captured_editor_without_rollback",
        async |directory| {
            for stage in [
                "application",
                "header",
                "padding",
                "height",
                "cursor",
                "provider",
                "transcript",
            ] {
                failed_reload(directory, stage).await;
            }
        },
    );
}

#[test]
fn reload_listing_failure_restores_captured_editor() {
    // A home that is not UTF-8 fails the listing's first path display, after the refresh.
    if let Some(output) = isolated_output(
        "reload_listing_failure_restores_captured_editor",
        "child",
        &[],
        "trap '' XFSZ; export HOME=$'\\xff'",
        async |directory| failed_reload(directory, "listing").await,
    ) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn reload_restores_settings_headers_and_diagnostics() {
    isolated(
        "reload_restores_settings_headers_and_diagnostics",
        async |directory| {
            for (theme, header, expanded, padding, maximum, cursor, clear) in [
                (None, "builtin", false, 3.0, 9.0, true, false),
                (Some(""), "plain", true, 1.0, 5.0, false, true),
                (Some("light"), "custom", true, 3.0, 9.0, true, false),
                (Some("missing"), "custom", false, 1.0, 5.0, false, true),
            ] {
                configured_reload(
                    directory,
                    (theme, header, expanded, padding, maximum, cursor, clear),
                )
                .await;
            }
        },
    );
}

/// Checks the loading frame while resource reload is held open.
async fn painted_reload(directory: &Path, pending: usize) {
    let world = started(directory).await;
    world.terminal.set_pending_output_bytes(pending);
    world.terminal.clear_writes();
    let (release, wait) = tokio::sync::oneshot::channel::<()>();
    let wait = RefCell::new(Some(wait));
    let (entered, entry) = tokio::sync::oneshot::channel();
    let entered = RefCell::new(Some(entered));
    world.loader.on_reload.replace(Some(Box::new(move || {
        entered.borrow_mut().take().unwrap().send(()).unwrap();
        let wait = wait.borrow_mut().take().unwrap();
        Box::pin(async move {
            wait.await.unwrap();
            Ok(())
        })
    })));
    submit(&world.mode, "/reload");
    let work = tokio::task::spawn_local(world.runtime.take_futures().pop().unwrap());
    entry.await.unwrap();
    let rows: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/interactive_reload.json"
    ))
    .unwrap();
    let theme = world.themes.theme().get().unwrap();
    assert_eq!(
        world.mode.editor_container.render(80),
        super::painted(&theme, &rows)
    );
    let output = maestro_app::strip_ansi(&world.terminal.writes().join("")).unwrap();
    assert!(
        output.contains("Reloading keybindings, extensions, skills, prompts, themes..."),
        "{output}"
    );
    assert!(!Focusable::focus_flag(world.mode.editor.as_ref()).get());
    let calls = world.loader.calls.borrow().clone();
    let last = calls.iter().rposition(|call| *call == "reload").unwrap();
    assert_eq!(&calls[last..], ["reload"]);
    release.send(()).unwrap();
    work.await.unwrap().unwrap();
    assert!(restored(&world, &world.mode.current_editor()));
}

/// Checks cancellation before the loader sees one abandoned dialog.
async fn settled_dialog_reload(directory: &Path, kind: &str) {
    let world = started(directory).await;
    let ui = port(&world.mode);
    let mut dialog: ExtensionUIFuture<Option<String>> = match kind {
        "select" => ui.select("choose".into(), vec!["a".into()], None),
        "input" => ui.input("enter".into(), None, None),
        "editor" => ui.editor("edit".into(), None),
        _ => Box::pin(async { Ok(None) }),
    };
    let mut context = Context::from_waker(Waker::noop());
    if kind != "none" {
        assert!(dialog.as_mut().poll(&mut context).is_pending());
    }
    let dialog = Rc::new(RefCell::new(Some(dialog)));
    let observation = dialog.clone();
    world.loader.on_reload.replace(Some(Box::new(move || {
        let mut dialog = observation.borrow_mut().take().unwrap();
        let mut context = Context::from_waker(Waker::noop());
        assert!(matches!(
            dialog.as_mut().poll(&mut context),
            Poll::Ready(Ok(None))
        ));
        Box::pin(std::future::ready(Ok(())))
    })));
    reload(&world).await;
    assert!(dialog.borrow().is_none());
    assert!(restored(&world, &world.mode.current_editor()));
}

/// Checks the final quiet transcript with or without broken resources.
async fn quiet_reload(directory: &Path, broken: bool) {
    for path in [
        directory.join("prompts"),
        directory.join(".maestro/prompts"),
    ] {
        std::fs::create_dir_all(&path).unwrap();
        let path = path.join("duplicate.md");
        if broken {
            std::fs::write(path, "same name").unwrap();
        } else {
            std::fs::remove_file(path).unwrap();
        }
    }
    let model = directory.join("models.json");
    std::fs::write(
        &model,
        if broken {
            "{invalid"
        } else {
            r#"{"providers":{}}"#
        },
    )
    .unwrap();
    let world = Setup {
        configure: Box::new(|settings| settings.set_quiet_startup(true)),
        ..Setup::default()
    }
    .build(directory);
    world.mode.init().await.unwrap();
    reload(&world).await;
    let rows = world.mode.chat_container.render(160).join("\n");
    assert_eq!(rows.contains("[Prompt conflicts]"), broken, "{rows}");
    assert_eq!(rows.contains("models.json error:"), broken, "{rows}");
    assert!(!rows.contains("[Prompts]"), "{rows}");
    assert!(!rows.contains("[Skills]"), "{rows}");
    let success = rows.find("Reloaded keybindings").unwrap();
    if broken {
        assert!(
            rows.find("[Prompt conflicts]").unwrap() < rows.find("models.json error:").unwrap()
        );
        assert!(rows.find("models.json error:").unwrap() < success);
    }
    assert!(restored(&world, &world.mode.current_editor()));
}

/// Starts a mode whose next reload fails at `stage`, returning it with the header's
/// expansion count.
async fn failing_world(directory: &Path, stage: &'static str) -> (World, Rc<Cell<usize>>) {
    // A quiet start lists no extension path, so only the reload reads the home.
    let quiet = Box::new(move |settings: &mut SettingsManager| {
        settings.set_quiet_startup(stage == "listing");
    });
    let (world, mut apis) =
        with_configured_factories(directory, quiet, |api| vec![api.clone()]).await;
    let api = apis.pop().unwrap();
    let expands = prepare_failed_reload(&world, &api, stage);
    (world, expands)
}

/// The failure notice a reload failing at `stage` shows.
fn failure_notice(stage: &str) -> &'static str {
    if stage == "listing" {
        "Reload failed: home directory is not valid UTF-8"
    } else {
        "Reload failed:"
    }
}

/// Checks captured-editor restoration after a reachable reload failure.
async fn failed_reload(directory: &Path, stage: &'static str) {
    let (world, expands) = failing_world(directory, stage).await;
    world
        .mode
        .chat_container
        .add_child(Rc::new(Text::new("old transcript".into(), 0, 0, None)));
    reload(&world).await;
    let default: Rc<dyn EditorComponent<Signal = Cancellation>> = world.mode.editor.clone();
    assert!(restored(&world, &default), "{stage}");
    assert_eq!(
        world.mode.custom_editor_current(),
        stage != "application",
        "{stage}"
    );
    if stage != "application" && stage != "header" {
        assert_eq!(
            world.mode.editor.get_padding_x().to_string(),
            "3",
            "{stage}"
        );
        assert_eq!(
            world.mode.editor.get_autocomplete_max_visible(),
            9,
            "{stage}"
        );
    }
    let rows = world.mode.chat_container.render(160).join("\n");
    assert!(rows.contains(failure_notice(stage)), "{stage}: {rows}");
    assert!(!rows.contains("Reloaded keybindings"), "{stage}: {rows}");
    assert_eq!(
        world
            .loader
            .calls
            .borrow()
            .iter()
            .filter(|call| **call == "reload")
            .count(),
        1
    );
    if stage == "header" {
        assert_eq!(expands.get(), 2);
    }
    if stage == "transcript" {
        assert!(!world.mode.ui.get_clear_on_shrink());
    }
    world.terminal.set_hide_error(None);
}

/// Checks distinct reloaded presentation settings at their consumers.
async fn configured_reload(
    directory: &Path,
    (theme, header, expanded, padding, maximum, cursor, clear): ReloadSettings,
) {
    let (world, mut apis) = with_factories(directory, |api| vec![api.clone()]).await;
    let api = apis.pop().unwrap();
    let (knobs, expansions) = prepare_configured_reload(
        &world,
        &api,
        (theme, header, expanded, padding, maximum, cursor, clear),
    );
    reload(&world).await;
    assert!(restored(&world, &world.mode.current_editor()));
    assert_editor_policies(&world, &knobs, padding, maximum);
    assert_eq!(world.mode.ui.get_show_hardware_cursor(), cursor);
    assert_eq!(world.mode.ui.get_clear_on_shrink(), clear);
    assert!(world.mode.hide_thinking_block.get());
    assert_reload_header_and_theme(
        &world,
        &expansions.borrow(),
        (theme, header, expanded, padding, maximum, cursor, clear),
    );
    assert_eq!(
        world.mode.keybindings.get_keys("app.tools.expand"),
        ["ctrl+g"]
    );
    let rows = world.mode.chat_container.render(160).join("\n");
    assert!(rows.contains("replayed user"), "{rows}");
    assert!(rows.contains("Reloaded keybindings"), "{rows}");
    assert!(knobs.calls.borrow().ends_with(&[
        "padding".into(),
        "height".into(),
        "provider".into()
    ]));
    assert_reload_completion(&world, &knobs).await;
}

/// Theme, header, expansion and distinct editor and terminal policies.
type ReloadSettings = (
    Option<&'static str>,
    &'static str,
    bool,
    f64,
    f64,
    bool,
    bool,
);

/// A widget whose disposer fails with an observable native error kind.
fn failing_widget(api: &ExtensionAPI<Local>) -> WidgetContent {
    let factory_api = api.clone();
    WidgetContent::Factory(api.ui_callback(move |_, _| {
        Ok(ExtensionComponent {
            component: Rc::new(Text::new("kept".into(), 0, 0, None)),
            dispose: Some(factory_api.ui_callback(|(), _| {
                Err(Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "cleanup denied",
                )))
            })),
            set_expanded: None,
            width: None,
        })
    }))
}

/// Reloads while a real shell is held until the test releases it.
async fn reload_during_shell(directory: &Path) {
    let started = directory.join("shell-started");
    let release = directory.join("shell-release");
    let script = super::scripted(
        directory,
        "reload-shell",
        &format!(
            ": > {}\nwhile [ ! -e {} ]; do :; done\nprintf 'shell done\\n'",
            started.display(),
            release.display()
        ),
    );
    let shell = super::bash::shelling(directory, &script).await;
    let ((), errors) = super::driving(&shell, async {
        submit(&shell.mode, "!held");
        super::until_exists(&started).await;
        assert!(shell.host.session().is_bash_running());
        submit(&shell.mode, "/reload");
        assert!(admitted(&shell.runtime).await.is_empty());
        assert!(shell.loader.calls.borrow().contains(&"reload"));
        std::fs::write(&release, "").unwrap();
        while super::active(&shell).is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(errors.is_empty(), "{errors:?}");
}

/// Installs reachable failures in reload's actual presentation consumers.
fn prepare_failed_reload(
    world: &World,
    api: &ExtensionAPI<Local>,
    stage: &'static str,
) -> Rc<Cell<usize>> {
    let knobs = Rc::new(Knobs::default());
    let editor = probe_factory(api, &knobs, false, None);
    let ui = port(&world.mode);
    let mode = Rc::downgrade(&world.mode);
    let terminal = world.terminal.clone();
    let failing = knobs;
    let expands = Rc::new(Cell::new(0));
    let header = failing_header(api, &expands);
    let loader = Rc::downgrade(&world.loader);
    world.loader.on_reload.replace(Some(Box::new(move || {
        if stage == "application" {
            return Box::pin(async { Err(io::Error::other("application failed")) });
        }
        let mode = mode.upgrade().unwrap();
        ui.set_editor_component(Some(editor.clone())).unwrap();
        let session = mode.runtime_host.session();
        let mut settings = session.settings_manager().borrow_mut();
        settings.set_editor_padding_x(3.0);
        settings.set_autocomplete_max_visible(9.0);
        settings.set_clear_on_shrink(false);
        drop(settings);
        if ["padding", "height", "provider"].contains(&stage) {
            failing.fail.set(Some(stage));
        }
        if stage == "header" {
            ui.set_header(Some(header.clone())).unwrap();
        }
        if stage == "listing" {
            session
                .settings_manager()
                .borrow_mut()
                .set_quiet_startup(false);
            *loader.upgrade().unwrap().context.borrow_mut() = Rc::from([ContextFile {
                path: "/AGENTS.md".into(),
                content: "context".into(),
            }]);
        }
        if stage == "cursor" {
            mode.ui.set_show_hardware_cursor(true).unwrap();
            session
                .settings_manager()
                .borrow_mut()
                .set_show_hardware_cursor(false);
            terminal.set_hide_error(Some(io::ErrorKind::BrokenPipe));
        }
        if stage == "transcript" {
            session
                .session_manager()
                .borrow_mut()
                .append_message(serde_json::json!({
                    "role":"assistant", "content":[{"type":"toolCall", "name":"bad"}], "timestamp":0
                }))
                .unwrap();
        }
        Box::pin(std::future::ready(Ok(())))
    })));
    expands
}

/// A header whose second expansion fails after installation succeeds.
fn failing_header(api: &ExtensionAPI<Local>, expands: &Rc<Cell<usize>>) -> ComponentFactory {
    let count = expands.clone();
    let expanding = api.ui_callback(move |_, _| {
        count.set(count.get() + 1);
        if count.get() == 2 {
            return Err(Box::new(io::Error::other("header failed")));
        }
        Ok(())
    });
    api.ui_callback(move |_, _| {
        Ok(ExtensionComponent {
            component: Rc::new(Text::new("header".into(), 0, 0, None)),
            dispose: None,
            set_expanded: Some(expanding.clone()),
            width: None,
        })
    })
}

/// Changes the reloaded resource settings while the application reload is entered.
fn prepare_configured_reload(
    world: &World,
    api: &ExtensionAPI<Local>,
    (theme, header, expanded, padding, maximum, cursor, clear): ReloadSettings,
) -> (Rc<Knobs>, Rc<RefCell<Vec<bool>>>) {
    let knobs = Rc::new(Knobs::default());
    let editor = probe_factory(api, &knobs, false, None);
    let mode = Rc::downgrade(&world.mode);
    let ui = port(&world.mode);
    let (custom, expansions) = traced_header(api, header);
    let path = Path::new(&world.host.cwd()).join("keybindings.json");
    std::fs::write(&path, "{}").unwrap();
    let session = world.host.session();
    session
        .session_manager()
        .borrow_mut()
        .append_message(super::transcript::user_record("replayed user"))
        .unwrap();
    world.mode.tool_output_expanded.set(expanded);
    let policy = world.host.session().settings_manager().clone();
    let previous_maximum = world.mode.editor.get_autocomplete_max_visible();
    world.loader.on_reload.replace(Some(Box::new(move || {
        std::fs::write(&path, r#"{"app.tools.expand":["ctrl+g"]}"#).unwrap();
        ui.set_editor_component(Some(editor.clone())).unwrap();
        if header != "builtin" {
            ui.set_header(Some(custom.clone())).unwrap();
        }
        let mut settings = policy.borrow_mut();
        settings.set_editor_padding_x(padding);
        settings.set_autocomplete_max_visible(maximum);
        settings.set_show_hardware_cursor(cursor);
        settings.set_clear_on_shrink(clear);
        settings.set_hide_thinking_block(true);
        settings.set_theme(theme.unwrap_or("").to_owned());
        drop(settings);
        let mode = mode.upgrade().unwrap();
        assert_eq!(mode.editor.get_autocomplete_max_visible(), previous_maximum);
        Box::pin(std::future::ready(Ok(())))
    })));
    (knobs, expansions)
}

/// A plain or expandable header, keeping every expansion observation.
fn traced_header(
    api: &ExtensionAPI<Local>,
    header: &str,
) -> (ComponentFactory, Rc<RefCell<Vec<bool>>>) {
    let expandable = header == "custom";
    let expansions = Rc::new(RefCell::new(Vec::new()));
    let seen = expansions.clone();
    let callback = api.ui_callback(move |value, _| {
        seen.borrow_mut().push(value);
        Ok(())
    });
    let custom = api.ui_callback(move |_, _| {
        Ok(ExtensionComponent {
            component: Rc::new(Text::new("custom header".into(), 0, 0, None)),
            dispose: None,
            set_expanded: expandable.then(|| callback.clone()),
            width: None,
        })
    });
    (custom, expansions)
}

/// Completes through the reloaded current editor, rather than sampling its provider.
async fn assert_reload_completion(world: &World, knobs: &Knobs) {
    knobs.editor().inner.set_text("/rel");
    for _ in 0..2 {
        world.terminal.send_input("\t");
        for work in world.runtime.take_futures() {
            work.await.unwrap();
        }
    }
    assert_eq!(knobs.editor().inner.get_text(), "/reload ");
}

/// Checks padding and completion independently on the default and current editors.
fn assert_editor_policies(world: &World, knobs: &Knobs, padding: f64, maximum: f64) {
    assert_eq!(
        world.mode.editor.get_padding_x().to_string(),
        padding.to_string()
    );
    assert_eq!(
        world.mode.editor.get_autocomplete_max_visible().to_string(),
        maximum.to_string()
    );
    assert_eq!(
        knobs.editor().inner.get_padding_x().to_string(),
        padding.to_string()
    );
    assert_eq!(
        knobs
            .editor()
            .inner
            .get_autocomplete_max_visible()
            .to_string(),
        maximum.to_string()
    );
}

/// Checks optional header expansion and selected or fallback theme independently.
fn assert_reload_header_and_theme(
    world: &World,
    expansions: &[bool],
    (theme, header, expanded, ..): ReloadSettings,
) {
    assert_eq!(
        expansions,
        if header == "custom" {
            vec![expanded, expanded]
        } else {
            vec![]
        }
    );
    let selected = world.themes.theme().get().unwrap();
    assert_eq!(
        selected.name(),
        Some(if theme == Some("light") {
            "light"
        } else {
            "dark"
        })
    );
}

/// The actual command context of the currently bound extension runner.
fn context(world: &World) -> ExtensionCommandContext {
    world
        .host
        .session()
        .extension_runner()
        .create_command_context()
}

#[test]
fn command_actions_use_live_idle_resume_and_reload_owners() {
    isolated(
        "command_actions_use_live_idle_resume_and_reload_owners",
        async |directory| {
            let world = started(directory).await;
            world.mode.editor.set_text("retained draft");
            context(&world).reload().await.unwrap();
            assert_eq!(world.mode.editor.get_text(), "retained draft");
            assert!(shows(&world.mode.chat_container, "Reloaded keybindings"));
            super::with_held_response(&world, async |_| {
                let mut idle = context(&world).wait_for_idle();
                assert!(
                    idle.as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
                world.loader.calls.borrow_mut().clear();
                context(&world).reload().await.unwrap();
                assert!(shows(
                    &world.mode.chat_container,
                    "Wait for the current response"
                ));
                assert!(!world.loader.calls.borrow().contains(&"reload"));
            })
            .await;
            context(&world).wait_for_idle().await.unwrap();
            bound_resume(&world, directory).await;
            quiet_bound_reload(directory).await;
        },
    );
}

/// The extension command context's idle wait covers the retry wait of a failed response:
/// it stays pending after the agent's own run has ended and returns once the run settles.
#[test]
fn command_wait_for_idle_covers_a_retry_wait() {
    isolated(
        "command_wait_for_idle_covers_a_retry_wait",
        async |directory| {
            let world = started(directory).await;
            let session = world.mode.runtime_host.session();
            let receiver = super::held_response(&world);
            let waiting = Rc::new(tokio::sync::Notify::new());
            let notify = Rc::clone(&waiting);
            drop(session.subscribe(Rc::new(move |event| {
                if matches!(event, AgentSessionEvent::AutoRetryStart { .. }) {
                    notify.notify_one();
                }
            })));
            let prompted = Rc::clone(&session);
            let run = tokio::task::spawn_local(async move {
                prompted.prompt("run", PromptOptions::default()).await
            });
            let held = receiver.await.unwrap();
            let failure = super::answer();
            {
                let mut message = failure.write().unwrap();
                message.stop_reason = StopReason::Error;
                message.error_message = Some("overloaded_error".to_owned());
            }
            held.stream.end(Some(failure));
            waiting.notified().await;
            let mut idle = context(&world).wait_for_idle();
            assert!(
                idle.as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending(),
                "the retry wait belongs to the run the context waits for"
            );
            session.abort_retry();
            run.await.unwrap().unwrap();
            idle.await.unwrap();
        },
    );
}

/// Session switching uses the existing resume presenter and missing-directory dialog.
async fn bound_resume(world: &World, directory: &std::path::Path) {
    for missing in [false, true] {
        let cwd = if missing {
            directory.join("gone")
        } else {
            directory.to_owned()
        };
        let path = saved(
            &directory.join("saved"),
            "other",
            &json!(cwd),
            "resumed question",
        );
        let mut switching = context(world).switch_session(path.clone(), None);
        if missing {
            assert!(
                switching
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            let shown = super::models::shown(world);
            assert!(
                shown[0]
                    .render(120)
                    .join("\n")
                    .contains("Session cwd not found")
            );
            world.terminal.send_input("\r");
        }
        assert!(!switching.await.unwrap().cancelled);
        assert_eq!(world.host.session().session_file(), Some(path));
        assert!(shows(&world.mode.chat_container, "resumed question"));
        assert!(shows(
            &world.mode.chat_container,
            if missing {
                "Resumed session in current cwd"
            } else {
                "Resumed session"
            }
        ));
    }
}

/// Quiet startup still presents real resource and model-file errors through bound reload.
async fn quiet_bound_reload(directory: &std::path::Path) {
    for parent in ["prompts", ".maestro/prompts"] {
        let path = directory.join(parent);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("duplicate.md"), "same name").unwrap();
    }
    std::fs::write(directory.join("models.json"), "{invalid").unwrap();
    let world = Setup {
        configure: Box::new(|settings| settings.set_quiet_startup(true)),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    context(&world).reload().await.unwrap();
    let rows = world.mode.chat_container.render(160).join("\n");
    assert!(rows.find("[Prompt conflicts]").unwrap() < rows.find("models.json error:").unwrap());
    assert!(rows.find("models.json error:").unwrap() < rows.find("Reloaded keybindings").unwrap());
    assert!(!rows.contains("[Prompts]"));
    assert!(!rows.contains("[Skills]"));
}

/// An extension vetoing both replacement operations through their actual events.
fn veto() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        api.on::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
            "session_before_switch".into(),
            Rc::new(|_, _| {
                Box::pin(async {
                    Ok(Some(SessionBeforeSwitchResult {
                        cancel: Presence::Present(true),
                    }))
                })
            }),
        )
        .unwrap();
        api.on::<SessionBeforeForkEvent, SessionBeforeForkResult>(
            "session_before_fork".into(),
            Rc::new(|_, _| {
                Box::pin(async {
                    Ok(Some(SessionBeforeForkResult {
                        cancel: Presence::Present(true),
                        skip_conversation_restore: Presence::Missing,
                    }))
                })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// Proves fatal replacement errors terminate through the frontend, not the caller.
fn fatal_replacements(test: &str) {
    for operation in ["new", "fork"] {
        let output = isolated_output(
            test,
            operation,
            &[],
            "trap '' XFSZ",
            async move |directory| {
                let world = Setup {
                    create: Some(Rc::new(|_| {
                        Box::pin(std::future::ready(Err(DiagnosticErrorInfo::error(
                            "replacement failed",
                        ))))
                    })),
                    ..Setup::default()
                }
                .build(directory);
                start(&world).await;
                let id = append_user(&world, "first");
                println!("\nPROBE entered {operation}");
                let result = if operation == "new" {
                    context(&world).new_session(None).await
                } else {
                    context(&world).fork(id, None).await
                };
                println!("\nPROBE survived {result:?}");
            },
        );
        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("PROBE entered"), "{output:?}");
            assert!(!stdout.contains("PROBE survived"), "{output:?}");
            assert_eq!(output.status.code(), Some(1), "{output:?}");
        }
    }
}

#[test]
fn command_actions_new_and_fork_preserve_outcomes() {
    let test = "command_actions_new_and_fork_preserve_outcomes";
    fatal_replacements(test);
    isolated(test, async |directory| {
        for cancel in [false, true] {
            new_and_fork_outcomes(directory, cancel).await;
        }
    });
}

/// Both replacement operations retain their distinct presentation on success and veto.
async fn new_and_fork_outcomes(directory: &std::path::Path, cancel: bool) {
    for fork in [false, true] {
        let world = Setup {
            extension: cancel.then(veto),
            ..Setup::default()
        }
        .build(directory);
        start(&world).await;
        let first = append_user(&world, "first");
        append_user(&world, "tail");
        world.mode.render_current_session_state().unwrap();
        world.mode.editor.set_text("draft");
        world.mode.show_working_loader();
        let retired = world.host.session();
        let result = replace(&world, first, fork).await;
        assert_eq!(result.cancelled, cancel);
        assert_eq!(Rc::ptr_eq(&retired, &world.host.session()), cancel);
        assert_eq!(shows(&world.mode.chat_container, "tail"), cancel);
        assert_eq!(
            shows(&world.mode.chat_container, "Forked to new session"),
            fork && !cancel
        );
        assert_eq!(
            world.mode.editor.get_text(),
            if fork && !cancel { "first" } else { "draft" }
        );
        if !fork {
            assert!(world.mode.working_loader().is_none());
            assert!(world.mode.status_container.render(120).is_empty());
            assert!(!shows(&world.mode.chat_container, "New session started"));
        }
    }
}

/// Invokes either replacement through the bound command context.
async fn replace(world: &World, first: String, fork: bool) -> maestro_app::SessionChangeResult {
    if fork {
        context(world).fork(first, None).await
    } else {
        context(world).new_session(None).await
    }
    .unwrap()
}

#[test]
fn command_fork_defaults_only_absent_selected_text() {
    isolated(
        "command_fork_defaults_only_absent_selected_text",
        async |directory| {
            for selected in [None, Some("")] {
                let world = started(directory).await;
                let target = match selected {
                    Some(text) => append_user(&world, text),
                    None => world
                        .host
                        .session()
                        .session_manager()
                        .borrow_mut()
                        .append_message(super::transcript::assistant(
                            &json!([{"type":"text","text":"anchor"}]),
                            "stop",
                            None,
                        ))
                        .unwrap(),
                };
                append_user(&world, "tail");
                world.mode.editor.set_text("old draft");
                assert!(
                    !context(&world)
                        .fork(
                            target,
                            Some(ForkOptions {
                                position: Some(if selected.is_some() {
                                    ForkPosition::Before
                                } else {
                                    ForkPosition::At
                                }),
                                ..ForkOptions::default()
                            })
                        )
                        .await
                        .unwrap()
                        .cancelled
                );
                assert_eq!(world.mode.editor.get_text(), "");
                assert!(shows(&world.mode.chat_container, "Forked to new session"));
            }
        },
    );
}

/// Holds navigation at the extension boundary and records the exact supplied controls.
fn navigation_gate(
    release: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<()>>>>,
    seen: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    cancel: bool,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (release, seen) = (release.clone(), seen.clone());
        api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |event, _| {
                assert!(event.preparation.user_wants_summary);
                assert_eq!(
                    event.preparation.custom_instructions,
                    Presence::Present("literal instructions".into())
                );
                assert_eq!(
                    event.preparation.replace_instructions,
                    Presence::Present(true)
                );
                assert_eq!(
                    event.preparation.label,
                    Presence::Present("literal label".into())
                );
                seen.borrow_mut().take().unwrap().send(()).unwrap();
                let release = release.borrow_mut().take().unwrap();
                Box::pin(async move {
                    release.await.unwrap();
                    Ok(Some(
                        serde_json::from_value(
                            json!({"cancel": cancel, "summary": {"summary": "controlled summary"}}),
                        )
                        .unwrap(),
                    ))
                })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

#[test]
fn command_navigation_preserves_options_editor_and_flush() {
    isolated(
        "command_navigation_preserves_options_editor_and_flush",
        async |directory| {
            for cancel in [false, true] {
                for (draft, expected) in [
                    ("", "first"),
                    ("\u{feff}", "first"),
                    ("\u{85}", "\u{85}"),
                    ("late draft", "late draft"),
                ] {
                    navigation_case(directory, cancel, draft, expected).await;
                }
            }
            completed_command_flush(directory).await;
            navigation_without_returned_text(directory).await;
        },
    );
}

/// A suspended navigation preserves controls, a late draft and queue admission.
async fn navigation_case(directory: &std::path::Path, cancel: bool, draft: &str, expected: &str) {
    let (send, receive) = tokio::sync::oneshot::channel();
    let (entered, observed) = tokio::sync::oneshot::channel();
    let world = Setup {
        extension: Some(navigation_gate(
            Rc::new(RefCell::new(Some(receive))),
            Rc::new(RefCell::new(Some(entered))),
            cancel,
        )),
        ..Setup::default()
    }
    .build(directory);
    start(&world).await;
    let first = append_user(&world, "first");
    append_user(&world, "tail");
    world.mode.render_current_session_state().unwrap();
    world.mode.compaction_queued_messages.borrow_mut().push(
        super::super::queues::CompactionQueuedMessage {
            text: "captured".into(),
            mode: maestro_app::messages::UserMessageDelivery::FollowUp,
        },
    );
    let mut navigating = context(&world).navigate_tree(
        first,
        Some(NavigateTreeOptions {
            summarize: Some(true),
            custom_instructions: Some("literal instructions".into()),
            replace_instructions: Some(true),
            label: Some("literal label".into()),
        }),
    );
    tokio::select! {
        entered = observed => entered.unwrap(),
        result = &mut navigating => panic!("navigation skipped the event: {result:?}"),
    }
    world.mode.editor.set_text(draft);
    send.send(()).unwrap();
    assert_eq!(navigating.await.unwrap().cancelled, cancel);
    assert_eq!(
        world.mode.editor.get_text(),
        if cancel { draft } else { expected }
    );
    assert_eq!(
        shows(&world.mode.chat_container, "Navigated to selected point"),
        !cancel
    );
    assert_eq!(shows(&world.mode.chat_container, "tail"), cancel);
    assert_eq!(
        world.mode.compaction_queued_messages.borrow().len(),
        usize::from(cancel)
    );
    let queued = world.runtime.take_futures();
    assert_eq!(
        queued.len(),
        usize::from(!cancel),
        "delivery is admitted, not awaited"
    );
    drop(queued);
}

/// The same captured batch reaches the real scripted provider after navigation returns.
async fn completed_command_flush(directory: &std::path::Path) {
    let scripted =
        super::prompts::Scripted::new(directory, crate::InteractiveModeOptions::default());
    let world = &scripted.world;
    start(world).await;
    let first = append_user(world, "first");
    append_user(world, "tail");
    scripted
        .provider
        .set_responses(vec![maestro_models::FauxResponseStep::Message(Box::new(
            maestro_models::faux_assistant_message(
                "answer",
                maestro_models::FauxAssistantMessageOptions::default(),
            ),
        ))]);
    world.mode.compaction_queued_messages.borrow_mut().push(
        super::super::queues::CompactionQueuedMessage {
            text: "delivered captured input".into(),
            mode: maestro_app::messages::UserMessageDelivery::FollowUp,
        },
    );
    assert!(
        !context(world)
            .navigate_tree(first, None)
            .await
            .unwrap()
            .cancelled
    );
    assert_eq!(scripted.provider.state.call_count(), 0);
    assert!(shows(
        &world.mode.chat_container,
        "Navigated to selected point"
    ));
    loop {
        let operations = world.runtime.take_futures();
        if operations.is_empty() {
            break;
        }
        for operation in operations {
            operation.await.unwrap();
        }
    }
    assert_eq!(scripted.provider.state.call_count(), 1);
    assert!(shows(
        &world.mode.chat_container,
        "delivered captured input"
    ));
    assert!(world.mode.compaction_queued_messages.borrow().is_empty());
    scripted.finish();
}

/// Absent and empty returned text leave even a whitespace-only draft untouched.
async fn navigation_without_returned_text(directory: &std::path::Path) {
    for returned in [None, Some("")] {
        let world = started(directory).await;
        let target = match returned {
            Some(text) => append_user(&world, text),
            None => world
                .host
                .session()
                .session_manager()
                .borrow_mut()
                .append_message(super::transcript::assistant(
                    &json!([{"type":"text","text":"anchor"}]),
                    "stop",
                    None,
                ))
                .unwrap(),
        };
        append_user(&world, "tail");
        world.mode.editor.set_text("\u{feff}");
        assert!(
            !context(&world)
                .navigate_tree(target, None)
                .await
                .unwrap()
                .cancelled
        );
        assert_eq!(world.mode.editor.get_text(), "\u{feff}");
        assert!(shows(
            &world.mode.chat_container,
            "Navigated to selected point"
        ));
    }
}

#[test]
fn command_navigation_propagates_its_original_failure() {
    isolated(
        "command_navigation_propagates_its_original_failure",
        async |directory| {
            let world = started(directory).await;
            world.mode.chat_container.clear();
            world.mode.chat_container.add_child(Rc::new(Text::new(
                "keep transcript".into(),
                0,
                0,
                None,
            )));
            world.mode.editor.set_text("keep draft");
            let error = context(&world)
                .navigate_tree("missing identity".into(), None)
                .await
                .unwrap_err();
            let error = error.downcast::<DiagnosticErrorInfo>().unwrap();
            assert_eq!(error.message, "Entry missing identity not found");
            assert_eq!(world.mode.editor.get_text(), "keep draft");
            assert!(shows(&world.mode.chat_container, "keep transcript"));
            assert!(!shows(
                &world.mode.chat_container,
                "Navigated to selected point"
            ));
            assert!(world.runtime.take_futures().is_empty());
            replay_failure_keeps_its_original_object(directory).await;
        },
    );
}

/// Stronger error bounds retain the custom editor's allocation through replay.
async fn replay_failure_keeps_its_original_object(directory: &std::path::Path) {
    use super::extension_editors::{Knobs, probe_factory};
    let knobs = Rc::new(Knobs::default());
    let captured = knobs.clone();
    let (world, mut factories) = super::extension_widgets::with_factories(directory, move |api| {
        vec![probe_factory(api, &captured, false, None)]
    })
    .await;
    let first = append_user(&world, "kept user");
    let target = world
        .host
        .session()
        .session_manager()
        .borrow_mut()
        .append_message(super::transcript::assistant(
            &json!([{"type":"text","text":"anchor"}]),
            "stop",
            None,
        ))
        .unwrap();
    append_user(&world, "tail");
    super::extension_ui::port(&world.mode)
        .set_editor_component(factories.pop())
        .unwrap();
    knobs.fail.set(Some("history"));
    let error = context(&world)
        .navigate_tree(target, None)
        .await
        .unwrap_err();
    assert!(knobs.is_original(error.as_ref()));
    assert!(
        world
            .host
            .session()
            .session_manager()
            .borrow()
            .get_entry(&first)
            .is_some()
    );
    assert!(!shows(
        &world.mode.chat_container,
        "Navigated to selected point"
    ));
    assert!(world.runtime.take_futures().is_empty());
}

#[test]
fn command_actions_keep_absent_options() {
    isolated("command_actions_keep_absent_options", async |directory| {
        let world = started(directory).await;
        let continued = Rc::new(Cell::new(0));
        supplied_new_options(&world, &continued).await;
        let target = append_user(&world, "fork anchor");
        let seen = continued.clone();
        assert!(
            !context(&world)
                .fork(
                    target,
                    Some(ForkOptions {
                        position: Some(ForkPosition::At),
                        with_session: Some(Rc::new(move |_| {
                            seen.set(seen.get() + 1);
                            Box::pin(async { Ok(()) })
                        })),
                    })
                )
                .await
                .unwrap()
                .cancelled
        );
        assert_eq!(continued.get(), 2);
        assert!(shows(&world.mode.chat_container, "fork anchor"));
        let target = append_user(&world, "first");
        append_user(&world, "tail");
        assert!(
            !context(&world)
                .navigate_tree(target, None)
                .await
                .unwrap()
                .cancelled
        );
        assert_eq!(world.mode.editor.get_text(), "first");
        assert!(shows(
            &world.mode.chat_container,
            "Navigated to selected point"
        ));
        supplied_switch_options(&world, directory, &continued).await;
    });
}

/// Switching preserves the supplied replacement continuation.
async fn supplied_switch_options(
    world: &World,
    directory: &std::path::Path,
    continued: &Rc<Cell<usize>>,
) {
    let path = saved(
        &directory.join("saved"),
        "options",
        &json!(directory),
        "switched",
    );
    let seen = continued.clone();
    assert!(
        !context(world)
            .switch_session(
                path,
                Some(SwitchSessionOptions {
                    with_session: Some(Rc::new(move |_| {
                        seen.set(seen.get() + 1);
                        Box::pin(async { Ok(()) })
                    })),
                })
            )
            .await
            .unwrap()
            .cancelled
    );
    assert_eq!(continued.get(), 3);
    assert!(shows(&world.mode.chat_container, "switched"));
}

/// New-session options keep the setup, parent and replacement continuation.
async fn supplied_new_options(world: &World, continued: &Rc<Cell<usize>>) {
    let seen = continued.clone();
    let setup = Rc::new(Cell::new(false));
    let prepared = setup.clone();
    let options = NewSessionCommandOptions {
        parent_session: Some("literal parent".into()),
        setup: Some(Rc::new(move |manager| {
            prepared.set(true);
            manager
                .borrow_mut()
                .append_message(super::transcript::user_record("set up message"))
                .unwrap();
            manager
                .borrow_mut()
                .append_message(super::transcript::assistant(
                    &json!([{"type":"text","text":"persisted answer"}]),
                    "stop",
                    None,
                ))
                .unwrap();
            Box::pin(async { Ok(()) })
        })),
        with_session: Some(Rc::new(move |_| {
            seen.set(seen.get() + 1);
            Box::pin(async { Ok(()) })
        })),
    };
    assert!(
        !context(world)
            .new_session(Some(options))
            .await
            .unwrap()
            .cancelled
    );
    assert!(setup.get());
    assert_eq!(continued.get(), 1);
    assert!(shows(&world.mode.chat_container, "set up message"));
    assert_eq!(
        world
            .host
            .session()
            .session_manager()
            .borrow()
            .get_header()
            .unwrap()["parentSession"],
        "literal parent"
    );
}

#[test]
fn command_actions_read_current_session_when_invoked() {
    isolated(
        "command_actions_read_current_session_when_invoked",
        async |directory| {
            let world = started(directory).await;
            context(&world).reload().await.unwrap();
            assert!(shows(&world.mode.chat_container, "Reloaded keybindings"));
            let actions = world.mode.command_context_actions();
            let old = context(&world);
            world
                .host
                .new_session(NewSessionCommandOptions::default())
                .await
                .unwrap();
            assert!(
                old.wait_for_idle().await.is_err(),
                "the runner keeps its stale-context guard"
            );
            super::with_held_response(&world, async |_| {
                let mut idle = (actions.operations().wait_for_idle)();
                assert!(
                    idle.as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
            })
            .await;
            (actions.operations().wait_for_idle)().await.unwrap();
        },
    );
}

#[test]
fn command_action_callbacks_release_the_mode() {
    isolated(
        "command_action_callbacks_release_the_mode",
        async |directory| {
            let world = started(directory).await;
            let actions = world.mode.command_context_actions();
            let runner = world.host.session().extension_runner();
            let weak = Rc::downgrade(&world.mode);
            let (send, receive) = tokio::sync::oneshot::channel();
            let receive = Rc::new(RefCell::new(Some(receive)));
            world.loader.on_reload.replace(Some(Box::new(move || {
                let receive = receive.borrow_mut().take().unwrap();
                Box::pin(async move {
                    receive.await.unwrap();
                    Ok(())
                })
            })));
            let mut running = runner.create_command_context().reload();
            assert!(
                running
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            drop(world.mode);
            assert!(weak.upgrade().is_some(), "admitted reload retains the mode");
            send.send(()).unwrap();
            running.await.unwrap();
            assert!(
                weak.upgrade().is_none(),
                "callbacks and runner do not retain the mode"
            );
            assert!((actions.operations().reload)().await.is_err());
        },
    );
}
