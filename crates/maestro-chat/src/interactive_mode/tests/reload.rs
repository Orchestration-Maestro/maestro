//! Reload through the installed submission route and the retained terminal presentation.
use super::extension_editors::{Knobs, probe_factory};
use super::extension_ui::port;
use super::extension_widgets::with_factories;
use super::{Setup, World, admitted, isolated, started, submit};
use maestro_app::{
    ComponentFactory, ExtensionAPI, ExtensionComponent, ExtensionUIFuture, Local, WidgetContent,
};
use maestro_tui::{Cancellation, Component, EditorComponent, Focusable, Text};
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

/// Checks captured-editor restoration after a reachable reload failure.
async fn failed_reload(directory: &Path, stage: &'static str) {
    let (world, mut apis) = with_factories(directory, |api| vec![api.clone()]).await;
    let api = apis.pop().unwrap();
    let expands = prepare_failed_reload(&world, &api, stage);
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
    assert!(rows.contains("Reload failed:"), "{stage}: {rows}");
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
