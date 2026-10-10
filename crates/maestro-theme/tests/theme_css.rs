//! Resolved color records and optional export colors.
#[cfg(test)]
mod live;

use live::{Ops, Scratch, custom_json, state};
use maestro_theme::{
    ColorMode, Theme, ThemeExportColors, ThemeOptions, ThemeState, is_light_theme,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// The 256 expected hex colors, one per palette index.
const PALETTE_HEX: &str = include_str!("live/palette_hex.txt");

/// A state over a scratch directory holding `<name>.json` custom themes.
#[cfg(test)]
fn custom(tag: &str, themes: &[(&str, Value)]) -> (Scratch, Rc<Ops>, Rc<ThemeState>) {
    let scratch = Scratch::new(tag);
    for (name, json) in themes {
        scratch.write(&format!("custom/{name}.json"), &json.to_string());
    }
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    (scratch, ops, state)
}

/// The export section of the shipped dark theme under another name.
#[cfg(test)]
fn with_export(export: &Value) -> Value {
    let mut json = custom_json("e", "#010203");
    json["export"] = export.clone();
    json
}

/// A custom document whose accent and export page background are both `color`.
#[cfg(test)]
fn with_page_bg(name: &str, color: &str) -> Value {
    let mut json = custom_json(name, color);
    json["export"] = json!({ "pageBg": color });
    json
}

/// Resolved value of one key.
#[cfg(test)]
fn color(colors: &[(String, String)], key: &str) -> String {
    colors
        .iter()
        .find(|(name, _)| name == key)
        .unwrap()
        .1
        .clone()
}

/// Export colors of a document stored as custom theme `e`.
#[cfg(test)]
fn exported(json: &Value) -> ThemeExportColors {
    let (_scratch, _ops, state) = custom("export", &[("e", json.clone())]);
    state.get_theme_export_colors(Some("e"))
}

/// Export colors with the given field and nothing else.
#[cfg(test)]
fn export_field(field: &str, value: &Value) -> ThemeExportColors {
    exported(&with_export(&json!({ field: value })))
}

/// Export colors with the given page background and nothing else.
#[cfg(test)]
fn page_bg(value: &Value) -> Option<String> {
    export_field("pageBg", value).page_bg
}

#[test]
fn theme_export_source_precedence_differs_from_instance_lookup() {
    let scratch = Scratch::new("precedence");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    let register = |name: &str, path: Option<String>| {
        let options = ThemeOptions {
            name: Some(name.to_owned()),
            source_path: path,
            ..ThemeOptions::default()
        };
        Rc::new(Theme::new([], [], ColorMode::Truecolor, options).unwrap())
    };
    scratch.write(
        "elsewhere/dark.json",
        &custom_json("dark", "#ffffff").to_string(),
    );
    scratch.write(
        "elsewhere/reg.json",
        &custom_json("reg", "#111111").to_string(),
    );
    state.set_registered_themes(vec![
        register("dark", Some(scratch.path("elsewhere/dark.json"))),
        register("reg", Some(scratch.path("elsewhere/reg.json"))),
        register("nosource", None),
        register("empty", Some(String::new())),
    ]);

    let shipped = state.get_resolved_theme_colors(Some("dark")).unwrap();
    assert_eq!(color(&shipped, "accent"), "#D9A066");
    assert_eq!(
        color(
            &state.get_resolved_theme_colors(Some("reg")).unwrap(),
            "accent"
        ),
        "#111111"
    );
    scratch.write(
        "elsewhere/reg.json",
        &custom_json("reg", "#222222").to_string(),
    );
    assert_eq!(
        color(
            &state.get_resolved_theme_colors(Some("reg")).unwrap(),
            "accent"
        ),
        "#222222"
    );
    for name in ["nosource", "empty"] {
        let error = state.get_resolved_theme_colors(Some(name)).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Theme \"{name}\" does not have a source path for export")
        );
    }
    let missing = state.get_resolved_theme_colors(Some("ghost")).unwrap_err();
    assert_eq!(missing.to_string(), "Theme not found: ghost");
}

#[test]
fn theme_css_expands_every_palette_index() {
    let mut json = custom_json("palette", "#010203");
    for index in 0..=255 {
        json["colors"][format!("palette{index}")] = index.into();
    }
    let (_scratch, _ops, state) = custom("palette", &[("palette", json)]);
    let colors = state.get_resolved_theme_colors(Some("palette")).unwrap();
    let expected: Vec<_> = PALETTE_HEX.lines().collect();
    assert_eq!(expected.len(), 256);
    for (index, hex) in expected.into_iter().enumerate() {
        assert_eq!(
            color(&colors, &format!("palette{index}")),
            hex,
            "index {index}"
        );
    }
}

#[test]
fn theme_resolved_css_preserves_key_order_and_empty_fallback() {
    let mut json = custom_json("light", "#010203");
    let extras = [
        ("4294967295", "#000001"),
        ("4294967294", "#000002"),
        ("-1", "#000003"),
        ("1.0", "#000004"),
        ("00", "#000005"),
        ("10", "#000006"),
        ("2", "#000007"),
        ("1", "#000008"),
        ("0", "#000009"),
        ("01", "#00000a"),
        ("before", "#010203"),
        ("__proto__", "#112233"),
        ("after", "#040506"),
    ];
    for (key, value) in extras {
        json["colors"][key] = value.into();
    }
    json["colors"]["text"] = "".into();
    json["colors"]["border"] = "accent".into();
    let (_scratch, _ops, state) = custom("order", &[("custom", json)]);
    let colors = state.get_resolved_theme_colors(Some("custom")).unwrap();
    let keys: Vec<_> = colors.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys[..5], ["0", "1", "2", "10", "4294967294"]);
    assert_eq!(keys[5], "accent");
    let tail = [
        "4294967295",
        "-1",
        "1.0",
        "00",
        "01",
        "before",
        "__proto__",
        "after",
    ];
    assert_eq!(keys[keys.len() - tail.len()..], tail);
    assert_eq!(color(&colors, "__proto__"), "#112233");
    assert_eq!(color(&colors, "4294967294"), "#000002");
    assert_eq!(color(&colors, "border"), "#D9A066");
    assert_eq!(color(&colors, "text"), "#e5e5e7");

    let light = state_for_shipped();
    assert_eq!(
        color(
            &light.get_resolved_theme_colors(Some("light")).unwrap(),
            "text"
        ),
        "#000000"
    );
}

/// A state over the shipped themes only.
#[cfg(test)]
fn state_for_shipped() -> Rc<ThemeState> {
    custom("shipped", &[]).2
}

#[test]
fn theme_export_selection_uses_explicit_current_then_default() {
    let (_scratch, ops, state) = custom(
        "selection",
        &[
            ("one", with_page_bg("one", "#010101")),
            ("two", with_page_bg("two", "#020202")),
        ],
    );
    ops.set("COLORFGBG", "0;8");
    let page = |name: Option<&str>| state.get_theme_export_colors(name).page_bg;
    assert_eq!(page(None), Some("#F2E8DC".to_owned()));
    assert_eq!(
        color(&state.get_resolved_theme_colors(None).unwrap(), "accent"),
        "#B7410E"
    );
    assert!(ops.read("COLORFGBG"));

    assert_eq!(page(Some("one")), Some("#010101".to_owned()));
    state.init_theme(Some("one"), None).unwrap();
    assert_eq!(page(None), Some("#010101".to_owned()));
    assert_eq!(page(Some("two")), Some("#020202".to_owned()));
    assert_eq!(
        color(&state.get_resolved_theme_colors(None).unwrap(), "accent"),
        "#010101"
    );
    let explicit = state.get_resolved_theme_colors(Some("two")).unwrap();
    assert_eq!(color(&explicit, "accent"), "#020202");
    let empty = state.get_resolved_theme_colors(Some("")).unwrap_err();
    assert_eq!(empty.to_string(), "Theme not found: ");
    assert_eq!(page(Some("")), None);

    state
        .set_theme_instance(state.get_theme_by_name("one").unwrap())
        .unwrap();
    let memory = state.get_resolved_theme_colors(None).unwrap_err();
    assert_eq!(memory.to_string(), "Theme not found: <in-memory>");
    assert_eq!(page(None), None);
}

#[test]
fn theme_export_with_a_name_never_reads_the_terminal_background() {
    let (_scratch, ops, state) = custom(
        "explicit-only",
        &[
            ("one", with_page_bg("one", "#010101")),
            ("two", with_page_bg("two", "#020202")),
        ],
    );
    ops.set("COLORFGBG", "0;8");
    assert_eq!(
        state.get_theme_export_colors(Some("one")).page_bg,
        Some("#010101".to_owned())
    );
    state.get_resolved_theme_colors(Some("two")).unwrap();
    state.get_resolved_theme_colors(Some("")).unwrap_err();
    state.init_theme(Some("one"), None).unwrap();
    assert_eq!(
        state.get_theme_export_colors(None).page_bg,
        Some("#010101".to_owned())
    );
    assert!(!ops.read("COLORFGBG"));
}

#[test]
fn theme_light_check_uses_only_the_supplied_name() {
    let state = state_for_shipped();
    state.init_theme(Some("light"), None).unwrap();
    for (name, expected) in [
        (Some("light"), true),
        (None, false),
        (Some(""), false),
        (Some("dark"), false),
        (Some("Light"), false),
        (Some("custom"), false),
    ] {
        assert_eq!(is_light_theme(name), expected, "name {name:?}");
    }
}

#[test]
fn maestro_theme_exports_resolved_alias_colors() {
    let mut json = custom_json("export-vars", "#010203");
    json["vars"]["pageBgVar"] = "#112233".into();
    json["vars"]["pageBgAlias"] = "pageBgVar".into();
    json["vars"]["cardBgVar"] = "#223344".into();
    json["vars"]["infoBgVar"] = "#445566".into();
    json["export"] =
        json!({ "pageBg": "pageBgAlias", "cardBg": "cardBgVar", "infoBg": "infoBgVar" });
    let colors = exported(&json);
    assert_eq!(colors.page_bg, Some("#112233".to_owned()));
    assert_eq!(colors.card_bg, Some("#223344".to_owned()));
    assert_eq!(colors.info_bg, Some("#445566".to_owned()));

    json["vars"]["deep"] = "deeper".into();
    json["vars"]["deeper"] = "#abcdef".into();
    json["export"] = json!({ "pageBg": "deep", "cardBg": 24, "infoBg": "" });
    let colors = exported(&json);
    assert_eq!(colors.page_bg, Some("#abcdef".to_owned()));
    assert_eq!(colors.card_bg, Some("#005f87".to_owned()));
    assert_eq!(colors.info_bg, None);

    json["vars"]["slot"] = 24.into();
    json["vars"]["slotAlias"] = "slot".into();
    json["vars"]["zero"] = 0.into();
    json["vars"]["last"] = 255.into();
    json["export"] = json!({ "pageBg": "slotAlias", "cardBg": "zero", "infoBg": "last" });
    let colors = exported(&json);
    assert_eq!(colors.page_bg, Some("#005f87".to_owned()));
    assert_eq!(colors.card_bg, Some("#000000".to_owned()));
    assert_eq!(colors.info_bg, Some("#eeeeee".to_owned()));
}

#[test]
fn theme_export_recursive_alias_yields_no_colors_while_resolution_reports_it() {
    let mut json = custom_json("loop", "#010203");
    json["vars"]["a"] = "b".into();
    json["vars"]["b"] = "a".into();
    json["export"] = json!({ "pageBg": "#112233", "cardBg": "a" });
    assert_eq!(exported(&json), ThemeExportColors::default());

    json["colors"]["accent"] = "a".into();
    let (_scratch, _ops, state) = custom("loop-colors", &[("loop", json)]);
    let error = state.get_resolved_theme_colors(Some("loop")).unwrap_err();
    assert_eq!(error.to_string(), "Circular variable reference detected: a");
}

#[test]
fn theme_export_optional_fields_omit_empty_and_missing_values() {
    assert_eq!(page_bg(&json!("#AbCdEf")), Some("#AbCdEf".to_owned()));
    assert_eq!(page_bg(&json!("")), None);
    assert_eq!(page_bg(&json!(0)), Some("#000000".to_owned()));
    assert_eq!(page_bg(&json!(24)), Some("#005f87".to_owned()));
    assert_eq!(page_bg(&json!(255)), Some("#eeeeee".to_owned()));
    assert_eq!(page_bg(&json!("accent")), Some("#D9A066".to_owned()));
    for (field, select) in [
        ("cardBg", (|c: ThemeExportColors| c.card_bg) as fn(_) -> _),
        ("infoBg", |c| c.info_bg),
    ] {
        for (value, expected) in [
            (json!("#AbCdEf"), Some("#AbCdEf")),
            (json!(""), None),
            (json!(0), Some("#000000")),
            (json!(24), Some("#005f87")),
            (json!(255), Some("#eeeeee")),
            (json!("accent"), Some("#D9A066")),
        ] {
            let colors = export_field(field, &value);
            assert_eq!(
                select(colors),
                expected.map(str::to_owned),
                "{field} {value}"
            );
        }
    }
    let json = with_export(&json!({}));
    assert_eq!(exported(&json), ThemeExportColors::default());
    let mut absent = custom_json("e", "#010203");
    absent.as_object_mut().unwrap().remove("export");
    assert_eq!(exported(&absent), ThemeExportColors::default());
    let mixed = with_export(&json!({ "cardBg": "#010203", "infoBg": "" }));
    let colors = exported(&mixed);
    assert_eq!(
        (colors.page_bg, colors.card_bg, colors.info_bg),
        (None, Some("#010203".to_owned()), None)
    );
}

#[test]
fn theme_export_admission_checks_selected_fields() {
    let bad = [
        json!(null),
        json!(true),
        json!({}),
        json!([]),
        json!(256),
        json!(-1),
        json!(1.5),
    ];
    let mut documents = Vec::new();
    for field in ["pageBg", "cardBg", "infoBg"] {
        for value in &bad {
            documents.push(with_export(&json!({ field: value })));
        }
    }
    for section in [json!(null), json!([]), json!(true), json!("text")] {
        documents.push(with_export(&section));
    }
    for (index, document) in documents.iter().enumerate() {
        let (_scratch, _ops, state) = custom("admission", &[("e", document.clone())]);
        assert_eq!(
            state.get_theme_export_colors(Some("e")),
            ThemeExportColors::default(),
            "case {index}"
        );
        let error = state.get_resolved_theme_colors(Some("e")).unwrap_err();
        assert!(
            error.to_string().starts_with("Invalid theme \"e\":"),
            "case {index}"
        );
    }
}

#[test]
fn theme_export_suppresses_errors_without_hiding_resolved_errors() {
    let mut dangling =
        with_export(&json!({ "pageBg": "#010203", "cardBg": "#020304", "infoBg": "nowhere" }));
    let mut cycle = with_export(&json!({ "pageBg": "loop" }));
    cycle["vars"]["loop"] = "loop".into();
    let scratch = Scratch::new("suppress");
    scratch.write("custom/dangling.json", &dangling.to_string());
    scratch.write("custom/cycle.json", &cycle.to_string());
    scratch.write("custom/malformed.json", "{");
    dangling["colors"]["accent"] = "nowhere".into();
    scratch.write("custom/bad-color.json", &dangling.to_string());
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    for name in ["dangling", "cycle", "malformed", "missing", "bad-color"] {
        assert_eq!(
            state.get_theme_export_colors(Some(name)),
            ThemeExportColors::default(),
            "{name}"
        );
        let error = state.get_resolved_theme_colors(Some(name));
        assert_eq!(
            error.is_err(),
            name != "dangling" && name != "cycle",
            "{name}"
        );
    }
    let cycle_error = state.get_resolved_theme_colors(Some("cycle")).unwrap();
    assert_eq!(color(&cycle_error, "accent"), "#010203");
}

#[test]
fn theme_export_resolves_only_the_requested_color_plane() {
    let mut unused_bad_color = with_export(&json!({ "pageBg": "#FFF" }));
    unused_bad_color["colors"]["accent"] = "unknownVar".into();
    let mut bad_export = with_export(&json!({ "pageBg": "unknownVar" }));
    bad_export["vars"]["spare"] = "#101010".into();
    let (_scratch, _ops, state) = custom(
        "plane",
        &[("bad-color", unused_bad_color), ("bad-export", bad_export)],
    );

    assert_eq!(
        state.get_theme_export_colors(Some("bad-color")).page_bg,
        Some("#FFF".to_owned())
    );
    let error = state
        .get_resolved_theme_colors(Some("bad-color"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Variable reference not found: unknownVar"
    );
    assert_eq!(
        state.get_theme_export_colors(Some("bad-export")),
        ThemeExportColors::default()
    );
    let colors = state.get_resolved_theme_colors(Some("bad-export")).unwrap();
    assert_eq!(color(&colors, "accent"), "#010203");
}

#[test]
fn theme_export_keeps_last_duplicate_and_ignores_unknown_fields() {
    let (scratch, ops, _state) = custom("duplicates", &[]);
    let base = custom_json("dup", "#010203").to_string();
    let tail = r##","export":{"pageBg":true},"export":{"pageBg":null,"pageBg":"#010203","cardBg":1.0,"infoBg":-0.0,"mystery":{"deep":[1,{"x":null}]}}}"##;
    let text = format!("{}{tail}", base.strip_suffix('}').unwrap());
    scratch.write("custom/dup.json", &text);
    let state = live::state(&scratch, &ops);
    let colors = state.get_theme_export_colors(Some("dup"));
    assert_eq!(colors.page_bg, Some("#010203".to_owned()));
    assert_eq!(colors.card_bg, Some("#800000".to_owned()));
    assert_eq!(colors.info_bg, Some("#000000".to_owned()));
    assert!(state.get_resolved_theme_colors(Some("dup")).is_ok());
}

#[test]
fn theme_scratch_directories_are_exclusively_owned() {
    let (_first, _, first) = custom("export", &[("e", with_page_bg("e", "#111111"))]);
    let (_second, _, second) = custom("export", &[("e", with_page_bg("e", "#222222"))]);
    assert_eq!(
        first.get_theme_export_colors(Some("e")).page_bg,
        Some("#111111".to_owned())
    );
    assert_eq!(
        second.get_theme_export_colors(Some("e")).page_bg,
        Some("#222222".to_owned())
    );
}
