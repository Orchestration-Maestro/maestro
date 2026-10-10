//! Live theme publication, change callback, custom-file watching and reload.
#[path = "live/fake.rs"]
#[cfg(test)]
mod fake;
#[cfg(test)]
mod live;

use fake::Fake;
use live::{Ops, Scratch, custom_json, state};
use maestro_theme::{
    ColorMode, Theme, ThemeChangeResult, ThemeColor, ThemeOperations, ThemeState,
    ThemeWatchOperations, load_theme_from_path,
};
use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fs;
use std::rc::Rc;

/// A change callback as the state stores it.
type Callback = Rc<dyn Fn() -> Result<(), Box<dyn Error + Send + Sync>>>;

/// Authored name of the published theme.
#[cfg(test)]
fn published(state: &ThemeState) -> String {
    state.theme().get().unwrap().name().unwrap().to_owned()
}

/// The fake as the watch effects a lifecycle call accepts.
#[cfg(test)]
fn effects(fake: &Rc<Fake>) -> Rc<dyn ThemeWatchOperations> {
    Rc::clone(fake) as Rc<dyn ThemeWatchOperations>
}

/// Count callback invocations without failing.
#[cfg(test)]
fn counting(calls: &Rc<Cell<usize>>) -> Callback {
    let calls = Rc::clone(calls);
    Rc::new(move || {
        calls.set(calls.get() + 1);
        Ok(())
    })
}

/// Compact text of [`custom_json`].
#[cfg(test)]
fn custom_theme(name: &str, accent: &str) -> String {
    custom_json(name, accent).to_string()
}

/// Path of an entry in the custom-themes directory.
#[cfg(test)]
fn custom_path(scratch: &Scratch, file: &str) -> String {
    scratch.path(&format!("custom/{file}"))
}

/// The foreground prefix of the accent color.
#[cfg(test)]
fn accent(theme: &Theme) -> String {
    theme.get_fg_ansi(&ThemeColor::Accent).unwrap().to_owned()
}

/// Write `name.json` into the custom directory with an accent.
#[cfg(test)]
fn put(scratch: &Scratch, file: &str, name: &str, accent: &str) {
    scratch.write(&format!("custom/{file}"), &custom_theme(name, accent));
}

/// A named instance loaded from a file outside the custom directory.
#[cfg(test)]
fn instance(scratch: &Scratch, ops: &Rc<Ops>, name: &str) -> Rc<Theme> {
    let path = scratch.path(&format!("elsewhere/{name}.json"));
    scratch.write(
        &format!("elsewhere/{name}.json"),
        &custom_theme(name, "#010203"),
    );
    let ops: Rc<dyn ThemeOperations> = Rc::clone(ops) as Rc<dyn ThemeOperations>;
    Rc::new(load_theme_from_path(&path, Some(ColorMode::Truecolor), ops.as_ref()).unwrap())
}

#[test]
fn theme_live_view_follows_publication() {
    let scratch = Scratch::new("live-view");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let live = state.theme();
    let error = live.get().err().unwrap();
    assert_eq!(
        error.to_string(),
        "Theme not initialized. Call initTheme() first."
    );

    state.init_theme(Some("dark"), None).unwrap();
    let first = live.get().unwrap();
    assert_eq!(first.name(), Some("dark"));
    let other_view = state.theme();
    let cloned = live.clone();
    state.set_theme("light", None).unwrap();
    assert_eq!(live.get().unwrap().name(), Some("light"));
    assert_eq!(other_view.get().unwrap().name(), Some("light"));
    assert!(Rc::ptr_eq(&cloned.get().unwrap(), &live.get().unwrap()));
    assert_eq!(first.name(), Some("dark"));

    let direct = instance(&scratch, &ops, "direct");
    state.set_theme_instance(Rc::clone(&direct)).unwrap();
    assert!(Rc::ptr_eq(&live.get().unwrap(), &direct));
    let independent = state.get_theme_by_name("dark").unwrap();
    state.set_theme("dark", None).unwrap();
    assert_eq!(independent.name(), Some("dark"));
    assert!(!Rc::ptr_eq(&live.get().unwrap(), &direct));
}

#[test]
fn theme_default_uses_the_second_background_field() {
    let scratch = Scratch::new("default");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let cases = [
        ("", "dark"),
        ("7", "dark"),
        ("x;y", "dark"),
        ("0;7", "dark"),
        ("0;8", "light"),
        ("0;7;8", "dark"),
        ("0;8;7", "light"),
        ("0;+8", "light"),
        ("0;-8", "dark"),
        ("0;0008", "light"),
        ("0;0007", "dark"),
        ("0;8.5", "light"),
        ("0;7.9", "dark"),
        ("0;1e9", "dark"),
        ("0;0x10", "dark"),
        ("0;10", "light"),
        ("0; 8", "light"),
        ("0;\t\n8", "light"),
        ("0;\u{feff}8", "light"),
        ("0;\u{2028}8", "light"),
        ("0;\u{85}8", "dark"),
        ("0;\u{200b}8", "dark"),
        ("0;99999999999999999999", "light"),
    ];
    state.init_theme(None, None).unwrap();
    assert_eq!(published(&state), "dark");
    for (index, (value, expected)) in cases.into_iter().enumerate() {
        ops.set("COLORFGBG", value);
        state.init_theme(None, None).unwrap();
        assert_eq!(published(&state), expected, "case {index}");
    }
}

#[test]
fn theme_initialization_falls_back_without_notifying() {
    let scratch = Scratch::new("init-fallback");
    let ops = Ops::new(&[("COLORFGBG", "0;8")]);
    let state = state(&scratch, &ops);
    let calls = Rc::new(Cell::new(0));
    state.on_theme_change(counting(&calls));

    state.init_theme(None, None).unwrap();
    assert_eq!(published(&state), "light");
    assert!(ops.read("COLORFGBG"));
    ops.env_reads.borrow_mut().clear();

    for name in ["", "missing"] {
        state.init_theme(Some(name), None).unwrap();
        assert_eq!(published(&state), "dark", "name {name:?}");
    }
    assert!(!ops.read("COLORFGBG"));
    state.init_theme(Some("light"), None).unwrap();
    assert_eq!(calls.get(), 0);

    let broken = ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: scratch.path("absent"),
            custom_themes_dir: scratch.path("custom"),
        },
        Rc::clone(&ops) as Rc<dyn ThemeOperations>,
    );
    assert!(Rc::new(broken).init_theme(Some("missing"), None).is_err());
}

#[test]
fn theme_named_switch_reports_failure_after_fallback() {
    let scratch = Scratch::new("named-switch");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    put(&scratch, "custom.json", "custom", "#112233");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let fail = Rc::new(Cell::new(false));
    let live = state.theme();
    let (log, failing) = (Rc::clone(&seen), Rc::clone(&fail));
    state.on_theme_change(Rc::new(move || {
        log.borrow_mut()
            .push(live.get().unwrap().name().unwrap().to_owned());
        if failing.get() {
            return Err("callback failed".into());
        }
        Ok(())
    }));

    assert_eq!(
        state.set_theme("custom", None).unwrap(),
        ThemeChangeResult::Success
    );
    fail.set(true);
    let failure = state.set_theme("light", None).unwrap();
    assert_eq!(
        failure,
        ThemeChangeResult::Failure {
            error: "callback failed".into()
        }
    );
    assert_eq!(*seen.borrow(), ["custom", "light"]);
    assert_eq!(published(&state), "dark");

    fail.set(false);
    let missing = state.set_theme("missing", None).unwrap();
    assert_eq!(
        missing,
        ThemeChangeResult::Failure {
            error: "Theme not found: missing".into()
        }
    );
    assert_eq!(published(&state), "dark");
    assert_eq!(seen.borrow().len(), 2);

    scratch.write("custom/invalid.json", "{");
    let selection = match state.set_theme("invalid", None).unwrap() {
        ThemeChangeResult::Failure { error } => error,
        ThemeChangeResult::Success => panic!("an unparsable file must not load"),
    };
    let broken = broken_shipped_state(&scratch, &ops);
    let error = broken.set_theme("invalid", None).unwrap_err().to_string();
    assert_ne!(error, selection);
    assert_eq!(error, "No such file or directory (os error 2)");
    assert!(broken.theme().get().is_err());
}

/// A state whose shipped theme directory does not exist, so no fallback can load.
#[cfg(test)]
fn broken_shipped_state(scratch: &Scratch, ops: &Rc<Ops>) -> Rc<ThemeState> {
    Rc::new(ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: scratch.path("absent"),
            custom_themes_dir: scratch.path("custom"),
        },
        Rc::clone(ops) as Rc<dyn ThemeOperations>,
    ))
}

#[test]
fn theme_failed_initialization_reports_the_fallback_error_not_the_selection_error() {
    let scratch = Scratch::new("init-errors");
    let ops = Ops::new(&[]);
    scratch.write("custom/invalid.json", "{");
    let healthy = state(&scratch, &ops);
    healthy.init_theme(Some("invalid"), None).unwrap();
    assert_eq!(published(&healthy), "dark");

    let broken = broken_shipped_state(&scratch, &ops);
    let error = broken.init_theme(Some("invalid"), None).unwrap_err();
    assert_eq!(error.to_string(), "No such file or directory (os error 2)");
    assert!(broken.theme().get().is_err());
}

/// Effects that run `during_load` once, inside the first read of the custom file.
#[cfg(test)]
struct ProbeOps {
    /// Real effects.
    inner: Rc<Ops>,
    /// File whose first read triggers the probe.
    target: String,
    /// Probe to run, taken on first use.
    during_load: RefCell<Option<Box<dyn FnOnce()>>>,
}

#[cfg(test)]
impl ThemeOperations for ProbeOps {
    fn read_to_string(&self, path: &str) -> std::io::Result<String> {
        let probe = if path == self.target {
            self.during_load.borrow_mut().take()
        } else {
            None
        };
        if let Some(probe) = probe {
            probe();
        }
        self.inner.read_to_string(path)
    }
    fn environment(&self, name: &str) -> Option<String> {
        self.inner.environment(name)
    }
    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }
    fn read_dir(&self, path: &str) -> std::io::Result<Vec<String>> {
        self.inner.read_dir(path)
    }
    fn sort_by_name(&self, themes: &mut [maestro_theme::ThemeInfo]) -> std::io::Result<()> {
        self.inner.sort_by_name(themes)
    }
}

#[test]
fn theme_selected_name_is_recorded_before_loading() {
    let scratch = Scratch::new("select-first");
    let ops = Ops::new(&[]);
    put(&scratch, "a.json", "a", "#112233");
    let seen = Rc::new(RefCell::new(None));
    let slot: Rc<RefCell<Option<Rc<ThemeState>>>> = Rc::default();
    let (probe_state, probe_seen) = (Rc::clone(&slot), Rc::clone(&seen));
    let probe = Rc::new(ProbeOps {
        inner: Rc::clone(&ops),
        target: custom_path(&scratch, "a.json"),
        during_load: RefCell::new(Some(Box::new(move || {
            let state = probe_state.borrow().clone().unwrap();
            *probe_seen.borrow_mut() = Some(state.get_resolved_theme_colors(None));
        }))),
    });
    let state = Rc::new(ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme").to_owned(),
            custom_themes_dir: scratch.path("custom"),
        },
        probe as Rc<dyn ThemeOperations>,
    ));
    *slot.borrow_mut() = Some(Rc::clone(&state));
    state.init_theme(Some("dark"), None).unwrap();

    state.init_theme(Some("a"), None).unwrap();
    let resolved = seen.borrow_mut().take().unwrap().unwrap();
    let accent = resolved.iter().find(|(key, _)| key == "accent").unwrap();
    assert_eq!(accent.1, "#112233");
    *slot.borrow_mut() = None;
}

#[test]
fn theme_construction_reports_integer_keys_before_authored_order() {
    let scratch = Scratch::new("key-order");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let mut json = custom_json("ordered", "missingAccent");
    json["colors"]["0"] = "missingIndex".into();
    scratch.write("custom/ordered.json", &json.to_string());

    assert_eq!(
        state.set_theme("ordered", None).unwrap(),
        ThemeChangeResult::Failure {
            error: "Variable reference not found: missingIndex".into()
        }
    );
}

#[test]
fn theme_direct_instance_publishes_before_callback() {
    let scratch = Scratch::new("direct");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    fake.emit(0, Some("a.json"));
    assert_eq!(fake.pending(), 1);

    let direct = instance(&scratch, &ops, "memory");
    let inside = Rc::new(RefCell::new(None));
    let (live, weak) = (state.theme(), Rc::downgrade(&state));
    let (fake_view, expected, slot) = (Rc::clone(&fake), Rc::clone(&direct), Rc::clone(&inside));
    state.on_theme_change(Rc::new(move || {
        let state = weak.upgrade().unwrap();
        *slot.borrow_mut() = Some((
            Rc::ptr_eq(&live.get().unwrap(), &expected),
            fake_view.watches.borrow()[0].closed.get(),
            fake_view.pending(),
            state
                .get_resolved_theme_colors(None)
                .err()
                .unwrap()
                .to_string(),
        ));
        Err("direct failed".into())
    }));

    let error = state.set_theme_instance(Rc::clone(&direct)).unwrap_err();
    assert_eq!(error.to_string(), "direct failed");
    let seen = inside.borrow().clone().unwrap();
    assert_eq!(
        seen,
        (true, true, 0, "Theme not found: <in-memory>".to_owned())
    );
    assert!(Rc::ptr_eq(&state.theme().get().unwrap(), &direct));
}

#[test]
fn theme_callback_registration_replaces_the_old_callback() {
    let scratch = Scratch::new("replace-callback");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let log = Rc::new(RefCell::new(Vec::new()));
    let token = Rc::new(());
    let released = Rc::downgrade(&token);
    let (first_log, held) = (Rc::clone(&log), token);
    let first: Rc<dyn Fn() -> Result<(), Box<dyn std::error::Error + Send + Sync>>> =
        Rc::new(move || {
            let _ = Rc::strong_count(&held);
            first_log.borrow_mut().push("first");
            Ok(())
        });
    state.on_theme_change(Rc::clone(&first));
    state.set_theme("light", None).unwrap();
    assert!(released.upgrade().is_some());
    assert_eq!(Rc::strong_count(&first), 2);

    let (second_log, weak) = (Rc::clone(&log), Rc::downgrade(&state));
    state.on_theme_change(Rc::new(move || {
        second_log.borrow_mut().push("second");
        let third_log = Rc::clone(&second_log);
        weak.upgrade().unwrap().on_theme_change(Rc::new(move || {
            third_log.borrow_mut().push("third");
            Ok(())
        }));
        Ok(())
    }));
    assert_eq!(Rc::strong_count(&first), 1);
    assert!(released.upgrade().is_some());
    first().unwrap();
    assert_eq!(log.borrow().last(), Some(&"first"));
    log.borrow_mut().pop();
    drop(first);
    assert!(released.upgrade().is_none());
    state.set_theme("dark", None).unwrap();
    state.set_theme("light", None).unwrap();
    assert_eq!(*log.borrow(), ["first", "second", "third"]);
}

#[test]
fn theme_callback_allows_reentrant_publication() {
    let scratch = Scratch::new("reentrant");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, weak, live) = (Rc::clone(&seen), Rc::downgrade(&state), state.theme());
    state.on_theme_change(Rc::new(move || {
        let current = live.get().unwrap().name().unwrap().to_owned();
        log.borrow_mut().push(current.clone());
        if current == "light" {
            let nested = weak.upgrade().unwrap().set_theme("dark", None).unwrap();
            assert_eq!(nested, ThemeChangeResult::Success);
        }
        Ok(())
    }));
    assert_eq!(
        state.set_theme("light", None).unwrap(),
        ThemeChangeResult::Success
    );
    assert_eq!(*seen.borrow(), ["light", "dark"]);
    assert_eq!(published(&state), "dark");
}

#[test]
fn theme_watch_disabled_switch_retains_existing_watch_state() {
    let scratch = Scratch::new("disabled");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    put(&scratch, "b.json", "b", "#223344");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    fake.emit(0, Some("a.json"));

    state.init_theme(Some("b"), None).unwrap();
    assert!(!fake.watches.borrow()[0].closed.get());
    assert_eq!(fake.pending(), 1);
    fake.advance(100);
    assert_eq!(published(&state), "b");
    assert_eq!(fake.pending(), 0);

    put(&scratch, "a.json", "a-edited", "#334455");
    state.set_theme("a", None).unwrap();
    fake.emit(0, Some("a.json"));
    fake.advance(100);
    assert_eq!(published(&state), "a-edited");

    keeps_old_watch_on_load_failure_and_new_watch_on_callback_failure(&state, &fake);
}

/// A failed load keeps the old watch; a failed callback keeps the new one.
#[cfg(test)]
fn keeps_old_watch_on_load_failure_and_new_watch_on_callback_failure(
    state: &Rc<ThemeState>,
    fake: &Rc<Fake>,
) {
    let missing = state.set_theme("missing", Some(effects(fake))).unwrap();
    assert!(matches!(missing, ThemeChangeResult::Failure { .. }));
    assert_eq!(fake.watches.borrow().len(), 1);
    assert!(!fake.watches.borrow()[0].closed.get());

    state.on_theme_change(Rc::new(|| Err("callback failed".into())));
    let failure = state.set_theme("b", Some(effects(fake))).unwrap();
    assert!(matches!(failure, ThemeChangeResult::Failure { .. }));
    assert_eq!(published(state), "dark");
    assert_eq!(fake.watches.borrow().len(), 2);
    assert!(fake.watches.borrow()[0].closed.get());
    assert!(!fake.watches.borrow()[1].closed.get());
}

#[test]
fn theme_watch_start_replaces_old_handles_and_timers() {
    let scratch = Scratch::new("start-replaces");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    put(&scratch, "b.json", "b", "#223344");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    fake.emit(0, None);
    assert_eq!(fake.pending(), 1);

    state.init_theme(Some("b"), Some(effects(&fake))).unwrap();
    assert!(fake.watches.borrow()[0].closed.get());
    assert_eq!(fake.pending(), 0);
    assert_eq!(fake.watches.borrow().len(), 2);
    assert_eq!(fake.watches.borrow()[1].path, scratch.path("custom"));

    put(&scratch, ".json", "", "#010203");
    for name in ["dark", "light", "registered", ""] {
        let fresh = Rc::new(ThemeState::new(
            maestro_theme::ThemeDirectories {
                themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme").to_owned(),
                custom_themes_dir: scratch.path("custom"),
            },
            Rc::clone(&ops) as Rc<dyn ThemeOperations>,
        ));
        let fake = Fake::new();
        fresh.set_registered_themes(vec![instance(&scratch, &ops, "registered")]);
        fresh.init_theme(Some("a"), Some(effects(&fake))).unwrap();
        fresh.set_theme(name, Some(effects(&fake))).unwrap();
        assert!(fake.watches.borrow()[0].closed.get(), "name {name:?}");
        assert_eq!(fake.watches.borrow().len(), 1, "name {name:?}");
    }
    state
        .set_theme_instance(instance(&scratch, &ops, "direct"))
        .unwrap();
    state.stop_theme_watcher();
    state.stop_theme_watcher();
    assert_eq!(fake.pending(), 0);
}

#[test]
fn theme_registered_watch_uses_the_custom_directory() {
    let scratch = Scratch::new("registered-dir");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "registered.json", "on-disk", "#112233");
    let registered = instance(&scratch, &ops, "registered");
    state.set_registered_themes(vec![Rc::clone(&registered)]);

    state
        .init_theme(Some("registered"), Some(effects(&fake)))
        .unwrap();
    assert!(Rc::ptr_eq(&state.theme().get().unwrap(), &registered));
    assert_eq!(fake.watches.borrow().len(), 1);
    assert_eq!(fake.watches.borrow()[0].path, scratch.path("custom"));
}

#[test]
fn theme_notifications_filter_names_before_debouncing() {
    let scratch = Scratch::new("filter");
    let ops = Ops::new(&[]);
    put(&scratch, "a.json", "a", "#112233");
    put(&scratch, "b.json", "b", "#223344");
    let pending_after = |filename: Option<&str>, switch_away: bool| {
        let state = state(&scratch, &ops);
        let fake = Fake::new();
        state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
        if switch_away {
            state.set_theme("b", None).unwrap();
        }
        fake.emit(0, filename);
        fake.pending()
    };
    for filename in [Some("a.json"), None, Some("")] {
        assert_eq!(pending_after(filename, false), 1, "filename {filename:?}");
    }
    for filename in [
        Some("b.json"),
        Some("A.json"),
        Some("sub/a.json"),
        Some("a.json.tmp"),
    ] {
        assert_eq!(pending_after(filename, false), 0, "filename {filename:?}");
    }
    assert_eq!(pending_after(Some("a.json"), true), 0);
}

#[test]
fn theme_reload_waits_for_the_full_debounce() {
    let scratch = Scratch::new("debounce");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    let calls = Rc::new(Cell::new(0));
    state.on_theme_change(counting(&calls));
    put(&scratch, "a.json", "edited", "#445566");

    fake.emit(0, Some("a.json"));
    fake.advance(99);
    assert_eq!((calls.get(), published(&state).as_str()), (0, "a"));
    fake.emit(0, Some("a.json"));
    fake.advance(99);
    assert_eq!((calls.get(), published(&state).as_str()), (0, "a"));
    fake.advance(1);
    assert_eq!((calls.get(), published(&state).as_str()), (1, "edited"));

    fake.emit(0, Some("a.json"));
    state.stop_theme_watcher();
    assert_eq!(fake.pending(), 0);
    fake.advance(200);
    assert_eq!(calls.get(), 1);

    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    fake.emit(1, Some("a.json"));
    state.set_theme("dark", Some(effects(&fake))).unwrap();
    assert_eq!(fake.pending(), 0);
}

#[test]
fn maestro_theme_keeps_last_good_file_on_reload_error() {
    let scratch = Scratch::new("last-good");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    let file = custom_path(&scratch, "a.json");
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    let calls = Rc::new(Cell::new(0));
    state.on_theme_change(counting(&calls));
    let reload = |content: Option<&str>| {
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&file);
        if let Some(content) = content {
            fs::write(&file, content).unwrap();
        }
        fake.emit(0, Some("a.json"));
        fake.advance(100);
    };

    reload(Some(&custom_theme("good", "#778899")));
    let good = state.theme().get().unwrap();
    assert_eq!((good.name(), calls.get()), (Some("good"), 1));
    assert!(Rc::ptr_eq(&state.get_theme_by_name("a").unwrap(), &good));

    let mut missing_color = custom_json("broken", "#010203");
    missing_color["colors"]
        .as_object_mut()
        .unwrap()
        .remove("text");
    let mut bad_alias = custom_json("broken", "#010203");
    bad_alias["colors"]["accent"] = "undefinedVariable".into();
    let broken = [
        missing_color.to_string(),
        bad_alias.to_string(),
        "{".to_owned(),
    ];
    for content in broken.iter().map(String::as_str).map(Some).chain([None]) {
        reload(content);
        assert!(Rc::ptr_eq(&state.theme().get().unwrap(), &good));
        assert!(Rc::ptr_eq(&state.get_theme_by_name("a").unwrap(), &good));
        assert_eq!(calls.get(), 1);
    }
    let _ = fs::remove_file(&file);
    fs::create_dir(&file).unwrap();
    fake.emit(0, Some("a.json"));
    fake.advance(100);
    assert!(Rc::ptr_eq(&state.theme().get().unwrap(), &good));
    assert_eq!(calls.get(), 1);

    reload(Some(&custom_theme("recovered", "#abcdef")));
    assert_eq!((published(&state).as_str(), calls.get()), ("recovered", 2));
    assert_eq!(good.name(), Some("good"));
}

#[test]
fn theme_reload_refreshes_registration_before_notifying() {
    let scratch = Scratch::new("registration");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "authored-other", "#445566");
    let old = instance(&scratch, &ops, "a");
    state.set_registered_themes(vec![Rc::clone(&old)]);
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    assert!(Rc::ptr_eq(&state.theme().get().unwrap(), &old));

    let seen = Rc::new(RefCell::new(Vec::new()));
    let (log, live, weak) = (Rc::clone(&seen), state.theme(), Rc::downgrade(&state));
    state.on_theme_change(Rc::new(move || {
        let theme = live.get().unwrap();
        let registered = weak.upgrade().unwrap().get_theme_by_name("a").unwrap();
        log.borrow_mut().push((
            theme.name().map(str::to_owned),
            theme.source_path().map(str::to_owned),
            Rc::ptr_eq(&theme, &registered),
        ));
        Ok(())
    }));
    fake.emit(0, Some("a.json"));
    fake.advance(100);

    let expected = (
        Some("authored-other".to_owned()),
        Some(custom_path(&scratch, "a.json")),
        true,
    );
    assert_eq!(*seen.borrow(), [expected]);
    assert!(!Rc::ptr_eq(&state.theme().get().unwrap(), &old));
    assert_eq!(
        accent(&state.theme().get().unwrap()),
        "\x1b[38;2;68;85;102m"
    );
}

#[test]
fn theme_reload_callback_failure_keeps_published_theme() {
    let scratch = Scratch::new("callback-failure");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    state.on_theme_change(Rc::new(move || {
        counter.set(counter.get() + 1);
        Err("reload callback failed".into())
    }));
    put(&scratch, "a.json", "after-error", "#abcdef");
    fake.emit(0, Some("a.json"));
    fake.advance(100);

    let theme = state.theme().get().unwrap();
    assert_eq!((theme.name(), calls.get()), (Some("after-error"), 1));
    assert!(Rc::ptr_eq(&state.get_theme_by_name("a").unwrap(), &theme));
}

#[test]
fn theme_watcher_error_closes_without_retrying() {
    let scratch = Scratch::new("watch-error");
    let ops = Ops::new(&[]);
    put(&scratch, "a.json", "a", "#112233");

    let created = Fake::new();
    created.fail_create.set(true);
    let state_a = state(&scratch, &ops);
    state_a
        .init_theme(Some("a"), Some(effects(&created)))
        .unwrap();
    assert_eq!((created.watches.borrow().len(), created.pending()), (0, 0));
    assert_eq!(published(&state_a), "a");

    for close_fails in [false, true] {
        put(&scratch, "a.json", "a", "#112233");
        let fake = Fake::new();
        fake.fail_close.set(close_fails);
        let state = state(&scratch, &ops);
        state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
        put(&scratch, "a.json", "timer-survives", "#abcdef");
        fake.emit(0, Some("a.json"));
        fake.fail(0);
        assert!(fake.watches.borrow()[0].closed.get());
        assert_eq!(fake.pending(), 1);
        fake.advance(100);
        assert_eq!(published(&state), "timer-survives");
        assert_eq!(fake.watches.borrow().len(), 1);
    }
}

#[test]
fn theme_watch_stop_releases_retained_callbacks() {
    let scratch = Scratch::new("stop-releases");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    fake.emit(0, Some("a.json"));
    let live = state.theme();
    let held = live.get().unwrap();
    assert!(Rc::strong_count(&fake) > 1);
    assert!(Rc::weak_count(&state) > 0);

    state.stop_theme_watcher();
    assert!(fake.watches.borrow()[0].closed.get());
    assert_eq!((fake.pending(), fake.cancelled.get()), (0, 1));
    assert_eq!(Rc::strong_count(&fake), 1);
    assert_eq!(Rc::weak_count(&fake), 0);
    assert_eq!(Rc::weak_count(&state), 0);
    assert_eq!(live.get().unwrap().name(), Some("a"));
    assert_eq!(held.name(), Some("a"));
}

#[test]
fn theme_drop_cancels_pending_reload() {
    let scratch = Scratch::new("drop");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let fake = Fake::new();
    put(&scratch, "a.json", "a", "#112233");
    state.init_theme(Some("a"), Some(effects(&fake))).unwrap();
    let calls = Rc::new(Cell::new(0));
    state.on_theme_change(counting(&calls));
    put(&scratch, "a.json", "late", "#abcdef");
    fake.emit(0, Some("a.json"));
    let weak = Rc::downgrade(&state);
    let live = state.theme();

    drop(state);
    assert!(weak.upgrade().is_none());
    assert!(fake.watches.borrow()[0].closed.get());
    assert_eq!(fake.cancelled.get(), 1);
    fake.advance(200);
    assert_eq!(calls.get(), 0);
    assert_eq!(live.get().unwrap().name(), Some("a"));
}

#[test]
fn theme_native_watch_stop_releases_operations_and_task_set_without_reads() {
    let scratch = Scratch::new("native-stop");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    put(&scratch, "a.json", "a", "#112233");
    let local = Rc::new(tokio::task::LocalSet::new());
    let native: Rc<dyn ThemeWatchOperations> = Rc::new(
        maestro_theme::NativeThemeWatchOperations::new(Rc::clone(&local)),
    );
    state.init_theme(Some("a"), Some(native)).unwrap();
    assert_eq!(Rc::strong_count(&local), 2);
    assert!(Rc::weak_count(&state) > 0);

    state.stop_theme_watcher();
    assert_eq!(Rc::strong_count(&local), 1);
    let local = Rc::into_inner(local).expect("the state released its operations");
    drop(local);
    assert_eq!(Rc::weak_count(&state), 0);
    assert_eq!(published(&state), "a");
}

#[test]
fn theme_watch_imports_are_shared_types() {
    use maestro_watch::fs_watch::{FsWatcher, WatchOperations, WatchTimer};
    fn watches(value: Box<dyn FsWatcher>) -> Box<dyn FsWatcher> {
        let root: Box<dyn maestro_theme::ThemeWatcher> = value;
        let module: Box<dyn maestro_theme::theme::ThemeWatcher> = root;
        let facade: Box<dyn maestro_theme::theme::fs_watch::ThemeWatcher> = module;
        facade
    }
    fn timers(value: Box<dyn WatchTimer>) -> Box<dyn WatchTimer> {
        let root: Box<dyn maestro_theme::ThemeReloadTimer> = value;
        let module: Box<dyn maestro_theme::theme::ThemeReloadTimer> = root;
        let facade: Box<dyn maestro_theme::theme::fs_watch::ThemeReloadTimer> = module;
        facade
    }
    fn operations(value: Rc<dyn WatchOperations>) -> Rc<dyn WatchOperations> {
        let root: Rc<dyn maestro_theme::ThemeWatchOperations> = value;
        let module: Rc<dyn maestro_theme::theme::ThemeWatchOperations> = root;
        let facade: Rc<dyn maestro_theme::theme::fs_watch::ThemeWatchOperations> = module;
        facade
    }
    let fake = Rc::new(Fake::default());
    let shared = operations(Rc::clone(&fake) as Rc<dyn WatchOperations>);
    let watcher = shared
        .watch("directory", Rc::new(|_| {}), Rc::new(|| {}))
        .unwrap();
    watches(watcher).close().unwrap();
    assert!(fake.watches.borrow()[0].closed.get());
    let calls = Rc::new(Cell::new(0));
    let counted = Rc::clone(&calls);
    let timer = shared.schedule(
        std::time::Duration::from_millis(100),
        Box::new(move || counted.set(counted.get() + 1)),
    );
    timers(timer).cancel();
    fake.advance(100);
    assert_eq!(calls.get(), 0);
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::any::TypeId;
        let shared = TypeId::of::<maestro_watch::fs_watch::NativeWatchOperations>();
        assert_eq!(
            shared,
            TypeId::of::<maestro_theme::NativeThemeWatchOperations>()
        );
        assert_eq!(
            shared,
            TypeId::of::<maestro_theme::theme::NativeThemeWatchOperations>()
        );
        assert_eq!(
            shared,
            TypeId::of::<maestro_theme::theme::fs_watch::NativeThemeWatchOperations>()
        );
    }
}
