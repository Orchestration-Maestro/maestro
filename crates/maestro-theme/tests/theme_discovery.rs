//! Theme registration, discovery and shared source metadata.
use crate::live::{Scratch, TRUECOLOR, in_child};
use maestro_request::source_info::{SourceInfo, SourceOrigin, SourceScope};
use maestro_theme::{
    ColorMode, Theme, ThemeColor, ThemeDirectories, ThemeError, ThemeInfo, ThemeOptions,
    ThemeState, load_theme_from_path,
};
use std::cell::RefCell;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::rc::Rc;

#[cfg(test)]
const SHIPPED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme");
#[cfg(test)]
const TRUECOLOR_ACCENT: &str = "\x1b[38;2;217;160;102m";

#[cfg(test)]
fn dirs(themes: &str, custom: &str) -> ThemeDirectories {
    ThemeDirectories {
        themes_dir: themes.to_owned(),
        custom_themes_dir: custom.to_owned(),
    }
}

/// A state over the shipped themes and `custom`.
#[cfg(test)]
fn make(custom: &str) -> ThemeState {
    ThemeState::new(dirs(SHIPPED, custom))
}

#[cfg(test)]
fn shipped_dark() -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(format!("{SHIPPED}/dark.json")).unwrap()).unwrap()
}

/// The shipped dark document renamed, with its accent variable replaced.
fn theme_text(name: &str, accent: &str) -> String {
    let mut doc = shipped_dark();
    doc["name"] = name.into();
    doc["vars"]["accent"] = accent.into();
    doc.to_string()
}

#[cfg(test)]
fn registered(name: Option<&str>, path: Option<&str>) -> Rc<Theme> {
    let options = ThemeOptions {
        name: name.map(str::to_owned),
        source_path: path.map(str::to_owned),
        ..ThemeOptions::default()
    };
    Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap())
}

#[cfg(test)]
fn accent(theme: &Theme) -> String {
    theme.get_fg_ansi(&ThemeColor::Accent).unwrap().to_owned()
}

#[cfg(test)]
fn names(infos: &[ThemeInfo]) -> Vec<(&str, Option<&str>)> {
    infos
        .iter()
        .map(|info| (info.name.as_str(), info.path.as_deref()))
        .collect()
}

#[cfg(test)]
fn io_kind(error: &ThemeError) -> Option<io::ErrorKind> {
    error
        .source()
        .and_then(|cause| cause.downcast_ref::<io::Error>())
        .map(io::Error::kind)
}

#[cfg(test)]
fn record(
    scope: SourceScope,
    origin: SourceOrigin,
    base_dir: Option<&str>,
) -> Rc<RefCell<SourceInfo>> {
    Rc::new(RefCell::new(SourceInfo {
        path: "original".into(),
        source: "label".into(),
        scope,
        origin,
        base_dir: base_dir.map(str::to_owned),
    }))
}

#[test]
fn theme_source_metadata_keeps_alias_and_slot_identity() {
    for scope in [
        SourceScope::User,
        SourceScope::Project,
        SourceScope::Temporary,
    ] {
        for origin in [SourceOrigin::Package, SourceOrigin::TopLevel] {
            for base in [None, Some("/base")] {
                let first = record(scope, origin, base);
                let options = ThemeOptions {
                    source_info: Some(Rc::clone(&first)),
                    ..ThemeOptions::default()
                };
                let theme = Theme::new([], [], ColorMode::Truecolor, options).unwrap();
                assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &first));
                first.borrow_mut().path = "edited".into();
                assert_eq!(theme.source_info().unwrap().borrow().path, "edited");
                assert_eq!(
                    theme.source_info().unwrap().borrow().base_dir.as_deref(),
                    base
                );

                let second = record(scope, origin, base);
                let weak_first = Rc::downgrade(&first);
                theme.set_source_info(Some(Rc::clone(&second)));
                assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &second));
                assert_eq!(first.borrow().path, "edited");
                drop(first);
                assert!(weak_first.upgrade().is_none());

                theme.set_source_info(None);
                assert!(theme.source_info().is_none());
                assert_eq!(second.borrow().path, "original");
            }
        }
    }
    let bare = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
    assert!(bare.source_info().is_none());
}

#[test]
fn theme_registration_replaces_without_reordering() {
    let scratch = Scratch::new();
    let state = make(&scratch.path("missing"));
    let decomposed = registered(Some("e\u{301}"), Some("second"));
    let replacement = registered(Some("é"), Some("replacement"));
    state.set_registered_themes(vec![
        registered(Some("é"), Some("first")),
        Rc::clone(&decomposed),
        registered(None, Some("nameless")),
        registered(Some(""), Some("empty")),
        Rc::clone(&replacement),
    ]);
    let listed = state.get_available_themes_with_paths().unwrap();
    let dark = maestro_path::join(&[SHIPPED, "dark.json"]);
    let light = maestro_path::join(&[SHIPPED, "light.json"]);
    assert_eq!(
        names(&listed),
        [
            ("dark", Some(dark.as_str())),
            ("é", Some("replacement")),
            ("e\u{301}", Some("second")),
            ("light", Some(light.as_str())),
        ]
    );
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("é").unwrap(),
        &replacement
    ));
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("e\u{301}").unwrap(),
        &decomposed
    ));

    state.set_registered_themes(vec![registered(Some("other"), None)]);
    assert!(state.get_theme_by_name("é").is_none());
    assert_eq!(
        state.get_available_themes().unwrap(),
        ["dark", "light", "other"]
    );
}

#[test]
fn theme_registration_clear_preserves_retained_instances() {
    let state = make("/nonexistent-custom");
    let info = record(SourceScope::User, SourceOrigin::TopLevel, None);
    let options = ThemeOptions {
        name: Some("mine".into()),
        source_info: Some(Rc::clone(&info)),
        ..ThemeOptions::default()
    };
    let theme = Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap());
    let weak = Rc::downgrade(&theme);
    state.set_registered_themes(vec![Rc::clone(&theme)]);
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("mine").unwrap(),
        &theme
    ));

    state.set_registered_themes(vec![]);
    assert!(state.get_theme_by_name("mine").is_none());
    assert_eq!(theme.name(), Some("mine"));
    assert!(Rc::ptr_eq(&theme.source_info().unwrap(), &info));
    drop(theme);
    assert!(weak.upgrade().is_none());
}

#[test]
fn theme_registered_lookup_avoids_unneeded_effects() {
    // Neither directory exists, so any file read would fail the lookup.
    let scratch = Scratch::new();
    let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
    let shadow = registered(Some("dark"), None);
    state.set_registered_themes(vec![Rc::clone(&shadow)]);
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("dark").unwrap(),
        &shadow
    ));
}

#[test]
fn maestro_theme_resolves_registered_and_file_precedence() {
    if !in_child(
        "maestro_theme_resolves_registered_and_file_precedence",
        TRUECOLOR,
    ) {
        return;
    }
    let scratch = Scratch::new();
    scratch.write("custom/dark.json", &theme_text("custom-dark", "#123456"));
    let state = make(&scratch.path("custom"));
    let shadow = Rc::new(
        Theme::new(
            [(ThemeColor::Accent, maestro_theme::ColorValue::Index(24))],
            [],
            ColorMode::Truecolor,
            ThemeOptions {
                name: Some("dark".into()),
                ..ThemeOptions::default()
            },
        )
        .unwrap(),
    );
    state.set_registered_themes(vec![Rc::clone(&shadow)]);
    let found = state.get_theme_by_name("dark").unwrap();
    assert!(Rc::ptr_eq(&found, &shadow));
    assert_eq!(accent(&found), "\x1b[38;5;24m");

    state.set_registered_themes(vec![]);
    let shipped = state.get_theme_by_name("dark").unwrap();
    assert!(!Rc::ptr_eq(&shipped, &shadow));
    assert_eq!(accent(&shipped), TRUECOLOR_ACCENT);
    assert_eq!(shipped.name(), Some("dark"));
}

#[test]
fn theme_builtin_reads_are_lazy_and_retryable() {
    if !in_child("theme_builtin_reads_are_lazy_and_retryable", TRUECOLOR) {
        return;
    }
    let scratch = Scratch::new();
    // Construction reads nothing: the shipped files do not exist yet.
    let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
    assert!(state.get_theme_by_name("dark").is_none());

    // A failed read or parse is not cached: both files are read again on the next lookup.
    scratch.write("themes/dark.json", &theme_text("dark", "#111111"));
    scratch.write("themes/light.json", "{");
    assert!(state.get_theme_by_name("dark").is_none());

    scratch.write("themes/dark.json", &theme_text("dark", "#445566"));
    scratch.write("themes/light.json", &theme_text("light", "#778899"));
    let repaired = state.get_theme_by_name("dark").unwrap();
    assert_eq!(accent(&repaired), "\x1b[38;2;68;85;102m");
}

#[test]
fn theme_builtin_cache_keeps_data_not_instances() {
    if !in_child("theme_builtin_cache_keeps_data_not_instances", TRUECOLOR) {
        return;
    }
    let scratch = Scratch::new();
    scratch.write("themes/dark.json", &theme_text("dark", "#445566"));
    scratch.write("themes/light.json", &theme_text("light", "#778899"));
    let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
    let first = state.get_theme_by_name("dark").unwrap();
    assert_eq!(accent(&first), "\x1b[38;2;68;85;102m");

    fs::remove_file(scratch.path("themes/dark.json")).unwrap();
    scratch.write("themes/light.json", "changed");
    let second = state.get_theme_by_name("dark").unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.get_color_mode(), ColorMode::Truecolor);
    assert_eq!(accent(&second), "\x1b[38;2;68;85;102m");
    assert_eq!(accent(&first), "\x1b[38;2;68;85;102m");
}

#[test]
fn theme_lists_json_names_with_ordinal_order() {
    let scratch = Scratch::new();
    for file in [
        "z.json",
        "a.json",
        ".json",
        "ignored.JSON",
        "ignore.json.bak",
        "é.json",
        "\u{10000}.json",
        "\u{E000}.json",
        "dark.json",
        "custom-only.json",
    ] {
        scratch.write(&format!("custom/{file}"), "not json");
    }
    fs::create_dir(scratch.path("custom/directory.json")).unwrap();
    let state = make(&scratch.path("custom"));
    state.set_registered_themes(vec![
        registered(Some("z"), None),
        registered(Some("registered-only"), None),
        registered(Some("A"), None),
        registered(Some("e\u{301}"), None),
    ]);
    assert_eq!(
        state.get_available_themes().unwrap(),
        [
            "",
            "A",
            "a",
            "custom-only",
            "dark",
            "directory",
            "e\u{301}",
            "light",
            "registered-only",
            "z",
            "é",
            "\u{10000}",
            "\u{E000}",
        ]
    );
}

#[test]
fn theme_path_inventory_keeps_first_owner_and_sorted_names() {
    let scratch = Scratch::new();
    for file in ["a.json", "dark.json", "z.json", "custom-only.json"] {
        scratch.write(&format!("custom/{file}"), "not json");
    }
    let custom = scratch.path("custom");
    let native = make(&custom);
    native.set_registered_themes(vec![
        registered(Some("z"), Some("registered-z")),
        registered(Some("reg-none"), None),
        registered(Some("reg-empty"), Some("")),
    ]);
    let listed = native.get_available_themes_with_paths().unwrap();
    let (dark, light) = (
        maestro_path::join(&[SHIPPED, "dark.json"]),
        maestro_path::join(&[SHIPPED, "light.json"]),
    );
    let a = maestro_path::join(&[&custom, "a.json"]);
    let (only, z) = (
        maestro_path::join(&[&custom, "custom-only.json"]),
        maestro_path::join(&[&custom, "z.json"]),
    );
    assert_eq!(
        names(&listed),
        [
            ("a", Some(a.as_str())),
            ("custom-only", Some(only.as_str())),
            ("dark", Some(dark.as_str())),
            ("light", Some(light.as_str())),
            ("reg-empty", Some("")),
            ("reg-none", None),
            ("z", Some(z.as_str())),
        ]
    );
}

#[test]
fn theme_path_inventory_orders_canonically_equal_names_by_directory_byte_order() {
    let scratch = Scratch::new();
    for (pair, names) in [
        ("A.json/a.json", ["A.json", "a.json"]),
        (
            "\u{e9}.json/e\u{301}.json",
            ["\u{e9}.json", "e\u{301}.json"],
        ),
    ] {
        if !scratch.keeps_distinct(&[OsStr::new(names[0]), OsStr::new(names[1])]) {
            eprintln!("host folds {pair} into one entry");
            return;
        }
    }
    for file in ["A.json", "a.json", "\u{e9}.json", "e\u{301}.json"] {
        scratch.write(&format!("custom/{file}"), "x");
    }
    let custom = scratch.path("custom");
    let listed = make(&custom).get_available_themes_with_paths().unwrap();
    let tied: Vec<_> = names(&listed)
        .into_iter()
        .filter(|(name, _)| ["A", "a", "\u{e9}", "e\u{301}"].contains(name))
        .collect();
    let upper = maestro_path::join(&[&custom, "A.json"]);
    let lower = maestro_path::join(&[&custom, "a.json"]);
    let composed = maestro_path::join(&[&custom, "\u{e9}.json"]);
    let decomposed = maestro_path::join(&[&custom, "e\u{301}.json"]);
    // The collator orders the case pair; only the canonically equal pair falls back to the
    // order the directory reports.
    assert_eq!(
        tied,
        [
            ("a", Some(lower.as_str())),
            ("A", Some(upper.as_str())),
            ("e\u{301}", Some(decomposed.as_str())),
            ("\u{e9}", Some(composed.as_str())),
        ]
    );
}

#[test]
fn theme_path_inventory_sorts_with_the_supplied_operation() {
    // Swedish sorts `z` before `ö`; code-point order puts `B` before `a`; English puts `ö`
    // before `z`. Only the process locale's comparison gives this order.
    let test = "theme_path_inventory_sorts_with_the_supplied_operation";
    if !in_child(test, &[("LANG", "sv_SE.UTF-8")]) {
        return;
    }
    let scratch = Scratch::new();
    for file in ["dark.json", "\u{f6}.json", "B.json"] {
        scratch.write(&format!("custom/{file}"), "not json");
    }
    let custom = scratch.path("custom");
    let state = make(&custom);
    state.set_registered_themes(vec![
        registered(Some("B"), Some("ignored")),
        registered(Some("z"), Some("zed")),
        registered(Some("a"), Some("ay")),
    ]);
    let listed = state.get_available_themes_with_paths().unwrap();
    let (dark, light) = (
        maestro_path::join(&[SHIPPED, "dark.json"]),
        maestro_path::join(&[SHIPPED, "light.json"]),
    );
    let (b, o) = (
        maestro_path::join(&[&custom, "B.json"]),
        maestro_path::join(&[&custom, "\u{f6}.json"]),
    );
    assert_eq!(
        names(&listed),
        [
            ("a", Some("ay")),
            ("B", Some(b.as_str())),
            ("dark", Some(dark.as_str())),
            ("light", Some(light.as_str())),
            ("z", Some("zed")),
            ("\u{f6}", Some(o.as_str())),
        ]
    );
}

#[test]
fn theme_inventory_handles_absent_custom_directory() {
    let scratch = Scratch::new();
    let state = make(&scratch.path("custom"));
    state.set_registered_themes(vec![registered(Some("z"), Some("registered-z"))]);
    assert_eq!(
        state.get_available_themes().unwrap(),
        ["dark", "light", "z"]
    );
    let listed = state.get_available_themes_with_paths().unwrap();
    let (dark, light) = (
        maestro_path::join(&[SHIPPED, "dark.json"]),
        maestro_path::join(&[SHIPPED, "light.json"]),
    );
    assert_eq!(
        names(&listed),
        [
            ("dark", Some(dark.as_str())),
            ("light", Some(light.as_str())),
            ("z", Some("registered-z")),
        ]
    );
}

#[test]
fn theme_discovery_propagates_loading_failures() {
    let scratch = Scratch::new();
    let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
    for error in [
        state.get_available_themes().unwrap_err(),
        state.get_available_themes_with_paths().unwrap_err(),
    ] {
        assert_eq!(io_kind(&error), Some(io::ErrorKind::NotFound));
    }

    scratch.write("themes/dark.json", &theme_text("dark", "#111111"));
    scratch.write("themes/light.json", "{");
    let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
    let error = state.get_available_themes().unwrap_err();
    assert!(!error.to_string().contains("Failed to parse theme"));
    assert!(
        error
            .source()
            .is_some_and(|cause| cause.downcast_ref::<serde_json::Error>().is_some())
    );

    scratch.write("custom", "a file");
    let state = make(&scratch.path("custom"));
    for error in [
        state.get_available_themes().unwrap_err(),
        state.get_available_themes_with_paths().unwrap_err(),
    ] {
        assert_eq!(io_kind(&error), Some(io::ErrorKind::NotADirectory));
    }
}

#[test]
fn theme_custom_lookup_rereads_without_attaching_a_path() {
    if !in_child(
        "theme_custom_lookup_rereads_without_attaching_a_path",
        TRUECOLOR,
    ) {
        return;
    }
    let scratch = Scratch::new();
    scratch.write("custom/mine.json", &theme_text("authored-name", "#ABCDEF"));
    let state = make(&scratch.path("custom"));
    let first = state.get_theme_by_name("mine").unwrap();
    assert_eq!(first.name(), Some("authored-name"));
    assert_eq!(first.source_path(), None);
    assert_eq!(accent(&first), "\x1b[38;2;171;205;239m");

    scratch.write("custom/mine.json", &theme_text("changed-name", "#102030"));
    let second = state.get_theme_by_name("mine").unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.name(), Some("changed-name"));
    assert_eq!(second.source_path(), None);
    assert_eq!(accent(&second), "\x1b[38;2;16;32;48m");

    let path = scratch.path("custom/mine.json");
    let direct = load_theme_from_path(&path, Some(ColorMode::Truecolor)).unwrap();
    assert_eq!(direct.source_path(), Some(path.as_str()));
}

#[test]
fn theme_lookup_suppresses_load_failures() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path("custom/directory.json")).unwrap();
    scratch.write("custom/malformed.json", "{");
    let mut schema = shipped_dark();
    schema.as_object_mut().unwrap().shift_remove("colors");
    scratch.write("custom/schema.json", &schema.to_string());
    let mut alias = shipped_dark();
    alias["colors"]["accent"] = "no-such-variable".into();
    scratch.write("custom/alias.json", &alias.to_string());
    scratch.write("custom/hex.json", &theme_text("hex", "#12"));

    let state = make(&scratch.path("custom"));
    for name in [
        "missing",
        "directory",
        "malformed",
        "schema",
        "alias",
        "hex",
    ] {
        assert!(state.get_theme_by_name(name).is_none(), "{name}");
    }

    let light = Scratch::new();
    light.write("themes/dark.json", &theme_text("dark", "#111111"));
    light.write("themes/light.json", "{");
    let broken = ThemeState::new(dirs(&light.path("themes"), &light.path("custom")));
    assert!(broken.get_theme_by_name("dark").is_none());
}

#[test]
fn theme_lookup_rejects_shipped_data_without_a_colors_object() {
    for colors in [serde_json::Value::Null, serde_json::json!(["x"])] {
        let scratch = Scratch::new();
        let mut doc = shipped_dark();
        doc["colors"] = colors;
        scratch.write("themes/dark.json", &doc.to_string());
        scratch.write("themes/light.json", &theme_text("light", "#778899"));
        scratch.write("custom/mine.json", &theme_text("mine", "#123456"));
        let state = ThemeState::new(dirs(&scratch.path("themes"), &scratch.path("custom")));
        assert!(state.get_theme_by_name("dark").is_none());
        assert!(state.get_theme_by_name("light").is_some());
        assert!(state.get_theme_by_name("mine").is_some());
    }
}

#[test]
fn theme_lookup_joins_authored_names_without_trimming() {
    let scratch = Scratch::new();
    scratch.write("user/.json", &theme_text("empty-request", "#000001"));
    scratch.write("user/ spaced .json", &theme_text("  stored  ", "#000002"));
    scratch.write("user/normalized.json", &theme_text("inside", "#000003"));
    scratch.write("custom/normalized.json", &theme_text("outside", "#000004"));
    let state = make(&scratch.path("user"));
    let name_of = |requested: &str| {
        state
            .get_theme_by_name(requested)
            .unwrap()
            .name()
            .map(str::to_owned)
    };
    assert_eq!(name_of("").as_deref(), Some("empty-request"));
    assert_eq!(name_of(" spaced ").as_deref(), Some("  stored  "));
    assert_eq!(name_of("./normalized").as_deref(), Some("inside"));
    assert_eq!(name_of("../custom/normalized").as_deref(), Some("outside"));
}
