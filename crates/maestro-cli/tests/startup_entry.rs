//! The command-line startup entry over its existing application owners.

/// The extracted native pair runs work on the supplied local set without starting input.
#[cfg(any(unix, windows))]
#[test]
fn native_selector_terminal_drives_the_supplied_local_set() {
    use std::rc::Rc;
    let local = Rc::new(tokio::task::LocalSet::new());
    let (terminal, runtime) = maestro_cli::input::selector::native_terminal(Rc::clone(&local));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    runtime.spawn_local(Box::pin(async move {
        sender.send(()).unwrap();
        Ok(())
    }));
    crate::support::startup::runtime()
        .block_on(local.run_until(receiver))
        .unwrap();
    drop(terminal);
}

#[cfg(unix)]
mod entry {
    use crate::startup_runtime::run;
    use crate::support::entry::{Entry, Provider, cases, user_texts};
    use crate::support::startup::{set_modified, stored_history};
    use maestro_app::output_guard::is_stdout_taken_over;
    use maestro_app::{ExtensionFactory, Local, ReadonlySessionManager};
    use maestro_cli::startup::main::MainOptions;
    use serde_json::json;
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;

    /// The application version line.
    fn version() -> String {
        format!("{}\n", maestro_app::VERSION)
    }

    /// An inline factory recording each load and registering one boolean flag.
    fn recording_factory(log: &Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
        let log = Rc::clone(log);
        Rc::new(move |api| {
            log.borrow_mut().push("extension-load".into());
            let flag = maestro_app::ExtensionFlagOptions {
                description: Some("controlled flag".into()),
                r#type: maestro_app::FlagType::Boolean,
                default: None,
            };
            Box::pin(std::future::ready(
                api.register_flag("entry-flag".into(), flag),
            ))
        })
    }

    /// Record requested module paths and supply a factory with no registrations.
    fn module_paths(e: &mut Entry) -> Rc<RefCell<Vec<String>>> {
        let paths = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&paths);
        e.module_loader = Rc::new(move |path| {
            recorded.borrow_mut().push(path.to_owned());
            let factory: ExtensionFactory<Local> =
                Rc::new(|_| Box::pin(std::future::ready(Ok(()))));
            Box::pin(std::future::ready(Ok(Some(factory))))
        });
        paths
    }

    /// Options with one recording factory.
    fn options(log: &Rc<RefCell<Vec<String>>>) -> MainOptions {
        MainOptions {
            extension_factories: vec![recording_factory(log)],
        }
    }

    /// Store a session of another, existing project under the agent's session root.
    fn foreign_session(e: &Entry, id: &str) -> (String, String) {
        let project = format!("{}/../foreign", e.commands.cwd);
        std::fs::create_dir_all(&project).unwrap();
        let project = std::fs::canonicalize(project).unwrap();
        let project = project.to_str().unwrap().to_owned();
        let directory = Path::new(&e.commands.agent_dir).join("sessions/--foreign--");
        (stored_history(&directory, "foreign", id, &project), project)
    }

    /// Count the stored session files in every session directory except the foreign one.
    fn local_sessions(e: &Entry) -> usize {
        let root = Path::new(&e.commands.agent_dir).join("sessions");
        std::fs::read_dir(root).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| !entry.file_name().to_string_lossy().contains("foreign"))
                .flat_map(|entry| std::fs::read_dir(entry.path()).unwrap())
                .count()
        })
    }

    #[test]
    fn startup_offline_selection_keeps_raw_tokens() {
        let truth: &[(&str, &[(&str, &str)])] = &[
            ("absent", &[]),
            ("empty", &[("MAESTRO_OFFLINE", "")]),
            ("zero", &[("MAESTRO_OFFLINE", "0")]),
            ("false", &[("MAESTRO_OFFLINE", "false")]),
            ("y", &[("MAESTRO_OFFLINE", "y")]),
            ("on", &[("MAESTRO_OFFLINE", "on")]),
            ("padded", &[("MAESTRO_OFFLINE", " 1")]),
            ("one", &[("MAESTRO_OFFLINE", "1")]),
            ("upper", &[("MAESTRO_OFFLINE", "TRUE")]),
            ("mixed", &[("MAESTRO_OFFLINE", "Yes")]),
            ("value-token", &[]),
            ("after-separator", &[]),
        ];
        cases(
            "entry::startup_offline_selection_keeps_raw_tokens",
            truth,
            |local, case| async move {
                let mut e = Entry::new();
                e.commands.origin = "https://controlled.invalid".into();
                e.commands
                    .release(200, &json!({"version": "999.0.0"}).to_string());
                let args: &[&str] = match case {
                    "value-token" => &["update", "--self", "--model", "--offline"],
                    "after-separator" => &["update", "--self", "--", "--offline"],
                    _ => &["update", "--self"],
                };
                let offline = matches!(
                    case,
                    "one" | "upper" | "mixed" | "value-token" | "after-separator"
                );
                assert_eq!(
                    e.run(&local, args, MainOptions::default()).await.unwrap(),
                    i32::from(matches!(case, "value-token" | "after-separator"))
                );
                assert_eq!(e.log()[0], format!("apply:{offline}"), "{case}");
                assert_eq!(
                    e.commands
                        .release_requests
                        .load(std::sync::atomic::Ordering::Relaxed),
                    usize::from(matches!(case, "absent" | "empty")),
                    "{case}"
                );
                e.settings(None, Some(json!({"packages": ["npm:missing@1.0.0"]})));
                let config = [&["config"][..], &args[1..]].concat();
                assert_eq!(
                    e.run(&local, &config, MainOptions::default())
                        .await
                        .unwrap_err()
                        .to_string(),
                    "no configuration view"
                );
                let acquired = e
                    .commands
                    .calls()
                    .iter()
                    .any(|call| call.starts_with("run "));
                assert_eq!(acquired, !offline, "{case}");
            },
        );
    }

    #[test]
    fn package_and_config_precede_parse_and_takeover() {
        run(
            "entry::package_and_config_precede_parse_and_takeover",
            |local| async move {
                let e = Entry::new();
                let log = Rc::new(RefCell::new(Vec::new()));
                let status = e.run(&local, &["list"], options(&log)).await;
                assert_eq!(status.unwrap(), 0);
                assert!(!is_stdout_taken_over());
                assert!(log.borrow().is_empty());
                let error = e.run(&local, &["config", "--version"], options(&log)).await;
                assert_eq!(error.unwrap_err().to_string(), "no configuration view");
                assert!(!is_stdout_taken_over());
                assert!(!e.stdout().contains(maestro_app::VERSION));
                let status = e.run(&local, &["--mode", "json", "--version"], options(&log));
                assert_eq!(status.await.unwrap(), 0);
                assert!(is_stdout_taken_over());
                assert!(e.stdout().ends_with(&version()));
                assert!(log.borrow().is_empty());
            },
        );
    }

    #[test]
    fn version_keeps_stream_and_early_exit_order() {
        let modes: &[(&str, &[(&str, &str)])] = &[
            ("interactive", &[]),
            ("print", &[]),
            ("json", &[]),
            ("rpc", &[]),
            ("piped", &[]),
        ];
        cases(
            "entry::version_keeps_stream_and_early_exit_order",
            modes,
            |local, case| async move {
                let e = Entry::new();
                let mut args = vec!["--version", "--export", "s.jsonl", "@a.txt", "--fork", "f"];
                args.extend(["--session", "s"]);
                match case {
                    "print" => args.push("-p"),
                    "json" => args.extend(["--mode", "json"]),
                    "rpc" => args.extend(["--mode", "rpc"]),
                    "piped" => *e.tty.borrow_mut() = [false].into(),
                    _ => {}
                }
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                assert_eq!(e.stdout(), version());
                assert_eq!(e.stderr(), "");
                assert_eq!(e.log(), ["apply:false", "unread:0"]);
                assert_eq!(is_stdout_taken_over(), case != "interactive", "{case}");
            },
        );
    }

    #[test]
    fn export_uses_first_destination_and_short_circuits() {
        run(
            "entry::export_uses_first_destination_and_short_circuits",
            |local| async move {
                let e = Entry::new();
                let first = e.run(&local, &["--export", "s.jsonl"], MainOptions::default());
                assert_eq!(first.await.unwrap(), 0);
                assert!(!is_stdout_taken_over());
                let args = ["--export", "s.jsonl", "out.html", "second", "-p"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                assert!(is_stdout_taken_over());
                assert_eq!(e.stdout(), "Exported to: exported.html\n".repeat(2));
                *e.export.borrow_mut() = Err("unreadable session".into());
                let args = [
                    "--export", "s.jsonl", "--mode", "rpc", "@a.txt", "--fork", "f", "-c",
                ];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                assert_eq!(e.stderr(), "Error: unreadable session\n");
                assert_eq!(e.stdout(), "Exported to: exported.html\n".repeat(2));
                assert_eq!(
                    e.log(),
                    [
                        "apply:false",
                        "export:s.jsonl:None",
                        "unread:0",
                        "apply:false",
                        "export:s.jsonl:Some(\"out.html\")",
                        "unread:0",
                        "apply:false",
                        "export:s.jsonl:None",
                        "unread:0",
                    ]
                );
            },
        );
    }

    #[test]
    fn rpc_rejects_files_before_fork_validation() {
        run(
            "entry::rpc_rejects_files_before_fork_validation",
            |local| async move {
                let e = Entry::new();
                e.styles.set(true);
                let args = ["--mode", "rpc", "@a.txt", "--fork", "f", "--session", "s"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                assert_eq!(
                    e.stderr(),
                    "\u{1b}[31mError: @file arguments are not supported in RPC mode\u{1b}[39m\n"
                );
                let args = ["--mode", "rpc", "--fork", "f", "--session", "s"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                assert!(e.stderr().ends_with(
                    "\u{1b}[31mError: --fork cannot be combined with --session\u{1b}[39m\n"
                ));
                assert_eq!(e.stdout(), "");
            },
        );
    }

    #[test]
    fn conversions_precede_startup_settings() {
        run(
            "entry::conversions_precede_startup_settings",
            |local| async move {
                let e = Entry::new();
                let project = format!("{}/.maestro", e.commands.cwd);
                std::fs::create_dir_all(format!("{project}/commands")).unwrap();
                std::fs::write(format!("{project}/settings.json"), "{broken").unwrap();
                let steps = Rc::clone(&e.commands.state);
                e.commands
                    .stdout
                    .set_hook(Rc::new(move || steps.steps.borrow_mut().push("stdout")));
                e.commands.state.steps.borrow_mut().clear();
                let help = e.run(&local, &["--help"], MainOptions::default()).await;
                assert_eq!(help.unwrap(), 0);
                assert!(
                    e.stdout()
                        .starts_with("Migrated Project commands/ → prompts/\n")
                );
                assert!(Path::new(&format!("{project}/prompts")).is_dir());
                let steps = e.commands.state.steps.borrow().clone();
                let settings = steps.iter().position(|step| *step == "settings").unwrap();
                assert_eq!(
                    steps[..settings],
                    ["cwd", "stdout", "stdout", "cwd", "agent"]
                );
                let stderr = e.stderr();
                let lookups = stderr.matches("(startup session lookup, project settings)");
                assert_eq!(lookups.count(), 1, "{stderr}");
            },
        );
    }

    /// How the explicit session argument selects its fixture.
    #[derive(Clone, Copy, Debug)]
    enum RefusalSelection {
        /// Open an authored file path.
        Path,
        /// Match a listed session by its local identifier.
        Local,
    }

    /// A non-session path or a listed session whose entry identity prevents opening.
    fn session_refusal(
        e: &Entry,
        file: &str,
        selection: RefusalSelection,
    ) -> (String, String, String) {
        if matches!(selection, RefusalSelection::Local) {
            let content = format!(
                "{}\n{{\"type\":\"custom\",\"id\":false}}\n",
                json!({"type":"session","version":3,"id":"selected-id","cwd":e.commands.cwd})
            );
            (
                content,
                "selected".to_owned(),
                "nonstring entry id".to_owned(),
            )
        } else {
            (
                "not a session\n".to_owned(),
                file.to_owned(),
                format!("Session file is not a valid maestro session: {file}"),
            )
        }
    }

    /// Explicit paths and local identifier matches report open failures in the run's style.
    #[test]
    fn session_open_errors_are_reported_with_run_styles() {
        run(
            "entry::session_open_errors_are_reported_with_run_styles",
            |local| async move {
                for (styles, selection) in [
                    (false, RefusalSelection::Path),
                    (true, RefusalSelection::Path),
                    (false, RefusalSelection::Local),
                    (true, RefusalSelection::Local),
                ] {
                    let e = Entry::new();
                    e.styles.set(styles);
                    let directory = format!("{}/selected", e.commands.cwd);
                    std::fs::create_dir_all(&directory).unwrap();
                    let file = format!("{directory}/broken.jsonl");
                    let (content, argument, error) = session_refusal(&e, &file, selection);
                    std::fs::write(&file, &content).unwrap();
                    let status = e
                        .run(
                            &local,
                            &["--session-dir", &directory, "--session", &argument],
                            MainOptions::default(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(status, 1);
                    let message = format!("Error: {error}");
                    assert_eq!(
                        e.stderr(),
                        format!("{}\n", maestro_theme::Theme::red(&message, styles))
                    );
                    assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
                }
            },
        );
    }

    #[test]
    fn fork_errors_are_reported_without_partial_open() {
        run(
            "entry::fork_errors_are_reported_without_partial_open",
            |local| async move {
                let e = Entry::new();
                let missing = e.run(&local, &["--fork", "absent-id"], MainOptions::default());
                assert_eq!(missing.await.unwrap(), 1);
                assert_eq!(e.stderr(), "No session found matching 'absent-id'\n");
                let file = format!("{}/broken.jsonl", e.commands.cwd);
                std::fs::write(&file, "not a session\n").unwrap();
                let broken = e
                    .run(&local, &["--fork", &file], MainOptions::default())
                    .await;
                assert_eq!(broken.unwrap(), 1);
                let line = e.stderr().lines().nth(1).unwrap().to_owned();
                assert!(line.starts_with("Error: "), "{line}");
                assert_eq!(local_sessions(&e), 0);
                let (_, _) = foreign_session(&e, "foreign-session");
                let forked = e.run(
                    &local,
                    &["--fork", "foreign-s", "--help"],
                    MainOptions::default(),
                );
                assert_eq!(forked.await.unwrap(), 0);
                assert!(!e.stdout().contains("different project"), "{}", e.stdout());
                assert_eq!(local_sessions(&e), 1);
            },
        );
    }

    #[test]
    fn global_session_confirmation_preserves_input_and_output() {
        let answers: &[(&str, &[(&str, &str)])] = &[
            ("yes", &[]),
            ("upper-cr", &[]),
            ("no", &[]),
            ("padded", &[]),
            ("eof", &[]),
            ("unterminated", &[]),
            ("print", &[]),
        ];
        cases(
            "entry::global_session_confirmation_preserves_input_and_output",
            answers,
            |local, case| async move {
                let e = Entry::new();
                let (_, project) = foreign_session(&e, "foreign-session");
                let input: &[u8] = match case {
                    "yes" | "print" => b"y\nrest",
                    "upper-cr" => b"YES\r",
                    "no" => b"no\n",
                    "padded" => b" y\n",
                    "unterminated" => b"y",
                    _ => b"",
                };
                *e.stdin.borrow_mut() = input.to_vec();
                let mut args = vec!["--session", "foreign-s", "--help"];
                if case == "print" {
                    args.push("-p");
                }
                let status = e.run(&local, &args, MainOptions::default()).await.unwrap();
                assert_eq!(status, 0, "{case}");
                let notice = format!(
                    "Session found in different project: {project}\n\
                     Fork this session into current directory? [y/N] "
                );
                let accepted = matches!(case, "yes" | "upper-cr" | "print");
                let declined = matches!(case, "no" | "padded");
                let output = e.stdout();
                assert!(output.starts_with(&notice), "{case}: {output}");
                let rest = &output[notice.len()..];
                assert_eq!(rest.starts_with("Aborted.\n"), declined, "{case}");
                assert_eq!(rest.contains("Usage:"), accepted, "{case}");
                if !accepted && !declined {
                    assert_eq!(rest, "", "{case}");
                }
                assert_eq!(local_sessions(&e), usize::from(accepted), "{case}");
                assert_eq!(is_stdout_taken_over(), case == "print");
                assert_eq!(e.stderr(), "", "{case}");
            },
        );
    }

    /// Select the custom theme, which the recording watch observes.
    fn custom_theme(e: &Entry) {
        let root = Path::new(&e.commands.cwd).parent().unwrap().join("custom");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("custom.json"),
            include_str!("fixtures/custom_theme.json"),
        )
        .unwrap();
        e.settings(Some(json!({"theme": "custom"})), None);
    }

    #[test]
    fn resume_stops_watching_after_success_cancel_or_error() {
        let outcomes: &[(&str, &[(&str, &str)])] =
            &[("cancel", &[]), ("select", &[]), ("refuse", &[])];
        cases(
            "entry::resume_stops_watching_after_success_cancel_or_error",
            outcomes,
            |local, case| async move {
                let e = Entry::new();
                custom_theme(&e);
                let directory = Path::new(&e.commands.agent_dir).join("sessions/--root--");
                let path = stored_history(&directory, "own", "own-session", &e.commands.cwd);
                *e.resume.borrow_mut() = match case {
                    "cancel" => Ok(None),
                    "select" => Ok(Some(path)),
                    _ => Err("selector refused".into()),
                };
                let result = e
                    .run(&local, &["--resume", "--help"], MainOptions::default())
                    .await;
                let log = e.log();
                let stop = log.iter().position(|entry| entry == "watch-stop").unwrap();
                assert_eq!(log[..=stop].first().unwrap(), "apply:false");
                assert_eq!(log[1..3], ["watch", "resume-presented"]);
                match case {
                    "cancel" => {
                        assert_eq!(result.unwrap(), 0);
                        assert_eq!(e.stdout(), "No session selected\n");
                        assert!(
                            log[3..stop]
                                .iter()
                                .any(|entry| entry.starts_with("listed:"))
                        );
                    }
                    "select" => {
                        assert_eq!(result.unwrap(), 0);
                        assert!(e.stdout().contains("Usage:"));
                    }
                    _ => assert_eq!(result.unwrap_err().to_string(), "selector refused"),
                }
            },
        );
    }

    /// A stored history whose working directory does not exist.
    fn missing_directory_session(e: &Entry) -> String {
        let directory = Path::new(&e.commands.agent_dir).join("sessions/--root--");
        stored_history(
            &directory,
            "gone",
            "gone-session",
            "/maestro-missing-directory",
        )
    }

    #[test]
    fn missing_cwd_fails_before_bound_services() {
        run(
            "entry::missing_cwd_fails_before_bound_services",
            |local| async move {
                let e = Entry::new();
                let path = missing_directory_session(&e);
                let log = Rc::new(RefCell::new(Vec::new()));
                for mode in [&["-p"][..], &["--mode", "json"], &["--mode", "rpc"]] {
                    let mut args = vec!["--session", path.as_str()];
                    args.extend(mode);
                    assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 1);
                }
                let message = format!(
                    "Stored session working directory does not exist: /maestro-missing-directory\n\
                     Session file: {path}\nCurrent working directory: {}\n",
                    e.commands.cwd
                );
                assert_eq!(e.stderr(), message.repeat(3));
                assert!(log.borrow().is_empty());
                assert!(
                    !e.log()
                        .iter()
                        .any(|entry| entry.starts_with("missing-presented"))
                );
            },
        );
    }

    #[test]
    fn missing_cwd_continue_reopens_fallback_once() {
        run(
            "entry::missing_cwd_continue_reopens_fallback_once",
            |local| async move {
                let e = Entry::new();
                let path = missing_directory_session(&e);
                let log = Rc::new(RefCell::new(Vec::new()));
                e.missing.set("Cancel");
                let cancelled = e.run(&local, &["--session", &path], options(&log)).await;
                assert_eq!(cancelled.unwrap(), 0);
                assert!(log.borrow().is_empty());
                e.missing.set("Continue");
                let continued = e.run(&local, &["--session", &path], options(&log)).await;
                assert_eq!(continued.unwrap(), 0);
                let presented: Vec<_> = e
                    .log()
                    .into_iter()
                    .filter(|entry| entry.starts_with("missing-presented"))
                    .collect();
                assert_eq!(presented, ["missing-presented:Continue|Cancel"; 2]);
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                assert_eq!(runtime.cwd(), e.commands.cwd);
                assert_eq!(
                    runtime.session().session_file().as_deref(),
                    Some(path.as_str())
                );
                assert_eq!(*log.borrow(), ["extension-load"]);
            },
        );
    }

    /// One explicit startup-relative operand of each resource kind.
    fn authored_resources(e: &Entry) {
        let skill = format!("{}/skills/startup", e.commands.cwd);
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            format!("{skill}/SKILL.md"),
            "---\nname: startup\ndescription: controlled\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(format!("{}/review.md", e.commands.cwd), "Review.").unwrap();
        std::fs::write(
            format!("{}/extension.js", e.commands.cwd),
            "controlled module",
        )
        .unwrap();
        std::fs::write(
            format!("{}/theme.json", e.commands.cwd),
            include_str!("fixtures/custom_theme.json"),
        )
        .unwrap();
    }

    /// The authored skill, prompt and theme, and the registered flag, survived directory
    /// replacement.
    fn assert_authored_resources(loader: &dyn maestro_app::ResourceLoader<Local>) {
        let skills = loader.get_skills();
        assert_eq!(skills.skills.len(), 1);
        assert_eq!(skills.skills[0].name, "startup");
        assert_eq!(loader.get_prompts().prompts[0].name, "review");
        assert_eq!(loader.get_themes().themes[0].name(), Some("custom"));
        let extensions = loader.get_extensions();
        assert_eq!(
            extensions
                .runtime
                .flag_values
                .read()
                .unwrap()
                .get("entry-flag"),
            Some(&maestro_app::FlagValue::Boolean(true))
        );
    }

    #[test]
    fn runtime_factory_uses_final_cwd_and_shared_auth() {
        run(
            "entry::runtime_factory_uses_final_cwd_and_shared_auth",
            |local| async move {
                let mut e = Entry::new();
                let modules = module_paths(&mut e);
                authored_resources(&e);
                let log = Rc::new(RefCell::new(Vec::new()));
                let args = [
                    "--skill",
                    "./skills/startup",
                    "--entry-flag",
                    "--extension",
                    "./extension.js",
                    "--prompt-template",
                    "./review.md",
                    "--theme",
                    "./theme.json",
                ];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                let auth = Arc::clone(&runtime.services().auth_storage);
                assert_eq!(runtime.cwd(), e.commands.cwd);
                let (foreign, project) = foreign_session(&e, "foreign-session");
                let switched = runtime
                    .switch_session(&foreign, None, maestro_app::SwitchSessionOptions::default())
                    .await
                    .unwrap();
                assert!(!switched.cancelled);
                let services = runtime.services();
                assert_eq!(services.cwd, project);
                assert!(Arc::ptr_eq(&services.auth_storage, &auth));
                assert_authored_resources(&*services.resource_loader);
                assert_eq!(*log.borrow(), ["extension-load"; 2]);
                assert_eq!(
                    *modules.borrow(),
                    vec![format!("{}/extension.js", e.commands.cwd); 2]
                );
                assert!(
                    runtime.diagnostics().is_empty(),
                    "{:?}",
                    runtime.diagnostics()
                );
            },
        );
    }

    use std::sync::Arc;

    #[test]
    fn help_and_model_listing_follow_extension_registration() {
        run(
            "entry::help_and_model_listing_follow_extension_registration",
            |local| async move {
                let e = Entry::new();
                e.models(&["first", "second"]);
                let log = Rc::new(RefCell::new(Vec::new()));
                let args = ["--help", "--list-models", "--api-key", "k"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                assert!(e.stdout().contains("--entry-flag"), "{}", e.stdout());
                assert!(!e.stdout().contains("first"));
                assert_eq!(e.stderr(), "");
                e.commands.stdout.clear();
                let args = ["--list-models", "sec", "--api-key", "k"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                assert!(e.stdout().contains("second"), "{}", e.stdout());
                assert!(!e.stdout().contains("first"));
                e.commands.stdout.clear();
                let args = ["--list-models", "nothing-matches"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                assert_eq!(e.stderr(), "");
                assert!(e.recorder.runtime.borrow().is_none());
            },
        );
    }

    #[test]
    fn rpc_keeps_stdin_for_protocol() {
        run("entry::rpc_keeps_stdin_for_protocol", |local| async move {
            let e = Entry::new();
            e.models(&["first"]);
            *e.stdin.borrow_mut() = b"{\"type\":\"get_state\"}\n".to_vec();
            *e.tty.borrow_mut() = [false].into();
            let args = ["--mode", "rpc", "first prompt"];
            assert_eq!(
                e.run(&local, &args, MainOptions::default()).await.unwrap(),
                0
            );
            assert_eq!(e.log()[1..], ["rpc", "unread:21"]);
            let provider = Provider::start(vec![200, 200]);
            e.served_models(&provider.base_url, &["first"]);
            *e.tty.borrow_mut() = [false].into();
            *e.stdin.borrow_mut() =
                b"\xEF\xBB\xBF \xC2\x85piped\r\n\xFF text \n\t\xE2\x80\x83".to_vec();
            let args = ["-p", "--model", "startup/first", "@absent"];
            let printed = e.run(&local, &args[..3], MainOptions::default()).await;
            assert_eq!(printed.unwrap(), 0);
            *e.tty.borrow_mut() = [false, true].into();
            *e.stdin.borrow_mut() = b"never read".to_vec();
            let args = ["-p", "--model", "startup/first", "argument prompt"];
            assert_eq!(
                e.run(&local, &args, MainOptions::default()).await.unwrap(),
                0
            );
            let bodies = provider.join();
            assert_eq!(user_texts(&bodies[0]), ["\u{85}piped\r\n\u{FFFD} text"]);
            assert_eq!(user_texts(&bodies[1]), ["argument prompt"]);
            let log = e.log();
            assert_eq!(
                log[log.len() - 3..],
                ["unread:0", "apply:false", "unread:10"]
            );
        });
    }

    #[test]
    fn late_piped_input_does_not_retroactively_take_stdout() {
        let timing: &[(&str, &[(&str, &str)])] = &[
            ("empty", &[("MAESTRO_TIMING", "1")]),
            ("content", &[("MAESTRO_TIMING", "1")]),
        ];
        cases(
            "entry::late_piped_input_does_not_retroactively_take_stdout",
            timing,
            |local, case| async move {
                let e = Entry::new();
                let taken = Rc::new(RefCell::new(Vec::new()));
                let observed = Rc::clone(&taken);
                e.commands.stderr.set_hook(Rc::new(move || {
                    observed.borrow_mut().push(is_stdout_taken_over());
                }));
                *e.tty.borrow_mut() = [true, false].into();
                if case == "empty" {
                    e.models(&["first"]);
                    *e.stdin.borrow_mut() = b" \n\t".to_vec();
                    assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                    assert!(e.log().contains(&"factory".to_owned()));
                } else {
                    let provider = Provider::start(vec![200]);
                    e.served_models(&provider.base_url, &["first"]);
                    *e.stdin.borrow_mut() = b"late prompt".to_vec();
                    let args = ["--model", "startup/first"];
                    assert_eq!(
                        e.run(&local, &args, MainOptions::default()).await.unwrap(),
                        0
                    );
                    assert!(!e.log().contains(&"factory".to_owned()));
                    assert_eq!(user_texts(&provider.join()[0]), ["late prompt"]);
                }
                assert!(e.stderr().contains("--- Startup Timings ---"));
                assert!(!taken.borrow().is_empty());
                assert!(taken.borrow().iter().all(|taken| !taken), "{case}");
                assert!(!is_stdout_taken_over());
            },
        );
    }

    /// One project resource of every discovered kind, and a guidance file.
    fn project_resources(e: &Entry) {
        let project = format!("{}/.maestro", e.commands.cwd);
        std::fs::create_dir_all(format!("{project}/skills/local")).unwrap();
        std::fs::write(
            format!("{project}/skills/local/SKILL.md"),
            "---\nname: local\ndescription: discovered\n---\nbody\n",
        )
        .unwrap();
        std::fs::create_dir_all(format!("{project}/prompts")).unwrap();
        std::fs::write(
            format!("{project}/prompts/review.md"),
            "Review the change.\n",
        )
        .unwrap();
        std::fs::write(format!("{}/AGENTS.md", e.commands.cwd), "Guidance.\n").unwrap();
        std::fs::create_dir_all(format!("{project}/themes")).unwrap();
        std::fs::write(
            format!("{project}/themes/custom.json"),
            include_str!("fixtures/custom_theme.json"),
        )
        .unwrap();
        std::fs::create_dir_all(format!("{project}/extensions")).unwrap();
        std::fs::write(
            format!("{project}/extensions/discovered.js"),
            "controlled module",
        )
        .unwrap();
    }

    #[test]
    fn theme_and_conversion_warnings_follow_selected_mode() {
        let modes: &[(&str, &[(&str, &str)])] =
            &[("interactive", &[]), ("print", &[]), ("failing", &[])];
        cases(
            "entry::theme_and_conversion_warnings_follow_selected_mode",
            modes,
            |local, case| async move {
                let e = Entry::new();
                custom_theme(&e);
                std::fs::create_dir_all(format!("{}/.maestro/hooks", e.commands.cwd)).unwrap();
                let log = Rc::clone(&e.recorder.log);
                e.commands
                    .stderr
                    .set_hook(Rc::new(move || log.borrow_mut().push("stderr".into())));
                let log = Rc::clone(&e.recorder.log);
                e.commands
                    .stdout
                    .set_hook(Rc::new(move || log.borrow_mut().push("stdout".into())));
                let notice = "Warning: Project hooks/ directory found. Hooks have been renamed to extensions.";
                match case {
                    "print" => {
                        let provider = Provider::start(vec![200]);
                        e.served_models(&provider.base_url, &["first"]);
                        let args = ["-p", "--model", "startup/first", "hello"];
                        assert_eq!(
                            e.run(&local, &args, MainOptions::default()).await.unwrap(),
                            0
                        );
                        provider.join();
                        assert!(!e.stdout().contains(notice));
                        assert!(!e.log().contains(&"watch".to_owned()));
                    }
                    "interactive" => {
                        e.models(&["first"]);
                        assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                        assert!(e.stdout().contains(notice), "{}", e.stdout());
                        assert!(e.stdout().ends_with("\nPress any key to continue...\n\n"));
                        let log = e.log();
                        let watch = log.iter().position(|entry| entry == "watch").unwrap();
                        let shown = log.iter().position(|entry| entry == "stdout").unwrap();
                        let factory = log.iter().position(|entry| entry == "factory").unwrap();
                        assert!(watch < shown && shown < factory, "{log:?}");
                    }
                    _ => {
                        let args = ["-e", "./absent-extension.ts"];
                        assert_eq!(
                            e.run(&local, &args, MainOptions::default()).await.unwrap(),
                            1
                        );
                        let log = e.log();
                        let shown = log.iter().position(|entry| entry == "stdout").unwrap();
                        let failed = log.iter().position(|entry| entry == "stderr").unwrap();
                        assert!(shown < failed, "{log:?}");
                        assert!(e.stderr().starts_with("Error: Failed to load extension"));
                        assert!(!log.contains(&"factory".to_owned()));
                    }
                }
            },
        );
    }

    /// Verify that a saved model without credentials still reaches RPC dispatch.
    async fn saved_model_without_auth_reaches_dispatch(local: Rc<tokio::task::LocalSet>) {
        let e = Entry::new();
        std::fs::write(
            format!("{}/models.json", e.commands.agent_dir),
            json!({"providers": {"anthropic": {
                "baseUrl": "https://example.test/v1",
                "api": "openai-completions", "models": [{"id": "unknown"}]
            }}})
            .to_string(),
        )
        .unwrap();
        e.settings(
            Some(json!({
                "defaultProvider": "anthropic", "defaultModel": "unknown"
            })),
            None,
        );
        let args = ["--mode", "rpc"];
        assert_eq!(
            e.run(&local, &args, MainOptions::default()).await.unwrap(),
            0
        );
        assert_eq!(e.stderr(), "");
        assert_eq!(e.log(), ["apply:false", "rpc", "unread:0"]);
        let runtime = e.recorder.runtime.borrow().clone().unwrap();
        assert!(
            runtime
                .services()
                .model_registry
                .get_available()
                .unwrap()
                .is_empty()
        );
        assert!(runtime.session().scoped_models().is_empty());
        let model = runtime.session().model();
        let model = model.read().unwrap();
        assert_eq!((&*model.provider, &*model.id), ("anthropic", "unknown"));
        assert_eq!(runtime.model_fallback_message(), None);
    }

    #[test]
    fn noninteractive_empty_catalog_reports_guidance_before_benchmark() {
        cases(
            "entry::noninteractive_empty_catalog_reports_guidance_before_benchmark",
            &[
                ("saved-model", &[]),
                ("ordinary", &[]),
                ("benchmark", &[("MAESTRO_STARTUP_BENCHMARK", "1")]),
            ],
            |local, case| async move {
                if case == "saved-model" {
                    saved_model_without_auth_reaches_dispatch(local).await;
                    return;
                }
                let e = Entry::new();
                e.styles.set(true);
                assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                let runtime = e.recorder.runtime.borrow_mut().take().unwrap();
                assert!(
                    runtime
                        .services()
                        .model_registry
                        .get_available()
                        .unwrap()
                        .is_empty()
                );
                assert!(e.log().contains(&"factory".to_owned()));
                let docs = maestro_app::configuration::config::get_docs_path().unwrap();
                let expected = format!(
                    "\u{1b}[31mNo models available. Use /login to log into a provider via OAuth or API key. See:\u{1b}[39m\n\u{1b}[31m  {docs}/providers.md\u{1b}[39m\n\u{1b}[31m  {docs}/models.md\u{1b}[39m\n"
                );
                for args in [&["-p"][..], &["--mode", "json"], &["--mode", "rpc"]] {
                    e.commands.stderr.clear();
                    e.recorder.log.borrow_mut().clear();
                    assert_eq!(
                        e.run(&local, args, MainOptions::default()).await.unwrap(),
                        1
                    );
                    assert_eq!(e.stderr(), expected, "{args:?}");
                    assert_eq!(e.stdout(), "");
                    assert_eq!(e.log(), ["apply:false", "unread:0"]);
                    assert!(e.recorder.runtime.borrow().is_none());
                }
                e.commands.stderr.clear();
                let args = ["--mode", "rpc", "-e", "./absent-extension.ts"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                let error = e.stderr();
                assert!(
                    error.starts_with("\u{1b}[31mError: Failed to load extension"),
                    "{error}"
                );
                assert!(!error.contains("No models available"));
                assert!(!error.contains("MAESTRO_STARTUP_BENCHMARK"));
                assert!(e.recorder.runtime.borrow().is_none());
            },
        );
    }

    #[test]
    fn benchmark_values_and_mode_error_are_exact() {
        let values: &[(&str, &[(&str, &str)])] = &[
            ("one", &[("MAESTRO_STARTUP_BENCHMARK", "1")]),
            ("upper", &[("MAESTRO_STARTUP_BENCHMARK", "TRUE")]),
            ("yes", &[("MAESTRO_STARTUP_BENCHMARK", "yes")]),
            ("padded", &[("MAESTRO_STARTUP_BENCHMARK", " 1")]),
            ("zero", &[("MAESTRO_STARTUP_BENCHMARK", "0")]),
            ("earlier", &[("MAESTRO_STARTUP_BENCHMARK", "1")]),
        ];
        cases(
            "entry::benchmark_values_and_mode_error_are_exact",
            values,
            |local, case| async move {
                let e = Entry::new();
                e.styles.set(true);
                let rejected = matches!(case, "one" | "upper" | "yes");
                if case == "earlier" {
                    let args = ["-p", "-e", "./absent-extension.ts", "hello"];
                    assert_eq!(
                        e.run(&local, &args, MainOptions::default()).await.unwrap(),
                        1
                    );
                    assert!(!e.stderr().contains("MAESTRO_STARTUP_BENCHMARK"));
                    return;
                }
                let provider = Provider::start(if rejected { Vec::new() } else { vec![200] });
                e.served_models(&provider.base_url, &["first"]);
                let args = ["-p", "--model", "startup/first", "hello"];
                let status = e.run(&local, &args, MainOptions::default()).await.unwrap();
                assert_eq!(provider.join().len(), usize::from(!rejected));
                if rejected {
                    assert_eq!(status, 1);
                    assert_eq!(
                        e.stderr(),
                        "\u{1b}[31mError: MAESTRO_STARTUP_BENCHMARK only supports interactive mode\u{1b}[39m\n"
                    );
                } else {
                    assert_eq!(status, 0);
                    assert_eq!(e.stderr(), "");
                }
            },
        );
    }

    #[test]
    fn interactive_scope_banner_respects_verbose_and_quiet() {
        run(
            "entry::interactive_scope_banner_respects_verbose_and_quiet",
            |local| async move {
                let e = Entry::new();
                e.models(&["first", "second"]);
                e.styles.set(true);
                let scope = [
                    "--models",
                    "startup/second:high,startup/first,startup/second",
                ];
                let banner = "\u{1b}[2mModel scope: second:high, first \u{1b}[90m(Ctrl+P to cycle)\u{1b}[39m\u{1b}[22m\n";
                for (quiet, verbose, expected) in [
                    (false, false, banner),
                    (true, false, ""),
                    (true, true, banner),
                ] {
                    e.commands.stdout.clear();
                    e.settings(Some(json!({"quietStartup": quiet})), None);
                    let mut args = scope.to_vec();
                    args.extend(verbose.then_some("--verbose"));
                    assert_eq!(
                        e.run(&local, &args, MainOptions::default()).await.unwrap(),
                        0
                    );
                    assert_eq!(e.stdout(), expected, "{quiet} {verbose}");
                }
                e.commands.stdout.clear();
                assert_eq!(
                    e.run(&local, &["--verbose"], MainOptions::default())
                        .await
                        .unwrap(),
                    0
                );
                assert_eq!(e.stdout(), "");
            },
        );
    }

    #[test]
    fn interactive_launch_forwards_all_startup_inputs() {
        run(
            "entry::interactive_launch_forwards_all_startup_inputs",
            |local| async move {
                let e = Entry::new();
                e.models(&["first"]);
                let agent = maestro_app::configuration::config::get_agent_dir().unwrap();
                std::fs::write(
                    format!("{agent}/settings.json"),
                    json!({"apiKeys": {"legacy": "controlled"}}).to_string(),
                )
                .unwrap();
                let file = format!("{}/notes.txt", e.commands.cwd);
                std::fs::write(&file, "file text").unwrap();
                let attached = format!("@{file}");
                let image = format!(
                    "@{}/tests/fixtures/files/tiny.png",
                    env!("CARGO_MANIFEST_DIR")
                );
                let args = [
                    attached.as_str(),
                    image.as_str(),
                    "first",
                    "second",
                    "third",
                    "--verbose",
                ];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                let options = e.recorder.options.borrow_mut().take().unwrap();
                assert!(options.verbose);
                assert_eq!(options.migrated_providers, ["legacy"]);
                assert_eq!(options.model_fallback_message, None);
                let message = options.initial_message.unwrap();
                assert!(
                    message.contains("file text") && message.ends_with("first"),
                    "{message}"
                );
                assert_eq!(
                    serde_json::to_value(options.initial_images).unwrap(),
                    json!([{
                        "type": "image", "mimeType": "image/png",
                        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="
                    }])
                );
                assert_eq!(options.initial_messages, ["second", "third"]);
                assert_eq!(options.service_origin, None);
                assert!(options.fetch.is_none());
                let log = e.log();
                assert_eq!(log[log.len() - 3..], ["factory", "run", "unread:0"]);
            },
        );
    }

    #[test]
    fn benchmark_stops_before_drain_and_returns_without_run() {
        let benchmark: &[(&str, &[(&str, &str)])] = &[(
            "benchmark",
            &[("MAESTRO_STARTUP_BENCHMARK", "1"), ("MAESTRO_TIMING", "1")],
        )];
        cases(
            "entry::benchmark_stops_before_drain_and_returns_without_run",
            benchmark,
            |local, _| async move {
                let e = Entry::new();
                custom_theme(&e);
                e.models(&["first"]);
                let log = Rc::clone(&e.recorder.log);
                e.commands
                    .stderr
                    .set_hook(Rc::new(move || log.borrow_mut().push("stderr".into())));
                assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                let log = e.log();
                let factory = log.iter().position(|entry| entry == "factory").unwrap();
                let tail: Vec<_> = log[factory..]
                    .iter()
                    .filter(|entry| *entry != "stderr")
                    .cloned()
                    .collect();
                assert_eq!(tail, ["factory", "init", "stop", "watch-stop", "unread:0"]);
                let init = log.iter().position(|entry| entry == "init").unwrap();
                assert_eq!(log[init + 1], "stderr");
                assert!(e.stderr().contains("  interactiveMode.init: "));
                assert!(!log.contains(&"run".to_owned()));
            },
        );
    }

    #[test]
    fn rpc_launch_receives_real_runtime_after_timings() {
        let timing: &[(&str, &[(&str, &str)])] = &[("rpc", &[("MAESTRO_TIMING", "1")])];
        cases(
            "entry::rpc_launch_receives_real_runtime_after_timings",
            timing,
            |local, _| async move {
                let e = Entry::new();
                e.models(&["first"]);
                let log = Rc::clone(&e.recorder.log);
                e.commands
                    .stderr
                    .set_hook(Rc::new(move || log.borrow_mut().push("stderr".into())));
                let args = ["--mode", "rpc", "positional prompt"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                let log = e.log();
                assert_eq!(log[log.len() - 3..], ["stderr", "rpc", "unread:0"]);
                assert!(e.stderr().contains("--- Startup Timings ---"));
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                assert_eq!(runtime.cwd(), e.commands.cwd);
                assert!(runtime.session().messages().read().unwrap().is_empty());
            },
        );
    }

    #[test]
    fn print_restores_output_after_success_or_nonzero_status() {
        run(
            "entry::print_restores_output_after_success_or_nonzero_status",
            |local| async move {
                let e = Entry::new();
                e.settings(Some(json!({"retry": {"enabled": false}})), None);
                let provider = Provider::start(vec![200, 200, 200, 500]);
                e.served_models(&provider.base_url, &["first"]);
                let args = ["-p", "--model", "startup/first", "first", "second"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                assert!(!is_stdout_taken_over());
                let args = ["--mode", "json", "--model", "startup/first", "json prompt"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                assert!(!is_stdout_taken_over());
                let args = ["-p", "--model", "startup/first", "failing"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                assert!(!is_stdout_taken_over());
                let prompts: Vec<_> = provider.join().iter().map(user_texts).collect();
                assert_eq!(
                    prompts,
                    [
                        vec!["first"],
                        vec!["first", "second"],
                        vec!["json prompt"],
                        vec!["failing"]
                    ]
                );
            },
        );
    }

    /// The startup timing labels printed to `stderr`, in order.
    fn timing_labels(stderr: &str) -> Vec<String> {
        stderr
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .filter_map(|line| line.split_once(": ").map(|(label, _)| label.to_owned()))
            .filter(|label| label != "TOTAL")
            .collect()
    }

    /// Remove stored credentials exactly when the runtime-factory sample is taken.
    fn clear_auth_at_factory_sample(path: String) -> Rc<dyn Fn(f64)> {
        Rc::new(move |sample| {
            if sample.total_cmp(&6.0).is_eq() {
                std::fs::write(&path, "{}").unwrap();
            }
        })
    }

    #[test]
    fn runtime_timing_follows_auth_capture() {
        cases(
            "entry::runtime_timing_follows_auth_capture",
            &[("capture", &[("MAESTRO_TIMING", "1")])],
            |local, _| async move {
                let mut e = Entry::new();
                let auth = format!("{}/auth.json", e.commands.agent_dir);
                std::fs::write(
                    &auth,
                    json!({"timing-probe": {
                        "type": "api_key", "key": "controlled"
                    }})
                    .to_string(),
                )
                .unwrap();
                e.clock_hook = clear_auth_at_factory_sample(auth.clone());
                assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                assert_eq!(std::fs::read_to_string(auth).unwrap(), "{}");
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                let captured = runtime
                    .services()
                    .auth_storage
                    .has_auth("timing-probe")
                    .unwrap();
                let observations = e
                    .commands
                    .state
                    .steps
                    .borrow()
                    .iter()
                    .filter(|step| **step == "agent")
                    .count();
                assert_eq!((captured, observations), (true, 1));
            },
        );
    }

    /// Record, at the third clock reading of a run (its parser sample), whether output was
    /// taken over and how many queued terminal observations remain.
    fn observed_at_parse_sample(e: &mut Entry) -> Rc<RefCell<Vec<(bool, usize)>>> {
        let observed = Rc::new(RefCell::new(Vec::new()));
        let (seen, tty) = (Rc::clone(&observed), Rc::clone(&e.tty));
        e.clock_hook = Rc::new(move |sample| {
            if sample.total_cmp(&3.0).is_eq() {
                seen.borrow_mut()
                    .push((is_stdout_taken_over(), tty.borrow().len()));
            }
        });
        observed
    }

    #[test]
    fn parse_timing_precedes_mode_selection_and_early_exits() {
        cases(
            "entry::parse_timing_precedes_mode_selection_and_early_exits",
            &[("parse", &[("MAESTRO_TIMING", "1")])],
            |local, _| async move {
                let mut e = Entry::new();
                e.models(&["first"]);
                let observed = observed_at_parse_sample(&mut e);
                for args in [&["--version"][..], &["--mode", "rpc"]] {
                    e.clock.set(0.0);
                    *e.tty.borrow_mut() = [true, true].into();
                    assert_eq!(
                        e.run(&local, args, MainOptions::default()).await.unwrap(),
                        0
                    );
                }
                assert_eq!(*observed.borrow(), [(false, 2), (false, 2)]);
            },
        );
    }

    #[test]
    fn timing_markers_keep_order_and_mode_positions() {
        let modes: &[(&str, &[(&str, &str)])] = &[
            ("interactive", &[("MAESTRO_TIMING", "1")]),
            (
                "benchmark",
                &[("MAESTRO_TIMING", "1"), ("MAESTRO_STARTUP_BENCHMARK", "1")],
            ),
            ("help", &[("MAESTRO_TIMING", "1")]),
        ];
        cases(
            "entry::timing_markers_keep_order_and_mode_positions",
            modes,
            |local, case| async move {
                let e = Entry::new();
                e.models(&["first"]);
                let args: &[&str] = if case == "help" { &["--help"] } else { &[] };
                assert_eq!(
                    e.run(&local, args, MainOptions::default()).await.unwrap(),
                    0
                );
                let mut expected = vec![
                    "parseArgs",
                    "runMigrations",
                    "createSessionManager",
                    "createRuntime",
                    "readPipedStdin",
                    "prepareInitialMessage",
                    "initTheme",
                    "resolveModelScope",
                    "createAgentSession",
                ];
                match case {
                    "help" => expected.clear(),
                    "benchmark" => expected.push("interactiveMode.init"),
                    _ => {}
                }
                assert_eq!(timing_labels(&e.stderr()), expected, "{case}");
            },
        );
    }

    #[test]
    fn entry_parser_errors_end_before_runtime_creation() {
        run(
            "entry::entry_parser_errors_end_before_runtime_creation",
            |local| async move {
                let e = Entry::new();
                e.styles.set(true);
                let log = Rc::new(RefCell::new(Vec::new()));
                let args = ["--thinking", "max", "-x", "--mode", "json", "--version"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 1);
                let warning = "Invalid thinking level \"max\". Valid values: off, minimal, low, medium, high, xhigh";
                assert_eq!(
                    e.stderr(),
                    format!(
                        "\u{1b}[33mWarning: {warning}\u{1b}[39m\n\u{1b}[31mError: Unknown option: -x\u{1b}[39m\n"
                    )
                );
                assert!(!is_stdout_taken_over());
                assert_eq!(e.stdout(), "");
                e.commands.stderr.clear();
                let args = ["--thinking", "max", "--version"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                assert_eq!(
                    e.stderr(),
                    format!("\u{1b}[33mWarning: {warning}\u{1b}[39m\n")
                );
                assert_eq!(e.stdout(), version());
                assert!(log.borrow().is_empty());
            },
        );
    }

    #[test]
    fn entry_mode_selection_reads_stdin_not_stdout() {
        run(
            "entry::entry_mode_selection_reads_stdin_not_stdout",
            |local| async move {
                let modes = [None, Some("text"), Some("json"), Some("rpc")];
                let selections = modes.into_iter().flat_map(|mode| {
                    [(false, true), (false, false), (true, true), (true, false)]
                        .map(move |(print, tty)| (mode, print, tty))
                });
                for (mode, print, tty) in selections {
                    let e = Entry::new();
                    e.models(&["first"]);
                    *e.tty.borrow_mut() = [tty].into();
                    let mut args = Vec::new();
                    args.extend(mode.into_iter().flat_map(|mode| ["--mode", mode]));
                    args.extend(print.then_some("-p"));
                    maestro_app::output_guard::restore_stdout();
                    e.run(&local, &args, MainOptions::default()).await.unwrap();
                    let launched: Vec<_> = e
                        .log()
                        .into_iter()
                        .filter(|entry| matches!(entry.as_str(), "factory" | "rpc"))
                        .collect();
                    let expected: &[&str] = match mode {
                        Some("rpc") => &["rpc"],
                        Some("json") => &[],
                        _ if print || !tty => &[],
                        _ => &["factory"],
                    };
                    assert_eq!(launched, expected, "{mode:?} {print} {tty}");
                    assert!(expected != ["factory"] || !is_stdout_taken_over());
                }
            },
        );
    }

    #[test]
    fn entry_session_directory_keeps_empty_cli_and_precedence() {
        let sources: &[(&str, &[(&str, &str)])] = &[
            (
                "cli",
                &[("MAESTRO_CODING_AGENT_SESSION_DIR", "~/env-sessions")],
            ),
            (
                "environment",
                &[("MAESTRO_CODING_AGENT_SESSION_DIR", "~/env-sessions")],
            ),
            (
                "empty-environment",
                &[("MAESTRO_CODING_AGENT_SESSION_DIR", "")],
            ),
            ("default", &[]),
        ];
        cases(
            "entry::entry_session_directory_keeps_empty_cli_and_precedence",
            sources,
            |local, case| async move {
                let e = Entry::new();
                let stored = format!("{}/stored-sessions", e.commands.cwd);
                e.settings(Some(json!({"sessionDir": stored})), None);
                let cli = format!("{}/cli-sessions", e.commands.cwd);
                let args: Vec<&str> = if case == "cli" {
                    vec!["--session-dir", &cli]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                let directory = runtime
                    .session()
                    .session_manager()
                    .borrow()
                    .get_session_dir()
                    .to_owned();
                let home = std::env::var("HOME").unwrap();
                let expected = match case {
                    "cli" => cli.clone(),
                    "environment" => format!("{home}/env-sessions"),
                    _ => stored.clone(),
                };
                assert_eq!(directory, expected, "{case}");
                let explicit = ["--session-dir", "", "--no-session"];
                assert_eq!(
                    e.run(&local, &explicit, MainOptions::default())
                        .await
                        .unwrap(),
                    0
                );
            },
        );
    }

    /// Observe the actual selected session without interpreting the argument flags again.
    fn assert_selected_session(
        runtime: &maestro_app::AgentSessionRuntime,
        expected: Option<&str>,
        index: usize,
        originals: (&str, &str),
    ) {
        let session = runtime.session();
        let manager = session.session_manager().borrow();
        match expected {
            Some(path) => assert_eq!(manager.get_session_file(), Some(path), "{index}"),
            None if index == 0 => assert!(!manager.is_persisted(), "{index}"),
            None => {
                let file = manager.get_session_file().unwrap();
                assert!(file != originals.0 && file != originals.1, "{index}");
            }
        }
    }

    #[test]
    fn entry_session_operation_precedence_is_explicit() {
        run(
            "entry::entry_session_operation_precedence_is_explicit",
            |local| async move {
                let e = Entry::new();
                let directory = format!("{}/sessions", e.commands.cwd);
                let older = stored_history(
                    Path::new(&directory),
                    "older",
                    "older-session",
                    &e.commands.cwd,
                );
                let chosen = stored_history(
                    Path::new(&directory),
                    "chosen",
                    "chosen-session",
                    &e.commands.cwd,
                );
                *e.resume.borrow_mut() = Ok(Some(older.clone()));
                let base = ["--session-dir", directory.as_str()];
                let cases: [(&[&str], Option<&str>); 5] = [
                    (
                        &[
                            "--no-session",
                            "--session",
                            "chosen",
                            "--resume",
                            "--continue",
                        ],
                        None,
                    ),
                    (
                        &["--session", "chosen", "--resume", "--continue"],
                        Some(chosen.as_str()),
                    ),
                    (&["--resume", "--continue"], Some(older.as_str())),
                    (&["--continue"], Some(chosen.as_str())),
                    (&[], None),
                ];
                for (index, (flags, expected)) in cases.into_iter().enumerate() {
                    set_modified(&older, 1);
                    set_modified(&chosen, 2);
                    let mut args = base.to_vec();
                    args.extend(flags);
                    assert_eq!(
                        e.run(&local, &args, MainOptions::default()).await.unwrap(),
                        0
                    );
                    let runtime = e.recorder.runtime.borrow().clone().unwrap();
                    assert_selected_session(&runtime, expected, index, (&older, &chosen));
                    let resumed = e
                        .log()
                        .iter()
                        .filter(|entry| *entry == "resume-presented")
                        .count();
                    assert_eq!(resumed, usize::from(index >= 2), "{index}");
                }
            },
        );
    }

    #[test]
    fn runtime_diagnostic_errors_end_before_any_mode() {
        run(
            "entry::runtime_diagnostic_errors_end_before_any_mode",
            |local| async move {
                let e = Entry::new();
                e.models(&["first"]);
                e.styles.set(true);
                let args = ["-e", "./absent-one.ts", "-e", "./absent-two.ts"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    1
                );
                let lines: Vec<_> = e.stderr().lines().map(str::to_owned).collect();
                assert_eq!(lines.len(), 2, "{lines:?}");
                for (line, name) in lines.iter().zip(["absent-one", "absent-two"]) {
                    assert!(
                        line.starts_with("\u{1b}[31mError: Failed to load extension \"")
                            && line.contains(name),
                        "{line}"
                    );
                }
                assert!(!e.log().contains(&"factory".to_owned()));
                e.commands.stderr.clear();
                std::fs::create_dir_all(format!("{}/.maestro", e.commands.cwd)).unwrap();
                std::fs::write(
                    format!("{}/.maestro/settings.json", e.commands.cwd),
                    "{broken",
                )
                .unwrap();
                assert_eq!(e.run(&local, &[], MainOptions::default()).await.unwrap(), 0);
                let warnings = e.stderr();
                assert!(
                    warnings.contains("\u{1b}[33mWarning: (runtime creation, project settings)"),
                    "{warnings}"
                );
                assert!(e.log().contains(&"factory".to_owned()));
                e.commands.stderr.clear();
                e.commands.stdout.clear();
                let args = ["--help", "-e", "./absent-one.ts"];
                assert_eq!(
                    e.run(&local, &args, MainOptions::default()).await.unwrap(),
                    0
                );
                assert!(e.stdout().contains("Usage:"));
                assert!(!e.stderr().contains("Failed to load extension"));
            },
        );
    }

    /// Write and mutate registration from the real session-start callback.
    fn writing_factory(
        out: Rc<RefCell<dyn std::io::Write>>,
        err: Rc<RefCell<dyn std::io::Write>>,
    ) -> ExtensionFactory<Local> {
        Rc::new(move |api| {
            let (out, err) = (Rc::clone(&out), Rc::clone(&err));
            let registration = api.clone();
            let registered = api.on::<maestro_app::SessionStartEvent, ()>(
                "session_start".into(),
                Rc::new(move |event, _| {
                    let reason = format!("start:{:?}\n", event.reason);
                    out.borrow_mut().write_all(reason.as_bytes()).unwrap();
                    err.borrow_mut().write_all(b"diagnostic\n").unwrap();
                    registration
                        .register_flag(
                            "during-start".into(),
                            maestro_app::ExtensionFlagOptions {
                                description: None,
                                r#type: maestro_app::FlagType::Boolean,
                                default: None,
                            },
                        )
                        .unwrap();
                    Box::pin(std::future::ready(Ok(None)))
                }),
            );
            Box::pin(std::future::ready(registered))
        })
    }

    /// Bind the actual extension runner as a frontend does at startup and replacement.
    async fn bind_session(
        session: Rc<maestro_app::AgentSession>,
    ) -> Result<(), maestro_app::DiagnosticErrorInfo> {
        session
            .bind_extensions(maestro_app::agent_session::ExtensionBindings::default())
            .await
    }

    #[test]
    fn runtime_output_releases_each_stream_borrow_before_callbacks() {
        run(
            "entry::runtime_output_releases_each_stream_borrow_before_callbacks",
            |local| async move {
                let e = Entry::new();
                e.models(&["first"]);
                let writer = writing_factory(Rc::clone(&e.out), Rc::clone(&e.err));
                let options = MainOptions {
                    extension_factories: vec![writer],
                };
                let args = ["--models", "absent-pattern-*"];
                assert_eq!(e.run(&local, &args, options).await.unwrap(), 0);
                let runtime = e.recorder.runtime.borrow().clone().unwrap();
                bind_session(runtime.session()).await.unwrap();
                runtime
                    .set_rebind_session(Some(Rc::new(|session| Box::pin(bind_session(session)))));
                let (foreign, _) = foreign_session(&e, "foreign-session");
                runtime
                    .switch_session(&foreign, None, maestro_app::SwitchSessionOptions::default())
                    .await
                    .unwrap();
                let starts: Vec<_> = e.stdout().lines().map(str::to_owned).collect();
                assert_eq!(starts.len(), 2, "{starts:?}");
                assert!(starts[1].contains("Resume"), "{starts:?}");
                let stderr = e.stderr();
                assert!(stderr.contains("absent-pattern-*"), "{stderr}");
                assert_eq!(stderr.matches("diagnostic\n").count(), 2, "{stderr}");
            },
        );
    }

    #[test]
    fn factory_resources_keep_independent_disable_and_prompt_controls() {
        run(
            "entry::factory_resources_keep_independent_disable_and_prompt_controls",
            |local| async move {
                let mut e = Entry::new();
                let modules = module_paths(&mut e);
                e.models(&["first"]);
                project_resources(&e);
                let cases: [(&[&str], [bool; 5]); 6] = [
                    (&[], [true, true, true, true, true]),
                    (&["--no-skills"], [false, true, true, true, true]),
                    (&["--no-prompt-templates"], [true, false, true, true, true]),
                    (&["--no-context-files"], [true, true, false, true, true]),
                    (&["--no-themes"], [true, true, true, false, true]),
                    (&["--no-extensions"], [true, true, true, true, false]),
                ];
                for (flags, [skills, prompts, context, themes, extensions]) in cases {
                    modules.borrow_mut().clear();
                    let log = Rc::new(RefCell::new(Vec::new()));
                    let mut args = vec![
                        "--system-prompt",
                        "System.",
                        "--append-system-prompt",
                        "Also.",
                    ];
                    args.extend(flags);
                    assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                    let runtime = e.recorder.runtime.borrow().clone().unwrap();
                    let loader = Rc::clone(&runtime.services().resource_loader);
                    assert_eq!(!loader.get_skills().skills.is_empty(), skills, "{flags:?}");
                    assert_eq!(
                        !loader.get_prompts().prompts.is_empty(),
                        prompts,
                        "{flags:?}"
                    );
                    assert_eq!(!loader.get_agents_files().is_empty(), context, "{flags:?}");
                    assert_eq!(!loader.get_themes().themes.is_empty(), themes, "{flags:?}");
                    assert_eq!(!modules.borrow().is_empty(), extensions, "{flags:?}");
                    assert_eq!(loader.get_system_prompt().as_deref(), Some("System."));
                    assert_eq!(&*loader.get_append_system_prompt(), ["Also."]);
                    assert_eq!(*log.borrow(), ["extension-load"], "{flags:?}");
                }
                let log = Rc::new(RefCell::new(Vec::new()));
                let args = ["--no-extensions", "--entry-flag"];
                assert_eq!(e.run(&local, &args, options(&log)).await.unwrap(), 0);
                assert_eq!(*log.borrow(), ["extension-load"]);
            },
        );
    }
}
