//! Brand pack loading, theme projection, presentation values, CSS and mark templates.
#[cfg(test)]
mod brand_support;
#[cfg(test)]
mod live;

use brand_support::{
    ALTERNATE_PATH, FORGE_NATIVE, FORGE_PATH, Mem, SIGNAL_NATIVE, alternate, check_prefixes, forge,
    forge_json, load_value, mode_id, presented, required_colors, terminal,
};
use live::{Ops, Scratch, custom_json, state};
use maestro_request::source_info::{SourceInfo, SourceOrigin, SourceScope};
use maestro_theme::{
    BrandMode, BrandPack, BrandPresentation, ColorMode, NativeThemeOperations, Theme, ThemeColor,
    ThemeError, ThemeExportColors, ThemeOptions, load_brand_pack, load_theme_from_path,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::error::Error;
use std::rc::Rc;

#[cfg(test)]
const MODES: [BrandMode; 2] = [BrandMode::Dark, BrandMode::Light];

/// The message of an expected failure.
#[cfg(test)]
fn failure<T>(result: Result<T, ThemeError>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("expected a failure"),
    }
}

/// The shared pack document after an edit.
#[cfg(test)]
fn edited(edit: impl FnOnce(&mut Value)) -> Value {
    let mut value = forge_json();
    edit(&mut value);
    value
}

/// The shared pack document with one JSON Pointer member set, or removed when `None`.
#[cfg(test)]
fn changed(pointer: &str, value: Option<Value>) -> Value {
    edited(|pack| {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let key = key.replace("~1", "/").replace("~0", "~");
        let object = pack.pointer_mut(parent).unwrap().as_object_mut().unwrap();
        match value {
            Some(value) => object.insert(key, value),
            None => object.remove(&key),
        };
    })
}

/// Owned copy of a borrowed map.
#[cfg(test)]
fn owned(map: &BTreeMap<&str, &str>) -> BTreeMap<String, String> {
    map.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

/// Compare a pack's projected theme and exports to the recorded entry.
#[cfg(test)]
fn check_projection(pack: &BrandPack, mode: BrandMode, id: &str) {
    let document = pack.theme_json(mode).unwrap();
    assert!(document.ends_with("}\n"));
    let json: Value = serde_json::from_str(&document).unwrap();
    let keys: Vec<_> = json["colors"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, required_colors());
    let expected = terminal(id);
    check_prefixes(&document, &expected);
    let scratch = Scratch::new("projection");
    scratch.write(&format!("custom/{id}.json"), &document);
    let state = state(&scratch, &Ops::new(&[]));
    assert_eq!(
        state.get_theme_export_colors(Some(id)),
        ThemeExportColors {
            page_bg: Some(expected.export.page),
            card_bg: Some(expected.export.card),
            info_bg: Some(expected.export.info),
        }
    );
}

/// Compare resolved presentation, CSS and every template to the recorded entry.
#[cfg(test)]
fn check_presentation(pack: &BrandPack, mode: BrandMode, id: &str, files: &Mem) {
    let expected = presented(id);
    let presentation = pack.presentation(mode).unwrap();
    assert_eq!(owned(&presentation.colors), expected.colors);
    assert_eq!(presentation.css_properties(), expected.css);
    assert_eq!(owned(&presentation.ansi), expected.ansi);
    assert_eq!(owned(&presentation.truecolor), expected.truecolor);
    let glyphs: BTreeMap<_, _> = presentation
        .glyphs
        .iter()
        .map(|(role, glyph)| {
            let record = brand_support::Glyph {
                symbol: glyph.symbol.to_owned(),
                color: glyph.color.to_owned(),
            };
            ((*role).to_owned(), record)
        })
        .collect();
    assert_eq!(glyphs, expected.glyphs);
    assert_eq!(expected.svg.len(), 1 + presentation.mark.variants.len());
    for (name, text) in &expected.svg {
        let variant = (name != "template").then_some(name.as_str());
        assert_eq!(
            &pack.mark_svg(mode, variant, files).unwrap(),
            text,
            "{id} {name}"
        );
    }
}

/// All prefixes of a custom document loaded in both color modes.
#[cfg(test)]
fn custom_prefixes() -> Vec<String> {
    let document = custom_json("custom", "#123456").to_string();
    let files = Mem::new(&[("custom.json", &document)]);
    [ColorMode::Truecolor, ColorMode::Color256]
        .into_iter()
        .flat_map(|mode| {
            let theme = load_theme_from_path("custom.json", Some(mode), &files).unwrap();
            ["accent", "border", "text", "mdCode"]
                .map(|key| theme.fg(&ThemeColor::Named(key.into()), "x").unwrap())
        })
        .collect()
}

#[test]
fn brand_pack_projects_both_builtin_themes() {
    let pack = forge();
    for mode in MODES {
        check_projection(&pack, mode, &format!("forge-{}", mode_id(mode)));
    }
}

#[test]
fn maestro_identity_override_reaches_all_style_outputs() {
    let before = custom_prefixes();
    assert_eq!(before[0], "\x1b[38;2;18;52;86mx\x1b[39m");
    let forge_files = Mem::forge();
    let forge_pack = load_brand_pack(FORGE_PATH, &forge_files).unwrap();
    let alternate_files = Mem::alternate();
    let alternate_pack = load_brand_pack(ALTERNATE_PATH, &alternate_files).unwrap();
    for mode in MODES {
        let mode_name = mode_id(mode);
        for (pack, files, name) in [
            (&forge_pack, &forge_files, "forge"),
            (&alternate_pack, &alternate_files, "alternate"),
        ] {
            let id = format!("{name}-{mode_name}");
            if name == "alternate" {
                check_projection(pack, mode, &id);
            }
            check_presentation(pack, mode, &id, files);
        }
    }
    assert_ne!(
        terminal("forge-dark").truecolor["accent"],
        terminal("alternate-dark").truecolor["accent"]
    );
    assert_eq!(custom_prefixes(), before);
}

/// The four terminal-default keys stay empty in every projected theme.
#[cfg(test)]
fn assert_terminal_defaults() {
    for pack in [forge(), alternate()] {
        for mode in MODES {
            let document = pack.theme_json(mode).unwrap();
            let json: Value = serde_json::from_str(&document).unwrap();
            let theme = load_theme_from_path(
                "t.json",
                Some(ColorMode::Truecolor),
                &Mem::new(&[("t.json", &document)]),
            )
            .unwrap();
            for key in ["text", "userMessageText", "customMessageText", "toolTitle"] {
                assert_eq!(json["colors"][key], "");
                assert_eq!(
                    theme.get_fg_ansi(&ThemeColor::Named(key.into())).unwrap(),
                    "\x1b[39m"
                );
            }
        }
    }
}

/// Provenance of a registered package theme.
#[cfg(test)]
fn package_source() -> Rc<RefCell<SourceInfo>> {
    Rc::new(RefCell::new(SourceInfo {
        path: "/pkg/mine.json".into(),
        source: "pkg".into(),
        scope: SourceScope::Project,
        origin: SourceOrigin::Package,
        base_dir: Some("/pkg".into()),
    }))
}

#[test]
fn brand_pack_preserves_terminal_defaults_and_custom_themes() {
    let scratch = Scratch::new("custom");
    let ops = Ops::new(&[]);
    let state = state(&scratch, &ops);
    scratch.write(
        "custom/filemine.json",
        &custom_json("filemine", "#123456").to_string(),
    );
    let source = package_source();
    let registered = Rc::new(
        Theme::new(
            vec![(
                ThemeColor::Accent,
                maestro_theme::ColorValue::String("#010203".into()),
            )],
            vec![],
            ColorMode::Truecolor,
            ThemeOptions {
                name: Some("mine".into()),
                source_info: Some(Rc::clone(&source)),
                ..ThemeOptions::default()
            },
        )
        .unwrap(),
    );
    state.set_registered_themes(vec![Rc::clone(&registered)]);
    let file_before = state
        .get_theme_by_name("filemine")
        .unwrap()
        .fg(&ThemeColor::Accent, "x")
        .unwrap();
    assert_eq!(file_before, "\x1b[38;2;18;52;86mx\x1b[39m");
    assert_terminal_defaults();
    assert!(Rc::ptr_eq(
        &state.get_theme_by_name("mine").unwrap(),
        &registered
    ));
    assert_eq!(registered.name(), Some("mine"));
    let kept = state
        .get_theme_by_name("mine")
        .unwrap()
        .source_info()
        .unwrap();
    assert!(Rc::ptr_eq(&kept, &source));
    assert_eq!(kept.borrow().base_dir.as_deref(), Some("/pkg"));
    let file_after = state.get_theme_by_name("filemine").unwrap();
    assert_eq!(
        file_after.fg(&ThemeColor::Accent, "x").unwrap(),
        file_before
    );
    assert_eq!(file_after.name(), Some("filemine"));
    let colors = |name| state.get_resolved_theme_colors(Some(name)).unwrap();
    let text = |name| {
        colors(name)
            .into_iter()
            .find(|(key, _)| key == "text")
            .unwrap()
            .1
    };
    assert_eq!(text("dark"), "#e5e5e7");
    assert_eq!(text("light"), "#000000");
}

#[test]
fn brand_pack_aliases_do_not_depend_on_pack_or_swatch_names() {
    let renamed = edited(|pack| {
        pack["name"] = "Renamed".into();
        let palette = pack["palette"].as_object().unwrap().clone();
        let names: Vec<_> = palette.keys().cloned().collect();
        let swatch = |old: &str| format!("s{}", names.iter().position(|n| n == old).unwrap());
        pack["palette"] = palette
            .iter()
            .map(|(k, v)| (swatch(k), v.clone()))
            .collect();
        for mode in ["dark", "light"] {
            for value in pack["modes"][mode]["colors"]
                .as_object_mut()
                .unwrap()
                .values_mut()
            {
                *value = swatch(value.as_str().unwrap()).into();
            }
        }
    });
    let (base, other) = (forge(), load_value(&renamed).unwrap());
    for mode in MODES {
        assert_eq!(
            other.theme_json(mode).unwrap(),
            base.theme_json(mode).unwrap()
        );
        let (a, b) = (
            other.presentation(mode).unwrap(),
            base.presentation(mode).unwrap(),
        );
        assert_eq!(a.css_properties(), b.css_properties());
        assert_eq!(a.glyphs, b.glyphs);
        assert_eq!(a.ansi, b.ansi);
    }
    assert_eq!(other.presentation(BrandMode::Dark).unwrap().name, "Renamed");
    let changed = edited(|pack| pack["palette"]["vector-steel"] = "#aBcDeF".into());
    let changed = load_value(&changed).unwrap();
    let (new, old) = (
        changed.presentation(BrandMode::Dark).unwrap(),
        base.presentation(BrandMode::Dark).unwrap(),
    );
    for (role, color) in &new.colors {
        let expected = if *role == "border" {
            "#aBcDeF"
        } else {
            old.colors[role]
        };
        assert_eq!(*color, expected, "{role}");
    }
}

#[test]
fn brand_pack_terminal_and_glyph_aliases_reach_outputs() {
    let pack = load_value(&edited(|pack| {
        pack["terminal"]["ansi"]["blue"] = "warning".into();
        pack["terminal"]["truecolor"]["accent"] = "warning".into();
        pack["glyphs"]["working"] = json!({"symbol": "w!", "color": "warning"});
    }))
    .unwrap();
    let base = forge();
    let (new, old) = (
        pack.presentation(BrandMode::Light).unwrap(),
        base.presentation(BrandMode::Light).unwrap(),
    );
    assert_eq!(old.ansi["blue"], "#B7410E");
    assert_eq!(new.ansi["blue"], "#2A1F1A");
    assert_eq!(old.truecolor["accent"], "#B7410E");
    assert_eq!(new.truecolor["accent"], "#2A1F1A");
    assert_eq!(new.glyphs["working"].color, "#2A1F1A");
    assert_eq!(new.glyphs["working"].symbol, "w!");
    assert_eq!(new.ansi["bright-blue"], old.ansi["bright-blue"]);
}

#[test]
fn brand_pack_font_stacks_keep_order_duplicates_and_generics() {
    let document = edited(|pack| {
        pack["fonts"]["display"]["family"] = "Plain Face".into();
        pack["fonts"]["display"]["fallbacks"] = json!([
            "Serif",
            "sans-serif",
            "Mono Face",
            "Mono Face",
            "serif",
            "",
            " lead ",
            "Ünï Çode",
            "SANS-SERIF"
        ]);
        pack["fonts"]["body"]["family"] = "serif".into();
        pack["fonts"]["body"]["fallbacks"] = json!([]);
        pack["fonts"]["mono"]["fallbacks"] = json!([]);
    })
    .to_string();
    let files = Mem::new(&[(FORGE_PATH, &document)]);
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let css = presentation.css_properties();
    let display = r#""Plain Face", Serif, sans-serif, "Mono Face", "Mono Face", serif, "", " lead ", "Ünï Çode", SANS-SERIF"#;
    assert_eq!(css["--maestro-font-display"], display);
    assert_eq!(css["--maestro-type-display-font"], display);
    assert_eq!(css["--maestro-font-body"], "serif");
    assert_eq!(css["--maestro-font-mono"], r#""JetBrains Mono""#);
    let font = &presentation.fonts["display"];
    assert_eq!(font.license, "OFL-1.1");
    assert!(font.license_url.ends_with("/barlowcondensed/OFL.txt"));
    assert_eq!(font.fallbacks.len(), 9);
    assert_eq!(*files.reads.borrow(), [FORGE_PATH]);
}

#[test]
fn brand_pack_css_escapes_literal_names() {
    let pack = load_value(&edited(|pack| {
        pack["fonts"]["body"]["family"] = "A\"B\\C\n\u{1}\u{7f}<x>;}{ x: y".into();
        pack["fonts"]["mono"]["family"] = "N\0N".into();
        let spacing = pack["spacing"].as_object_mut().unwrap();
        for key in [
            "note role",
            "note-role",
            "a;b{c}\"\\",
            "x<y",
            "\u{1}z",
            "\0k",
        ] {
            spacing.insert(key.to_owned(), 1.into());
        }
    }))
    .unwrap();
    let css = pack.presentation(BrandMode::Dark).unwrap().css_properties();
    assert_eq!(
        css["--maestro-font-body"],
        r#""A\"B\\C\a \1 \7f \3c x>;}{ x: y", system-ui, sans-serif"#
    );
    assert_eq!(
        css["--maestro-font-mono"],
        "\"N\u{FFFD}N\", ui-monospace, monospace"
    );
    let spacing: Vec<_> = css
        .keys()
        .filter(|k| k.starts_with("--maestro-spacing-"))
        .cloned()
        .collect();
    let expected = [
        "--maestro-spacing-lg",
        "--maestro-spacing-md",
        "--maestro-spacing-note\\ role",
        "--maestro-spacing-note-role",
        "--maestro-spacing-sm",
        "--maestro-spacing-x\\3c y",
        "--maestro-spacing-xl",
        "--maestro-spacing-xs",
        "--maestro-spacing-a\\;b\\{c\\}\\\"\\\\",
        "--maestro-spacing-\\1 z",
        "--maestro-spacing-\u{FFFD}k",
    ];
    let mut expected: Vec<_> = expected.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(spacing, expected);
    assert_eq!(css.len(), 16 + 3 + 20 + 11 + 3);
}

#[test]
fn brand_pack_templates_resolve_all_forms_without_geometry_changes() {
    let files = Mem::forge();
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    for mode in MODES {
        let id = format!("forge-{}", mode_id(mode));
        let expected = presented(&id);
        for (variant, name) in [
            (None, "template"),
            (Some("flat"), "flat"),
            (Some("mono"), "mono"),
            (Some("small"), "small"),
        ] {
            let svg = pack.mark_svg(mode, variant, &files).unwrap();
            assert!(
                !svg.contains("var(--") && !svg.contains("{{") && !svg.contains("currentColor")
            );
            assert!(svg.contains("<title>Maestro"));
            assert_eq!(svg.contains("<rect"), name == "template");
        }
        let text = &expected.colors["text"];
        let source =
            maestro_theme::ThemeOperations::read_to_string(&files, "/forge/mark-mono.svg").unwrap();
        let strokes = source.matches("currentColor").count();
        assert!(strokes > 0);
        assert_eq!(expected.svg["mono"].matches(text.as_str()).count(), strokes);
    }
}

#[test]
fn brand_pack_asset_paths_use_the_selected_pack_directory() {
    let pack_text = edited(|pack| {
        pack["mark"]["template"] = "../marks/m a.svg".into();
        pack["mark"]["variants"] = json!({"flat": "../../other/v.svg"});
    })
    .to_string();
    let path = "/q/ünï dir/packs/x.json";
    let files = Mem::new(&[
        (path, &pack_text),
        ("/q/ünï dir/marks/m a.svg", "<t>{{wordmark}}</t>"),
        ("/q/other/v.svg", "<v>{{wordmark}}</v>"),
        ("/marks/m a.svg", "decoy"),
        ("/q/ünï dir/packs/m a.svg", "decoy"),
        ("/q/ünï dir/v.svg", "decoy"),
    ]);
    let pack = load_brand_pack(path, &files).unwrap();
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, None, &files).unwrap(),
        "<t>Maestro</t>"
    );
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, Some("flat"), &files)
            .unwrap(),
        "<v>Maestro</v>"
    );
    assert_eq!(
        *files.reads.borrow(),
        [path, "/q/ünï dir/marks/m a.svg", "/q/other/v.svg"]
    );
    let scratch = Scratch::new("assets");
    scratch.write("a b/ünï/packs/x.json", &pack_text);
    scratch.write("a b/ünï/marks/m a.svg", "<n>{{wordmark}}</n>");
    scratch.write("a b/marks/m a.svg", "decoy");
    let native = load_brand_pack(
        &scratch.path("a b/ünï/packs/x.json"),
        &NativeThemeOperations,
    )
    .unwrap();
    assert_eq!(
        native
            .mark_svg(BrandMode::Light, None, &NativeThemeOperations)
            .unwrap(),
        "<n>Maestro</n>"
    );
}

#[test]
fn brand_pack_reads_only_the_requested_mark_asset() {
    let files = Mem::new(&[
        (FORGE_PATH, &forge_json().to_string()),
        ("/forge/mark-flat.svg", "<f/>"),
    ]);
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    assert_eq!(*files.reads.borrow(), [FORGE_PATH]);
    let presentation = pack.presentation(BrandMode::Dark).unwrap();
    let _ = presentation.css_properties();
    pack.theme_json(BrandMode::Light).unwrap();
    assert_eq!(files.reads.borrow().len(), 1);
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, Some("flat"), &files)
            .unwrap(),
        "<f/>"
    );
    assert_eq!(*files.reads.borrow(), [FORGE_PATH, "/forge/mark-flat.svg"]);
    let error = pack
        .mark_svg(BrandMode::Dark, Some("mono"), &files)
        .err()
        .unwrap();
    let cause = error
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(error.to_string(), cause.to_string());
}

/// Pointer and admitted-shape marker of each pack field read by the typed tables.
#[cfg(test)]
const FIELDS: [(&str, char); 35] = [
    ("/name", 's'),
    ("/wordmark", 's'),
    ("/tagline", 's'),
    ("/palette", 'o'),
    ("/modes", 'o'),
    ("/modes/dark", 'o'),
    ("/modes/dark/colors", 'o'),
    ("/fonts", 'o'),
    ("/fonts/display", 'o'),
    ("/fonts/display/family", 's'),
    ("/fonts/display/license", 's'),
    ("/fonts/display/license-url", 's'),
    ("/fonts/display/fallbacks", 'a'),
    ("/type", 'o'),
    ("/type/body", 'o'),
    ("/type/body/font", 's'),
    ("/type/body/size", 'n'),
    ("/type/body/line-height", 'n'),
    ("/type/body/weight", 'n'),
    ("/spacing", 'o'),
    ("/radii", 'o'),
    ("/mark", 'o'),
    ("/mark/template", 's'),
    ("/mark/variants", 'o'),
    ("/mark/minimum-size", 'n'),
    ("/mark/small-minimum-size", 'n'),
    ("/mark/clear-space", 'n'),
    ("/mark/app-icon-scale", 'n'),
    ("/glyphs", 'o'),
    ("/glyphs/mark", 'o'),
    ("/glyphs/mark/symbol", 's'),
    ("/glyphs/mark/color", 's'),
    ("/terminal", 'o'),
    ("/terminal/ansi", 'o'),
    ("/terminal/truecolor", 'o'),
];

/// Map entries whose absence is a consumer error rather than a decoding error.
#[cfg(test)]
const MAP_MEMBERS: [&str; 4] = [
    "/modes/dark",
    "/fonts/display",
    "/type/body",
    "/glyphs/mark",
];

#[test]
fn brand_pack_fields_reject_missing_null_and_wrong_shapes() {
    let label = format!("Invalid brand pack {FORGE_PATH}: ");
    let mut cases = 0;
    for (pointer, shape) in FIELDS {
        let wrong: Vec<Value> = match shape {
            's' => vec![json!(7), json!({}), json!([]), json!(true)],
            'n' => vec![json!("16"), json!({}), json!([]), json!(true)],
            'a' => vec![json!("x"), json!(7), json!({})],
            _ => vec![json!([]), json!("x"), json!(7)],
        };
        let mut attempts = vec![edited(|v| *v.pointer_mut(pointer).unwrap() = Value::Null)];
        attempts.extend(
            wrong
                .into_iter()
                .map(|bad| edited(|v| *v.pointer_mut(pointer).unwrap() = bad)),
        );
        if !MAP_MEMBERS.contains(&pointer) {
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            let mut missing = forge_json();
            let object = match parent {
                "" => missing.as_object_mut(),
                parent => missing.pointer_mut(parent).unwrap().as_object_mut(),
            };
            assert!(object.unwrap().remove(key).is_some(), "{pointer}");
            attempts.push(missing);
        }
        for attempt in attempts {
            let result = load_value(&attempt);
            assert!(result.is_err(), "{pointer}: {attempt}");
            let message = failure(result);
            assert!(message.starts_with(&label), "{pointer}: {message}");
            cases += 1;
        }
    }
    assert_eq!(cases, 188);
    let positional = [
        ("/fonts/display", json!(["F", "OFL-1.1", "u", []])),
        ("/type/body", json!(["body", 16, 1.5, 400])),
        ("/mark", json!(["m.svg", {}, 32, 16, 0.1, 0.7])),
        ("/glyphs/mark", json!(["*", "accent"])),
        ("/terminal", json!([{}, {}])),
        ("/modes/dark", json!([{}])),
    ];
    for (pointer, record) in positional {
        let message = failure(load_value(&edited(|v| {
            *v.pointer_mut(pointer).unwrap() = record;
        })));
        assert!(message.starts_with(&label), "{pointer}");
    }
    assert!(failure(load_value(&json!([]))).starts_with(&label));
}

#[test]
fn brand_pack_fields_keep_last_duplicates_and_ignore_extensions() {
    let compact = forge_json().to_string();
    let load_text = |text: &str| load_brand_pack(FORGE_PATH, &Mem::new(&[(FORGE_PATH, text)]));
    let swap = |from: &str, to: &str| {
        assert!(compact.contains(from), "{from}");
        compact.replacen(from, to, 1)
    };
    let base = forge();
    let dark = |pack: &BrandPack| pack.presentation(BrandMode::Dark).unwrap().css_properties();
    let later_wins = load_text(&swap(r#""name":"Forge""#, r#""name":7,"name":"Later""#)).unwrap();
    assert_eq!(
        later_wins.presentation(BrandMode::Dark).unwrap().name,
        "Later"
    );
    let nested = load_text(&swap(r#""size":56"#, r#""size":"bad","size":56"#)).unwrap();
    assert_eq!(dark(&nested), dark(&base));
    let later_invalid = swap(r#""name":"Forge""#, r#""name":"Earlier","name":7"#);
    assert!(failure(load_text(&later_invalid)).starts_with("Invalid brand pack"));
    let extended = edited(|pack| {
        pack["extension"] = json!({"deep": [[[1]]]});
        pack["fonts"]["display"]["extra"] = Value::Null;
        pack["glyphs"]["mark"]["note"] = json!([]);
    });
    assert_eq!(dark(&load_value(&extended).unwrap()), dark(&base));
    let escaped = load_text(&swap(r#""forge-black":"#, r#""forge\u002dblack":"#)).unwrap();
    assert_eq!(dark(&escaped), dark(&base));
    let distinct = load_value(&edited(|pack| {
        pack["spacing"]["\u{e9}"] = 1.into();
        pack["spacing"]["e\u{301}"] = 2.into();
    }))
    .unwrap();
    let css = dark(&distinct);
    assert_eq!(css["--maestro-spacing-\u{e9}"], "1px");
    assert_eq!(css["--maestro-spacing-e\u{301}"], "2px");
}

#[test]
fn brand_pack_invalid_color_and_font_references_fail_explicitly() {
    let cases = [
        ("/modes/dark", None, "Missing brand mode: dark"),
        (
            "/modes/dark/colors/text",
            Some(json!("nope")),
            "Missing brand palette color: nope",
        ),
        (
            "/glyphs/mark/color",
            Some(json!("ghost")),
            "Missing brand color role: ghost",
        ),
        (
            "/terminal/ansi/red",
            Some(json!("ghost")),
            "Missing brand color role: ghost",
        ),
        (
            "/terminal/truecolor/text",
            Some(json!("ghost")),
            "Missing brand color role: ghost",
        ),
        (
            "/type/body/font",
            Some(json!("ghost")),
            "Missing brand font: ghost",
        ),
        (
            "/modes/dark/colors/muted",
            None,
            "Missing brand color role: muted",
        ),
    ];
    for (pointer, value, message) in cases {
        let pack = load_value(&changed(pointer, value)).unwrap();
        let mode = BrandMode::Dark;
        assert_eq!(failure(pack.presentation(mode)), message);
        assert_eq!(failure(pack.theme_json(mode)), message);
    }
    for bad in [
        "", "#12345", "#1234567", "#1g0000", "red", "0E0B09", "#0E0B0é",
    ] {
        let pack = load_value(&edited(|p| p["palette"]["bone"] = bad.into())).unwrap();
        assert_eq!(
            failure(pack.presentation(BrandMode::Dark)),
            format!("Invalid brand color: {bad}")
        );
    }
    let no_light = load_value(&edited(|p| {
        p["modes"].as_object_mut().unwrap().remove("light");
    }))
    .unwrap();
    assert!(no_light.presentation(BrandMode::Dark).is_ok());
    assert_eq!(
        failure(no_light.presentation(BrandMode::Light)),
        "Missing brand mode: light"
    );
}

#[test]
fn brand_pack_measurements_keep_values_units_and_constraints() {
    for pointer in [
        "/type/body/size",
        "/spacing/md",
        "/radii/control",
        "/spacing/a~1b~0c",
    ] {
        for bad in [-1.0, -0.5] {
            let pack = changed(pointer, Some(json!(bad)));
            assert_eq!(
                failure(load_value(&pack)),
                format!("Invalid brand measurement: {pointer}")
            );
        }
    }
    let zero = changed("/type/body/size", Some(json!(0)));
    assert_eq!(
        failure(load_value(&zero)),
        "Invalid brand measurement: /type/body/size"
    );
    let pack = load_value(&edited(|p| {
        p["type"]["body"] =
            json!({"font": "body", "size": 15.5, "line-height": 1.25, "weight": 450.5});
        p["spacing"]["md"] = 3.5.into();
        p["radii"]["none"] = 0.into();
        p["radii"]["control"] = 0.25.into();
        p["mark"]["clear-space"] = 0.054_687_5.into();
    }))
    .unwrap();
    let css = pack.presentation(BrandMode::Dark).unwrap().css_properties();
    assert_eq!(css["--maestro-type-body-size"], "15.5px");
    assert_eq!(css["--maestro-type-body-line-height"], "1.25");
    assert_eq!(css["--maestro-type-body-weight"], "450.5");
    assert_eq!(css["--maestro-spacing-md"], "3.5px");
    assert_eq!(css["--maestro-radii-none"], "0px");
    assert_eq!(css["--maestro-radii-control"], "0.25px");
    let clear_space = pack.presentation(BrandMode::Dark).unwrap().mark.clear_space;
    assert!((clear_space - 0.054_687_5).abs() < f64::EPSILON);
}

#[test]
fn brand_pack_io_and_decode_failures_keep_their_phase() {
    let missing = Mem::new(&[]);
    let error = load_brand_pack(FORGE_PATH, &missing).err().unwrap();
    assert!(!error.to_string().contains("Invalid brand pack"));
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(*missing.reads.borrow(), [FORGE_PATH]);
    let malformed = Mem::new(&[(FORGE_PATH, "{")]);
    let error = load_brand_pack(FORGE_PATH, &malformed).err().unwrap();
    assert!(
        error
            .to_string()
            .starts_with(&format!("Invalid brand pack {FORGE_PATH}: "))
    );
    assert!(error.source().unwrap().is::<serde_json::Error>());
    let shape = Mem::new(&[(FORGE_PATH, r#"{"name": 1}"#)]);
    assert!(
        load_brand_pack(FORGE_PATH, &shape)
            .err()
            .unwrap()
            .source()
            .unwrap()
            .is::<serde_json::Error>()
    );
    assert_eq!(*shape.reads.borrow(), [FORGE_PATH]);
    let invalid = edited(|p| p["type"]["body"]["size"] = 0.into()).to_string();
    let files = Mem::new(&[(FORGE_PATH, &invalid)]);
    assert_eq!(
        failure(load_brand_pack(FORGE_PATH, &files)),
        "Invalid brand measurement: /type/body/size"
    );
    assert_eq!(files.reads.borrow().len(), 1);
    let alias = edited(|p| p["glyphs"]["mark"]["color"] = "ghost".into()).to_string();
    let files = Mem::new(&[(FORGE_PATH, &alias), ("/forge/mark.svg", "<x/>")]);
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    assert_eq!(
        failure(pack.mark_svg(BrandMode::Dark, None, &files)),
        "Missing brand color role: ghost"
    );
    assert_eq!(files.reads.borrow().len(), 1);
}

#[test]
fn brand_pack_unknown_variants_and_template_roles_fail_explicitly() {
    let files = Mem::forge()
        .with("/forge/mark-flat.svg", "<a>var(--ghost)</a>")
        .with("/forge/mark-mono.svg", "<a>var(--mark-ring</a>")
        .with(
            "/forge/mark-small.svg",
            "<a>var(--text) var(--text) {{wordmark}}{{wordmark}} vcc</a>",
        );
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    let reads = files.reads.borrow().len();
    assert_eq!(
        failure(pack.mark_svg(BrandMode::Dark, Some("nope"), &files)),
        "Unknown brand mark variant: nope"
    );
    assert_eq!(files.reads.borrow().len(), reads);
    assert_eq!(
        failure(pack.mark_svg(BrandMode::Dark, Some("flat"), &files)),
        "Missing brand color role: ghost"
    );
    assert_eq!(files.reads.borrow().len(), reads + 1);
    assert_eq!(
        failure(pack.mark_svg(BrandMode::Dark, Some("mono"), &files)),
        "Unterminated brand template color"
    );
    assert_eq!(files.reads.borrow().len(), reads + 2);
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, Some("small"), &files)
            .unwrap(),
        "<a>#F2E8DC #F2E8DC MaestroMaestro vcc</a>"
    );
}

#[test]
fn brand_pack_derived_themes_match_the_shipped_assets() {
    let pack = load_brand_pack(FORGE_NATIVE, &NativeThemeOperations).unwrap();
    let schema: Value =
        serde_json::from_str(include_str!("../assets/theme/theme-schema.json")).unwrap();
    let shipped = [
        (BrandMode::Dark, include_str!("../assets/theme/dark.json")),
        (BrandMode::Light, include_str!("../assets/theme/light.json")),
    ];
    for (mode, text) in shipped {
        let document = pack.theme_json(mode).unwrap();
        assert_eq!(document, text);
        assert!(jsonschema::draft7::is_valid(
            &schema,
            &serde_json::from_str(&document).unwrap()
        ));
    }
    let edited = load_value(&edited(|p| p["palette"]["bone"] = "#010203".into())).unwrap();
    assert_ne!(edited.theme_json(BrandMode::Dark).unwrap(), shipped[0].1);
}

/// WCAG relative luminance of `#rrggbb`.
#[cfg(test)]
fn luminance(hex: &str) -> f64 {
    let channel = |at: usize| {
        let c = f64::from(u8::from_str_radix(&hex[at..at + 2], 16).unwrap()) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
}

/// Contrast ratio of two `#rrggbb` colors.
#[cfg(test)]
fn ratio(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn brand_pack_contrast_covers_only_the_documented_pairs() {
    let mut pairs = 0;
    for path in [FORGE_NATIVE, SIGNAL_NATIVE] {
        let pack = load_brand_pack(path, &NativeThemeOperations).unwrap();
        for mode in MODES {
            let presentation = pack.presentation(mode).unwrap();
            let colors = &presentation.colors;
            let mut check = |foreground: &str, background: &str, minimum: f64| {
                let found = ratio(colors[foreground], colors[background]);
                assert!(
                    found >= minimum,
                    "{path} {mode:?} {foreground}/{background}: {found}"
                );
                pairs += 1;
            };
            for role in [
                "text", "muted", "accent", "live", "success", "warning", "error",
            ] {
                check(role, "background", 4.5);
                check(role, "panel", 4.5);
            }
            check("on-accent", "accent", 4.5);
            for role in ["mark-ring", "mark-m", "mark-node", "mark-live", "mark-star"] {
                check(role, "background", 3.0);
            }
            check("border", "background", 3.0);
            check("border", "panel", 3.0);
        }
    }
    assert_eq!(pairs, 88);
}

#[test]
fn brand_pack_template_values_are_inserted_once() {
    let wordmark = "A&B <x> \"q\" 'z' $& var(--mark-star) {{wordmark}}";
    let pack_text = edited(|p| p["wordmark"] = wordmark.into()).to_string();
    let files = Mem::new(&[
        (FORGE_PATH, &pack_text),
        ("/forge/mark.svg", "<t>{{wordmark}}</t>|{{wordmark}}|"),
        ("/forge/mark-flat.svg", "{{wordmark}}"),
    ]);
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    let escaped =
        "A&amp;B &lt;x&gt; &quot;q&quot; &apos;z&apos; $&amp; var(--mark-star) {{wordmark}}";
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, None, &files).unwrap(),
        format!("<t>{escaped}</t>|{escaped}|")
    );
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, Some("flat"), &files)
            .unwrap(),
        escaped
    );
    let empty = Mem::new(&[
        (
            FORGE_PATH,
            &edited(|p| p["wordmark"] = "".into()).to_string(),
        ),
        ("/forge/mark.svg", "a{{wordmark}}b{{wordmark}}c"),
    ]);
    let pack = load_brand_pack(FORGE_PATH, &empty).unwrap();
    assert_eq!(
        pack.mark_svg(BrandMode::Light, None, &empty).unwrap(),
        "abc"
    );
}

/// The shared pack with one extra entry in every extensible map.
#[cfg(test)]
fn with_additional_roles() -> String {
    edited(|p| {
        p["palette"]["extra swatch"] = "#112233".into();
        for mode in ["dark", "light"] {
            p["modes"][mode]["colors"]["extra role"] = "extra swatch".into();
            p["modes"][mode]["colors"]["extra-role"] = "bone".into();
        }
        p["fonts"]["caption"] =
            json!({"family": "Cap", "license": "L", "license-url": "u", "fallbacks": ["serif"]});
        p["type"]["caption"] =
            json!({"font": "caption", "size": 12, "line-height": 1.5, "weight": 300});
        p["spacing"]["xxl"] = 48.into();
        p["radii"]["pill"] = 99.into();
        p["glyphs"]["queued"] = json!({"symbol": "q", "color": "extra role"});
        p["terminal"]["ansi"]["extra-slot"] = "extra role".into();
        p["terminal"]["truecolor"]["extra"] = "extra-role".into();
        p["mark"]["variants"]["badge"] = "mark-badge.svg".into();
    })
    .to_string()
}

/// The extra entries of `with_additional_roles` resolve in every output family.
#[cfg(test)]
fn assert_additional_presentation(presentation: &BrandPresentation<'_>) {
    assert_eq!(presentation.colors["extra role"], "#112233");
    assert_eq!(presentation.colors["extra-role"], "#F2E8DC");
    assert_eq!(presentation.glyphs["queued"].color, "#112233");
    assert_eq!(presentation.ansi["extra-slot"], "#112233");
    assert_eq!(presentation.truecolor["extra"], "#F2E8DC");
    let css = presentation.css_properties();
    assert_eq!(css["--maestro-color-extra\\ role"], "#112233");
    assert_eq!(css["--maestro-color-extra-role"], "#F2E8DC");
    assert_eq!(css["--maestro-font-caption"], r#""Cap", serif"#);
    assert_eq!(css["--maestro-type-caption-size"], "12px");
    assert_eq!(css["--maestro-spacing-xxl"], "48px");
    assert_eq!(css["--maestro-radii-pill"], "99px");
}

#[test]
fn brand_pack_additional_named_roles_survive_projection() {
    let pack_text = with_additional_roles();
    let files = Mem::forge().with(FORGE_PATH, &pack_text).with(
        "/forge/mark-badge.svg",
        "<b>var(--extra role)|var(--extra-role)</b>",
    );
    let pack = load_brand_pack(FORGE_PATH, &files).unwrap();
    assert_additional_presentation(&pack.presentation(BrandMode::Dark).unwrap());
    assert_eq!(
        pack.mark_svg(BrandMode::Dark, Some("badge"), &files)
            .unwrap(),
        "<b>#112233|#F2E8DC</b>"
    );
    let document: Value = serde_json::from_str(&pack.theme_json(BrandMode::Dark).unwrap()).unwrap();
    assert_eq!(document["colors"].as_object().unwrap().len(), 51);
    assert_eq!(document["vars"]["extra role"], "#112233");
    assert_eq!(document["vars"]["extra-role"], "#F2E8DC");
}

#[test]
fn brand_pack_dictionary_values_reject_null_and_wrong_types() {
    let label = format!("Invalid brand pack {FORGE_PATH}: ");
    let text = [
        "/palette/bone",
        "/modes/dark/colors/text",
        "/mark/variants/flat",
        "/terminal/ansi/red",
        "/terminal/truecolor/accent",
        "/fonts/display/fallbacks/0",
    ];
    let number = ["/spacing/md", "/radii/control"];
    let bad_text = [Value::Null, json!(7), json!({}), json!([]), json!(true)];
    let bad_number = [Value::Null, json!("8"), json!({}), json!([]), json!(true)];
    for (pointers, bad) in [(&text[..], &bad_text), (&number[..], &bad_number)] {
        for pointer in pointers {
            for value in bad {
                let pack = edited(|v| *v.pointer_mut(pointer).unwrap() = value.clone());
                let message = failure(load_value(&pack));
                assert!(message.starts_with(&label), "{pointer} {value}: {message}");
            }
        }
    }
}

#[test]
fn brand_pack_presentation_keeps_tagline_and_mark_measurements() {
    let forge = forge();
    let presentation = forge.presentation(BrandMode::Dark).unwrap();
    assert_eq!(presentation.tagline, "Conduct every model.");
    let mark = presentation.mark;
    assert_eq!(
        (
            mark.minimum_size,
            mark.small_minimum_size,
            mark.app_icon_scale
        ),
        (32.0, 16.0, 0.72)
    );
    let alternate = alternate();
    let presentation = alternate.presentation(BrandMode::Light).unwrap();
    assert_eq!(presentation.tagline, "Next <turn> & \"quoted\"");
    let mark = presentation.mark;
    assert_eq!(
        (
            mark.minimum_size,
            mark.small_minimum_size,
            mark.clear_space,
            mark.app_icon_scale
        ),
        (40.0, 20.0, 0.125, 0.8)
    );
}
