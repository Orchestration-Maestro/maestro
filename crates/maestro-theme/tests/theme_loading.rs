//! Public theme file loading, admission and alias resolution.
#[cfg(test)]
mod support;

use maestro_theme::{
    ColorMode, NativeThemeOperations, ThemeColor, ThemeOperations, load_theme_from_path,
};
use serde_json::{Value, json};
use std::io::ErrorKind;
use support::{Controlled, dark, fg, load, required_colors};

#[cfg(test)]
/// The background prefix stored under a literal key.
fn bg(theme: &maestro_theme::Theme, name: &str) -> String {
    theme
        .get_bg_ansi(&maestro_theme::ThemeBg::Named(name.into()))
        .unwrap()
        .to_owned()
}

/// A controlled operation whose file is missing.
fn missing() -> Controlled {
    let mut operations = Controlled::new("", &[]);
    operations.content = None;
    operations
}

/// Serialize a theme document and load it with an explicit truecolor mode.
fn load_doc(doc: &Value) -> Result<maestro_theme::Theme, maestro_theme::ThemeError> {
    load(&doc.to_string(), Some(ColorMode::Truecolor))
}

#[cfg(test)]
/// The error text of a failed load.
fn failure(text: &str) -> String {
    load(text, Some(ColorMode::Truecolor))
        .unwrap_err()
        .to_string()
}

/// Dark theme with every color set to `value` and the given variables.
fn uniform(value: &Value, vars: Value) -> Value {
    let mut doc = dark();
    doc["vars"] = vars;
    for name in required_colors() {
        doc["colors"][name] = value.clone();
    }
    doc
}

#[test]
fn theme_published_schema_keeps_all_required_tokens() {
    let schema: Value =
        serde_json::from_str(include_str!("../assets/theme/theme-schema.json")).unwrap();
    let runtime: Value =
        serde_json::from_str(include_str!("../assets/theme/runtime-schema.json")).unwrap();
    let required = required_colors();
    assert_eq!(required.len(), 51);
    let backgrounds: Vec<_> = required
        .iter()
        .filter(|name| name.ends_with("Bg"))
        .collect();
    assert_eq!(
        backgrounds,
        [
            "selectedBg",
            "userMessageBg",
            "customMessageBg",
            "toolPendingBg",
            "toolSuccessBg",
            "toolErrorBg"
        ]
    );
    let validator = jsonschema::validator_for(&schema).unwrap();
    let valid = |doc: &Value| validator.is_valid(doc);
    assert!(valid(&dark()));
    let mutated = |path: &[&str], value: Option<Value>| {
        let mut doc = dark();
        set(&mut doc, path, value);
        doc
    };
    let accepted = [
        mutated(&["colors", "accent"], Some(json!(0))),
        mutated(&["colors", "accent"], Some(json!(255))),
        mutated(&["colors", "accent"], Some(json!(""))),
        mutated(&["export", "pageBg"], Some(json!("#101010"))),
    ];
    for doc in &accepted {
        assert!(valid(doc), "{doc}");
    }
    let rejected = [
        mutated(&["colors", "accent"], None),
        mutated(&["colors", "accent"], Some(json!(true))),
        mutated(&["colors", "accent"], Some(json!(1.5))),
        mutated(&["colors", "accent"], Some(json!(256))),
        mutated(&["colors", "accent"], Some(json!(-1))),
        mutated(&["name"], None),
        mutated(&["extra"], Some(json!(1))),
        mutated(&["colors", "extra"], Some(json!("#000000"))),
        mutated(&["export", "extra"], Some(json!("#000000"))),
        mutated(&["export", "pageBg"], Some(json!(1.5))),
        mutated(&["vars", "extra"], Some(json!(1.5))),
    ];
    for doc in &rejected {
        assert!(!valid(doc), "{doc}");
    }
    assert!(jsonschema::is_valid(
        &runtime,
        &mutated(&["colors", "extra"], Some(json!("#000000")))
    ));
}

/// The stored prefix of `name` on its own plane.
fn prefix_of(theme: &maestro_theme::Theme, name: &str) -> String {
    if name.ends_with("Bg") {
        bg(theme, name)
    } else {
        fg(theme, name)
    }
}

/// Independently computed prefixes for every shipped token, per asset and mode.
const SHIPPED_PREFIXES: &str = include_str!("support/shipped_prefixes.json");

#[test]
fn theme_loads_shipped_theme_data() {
    let expected: Value = serde_json::from_str(SHIPPED_PREFIXES).unwrap();
    for file in ["dark", "light"] {
        let path = format!("{}/assets/theme/{file}.json", env!("CARGO_MANIFEST_DIR"));
        for (mode, key) in [
            (ColorMode::Truecolor, "truecolor"),
            (ColorMode::Color256, "color256"),
        ] {
            let theme = load_theme_from_path(&path, Some(mode), &NativeThemeOperations).unwrap();
            assert_eq!(theme.name(), Some(file));
            let table = expected[file][key].as_object().unwrap();
            assert_eq!(table.len(), 51);
            for (name, prefix) in table {
                let actual = prefix_of(&theme, name);
                assert_eq!(actual, prefix.as_str().unwrap(), "{file} {key} {name}");
            }
        }
    }
}

/// Authored color literals with the expected foreground and background output.
#[cfg(test)]
fn authored_color_cases() -> Vec<(Value, &'static str, &'static str)> {
    vec![
        (json!(""), "\x1b[39mtext\x1b[39m", "\x1b[49mtext\x1b[49m"),
        (
            json!(0),
            "\x1b[38;5;0mtext\x1b[39m",
            "\x1b[48;5;0mtext\x1b[49m",
        ),
        (
            json!(1),
            "\x1b[38;5;1mtext\x1b[39m",
            "\x1b[48;5;1mtext\x1b[49m",
        ),
        (
            json!(15),
            "\x1b[38;5;15mtext\x1b[39m",
            "\x1b[48;5;15mtext\x1b[49m",
        ),
        (
            json!(16),
            "\x1b[38;5;16mtext\x1b[39m",
            "\x1b[48;5;16mtext\x1b[49m",
        ),
        (
            json!(231),
            "\x1b[38;5;231mtext\x1b[39m",
            "\x1b[48;5;231mtext\x1b[49m",
        ),
        (
            json!(232),
            "\x1b[38;5;232mtext\x1b[39m",
            "\x1b[48;5;232mtext\x1b[49m",
        ),
        (
            json!(255),
            "\x1b[38;5;255mtext\x1b[39m",
            "\x1b[48;5;255mtext\x1b[49m",
        ),
        (
            json!("#000000"),
            "\x1b[38;2;0;0;0mtext\x1b[39m",
            "\x1b[48;2;0;0;0mtext\x1b[49m",
        ),
        (
            json!("#FFFFFF"),
            "\x1b[38;2;255;255;255mtext\x1b[39m",
            "\x1b[48;2;255;255;255mtext\x1b[49m",
        ),
        (
            json!("#aB09fF"),
            "\x1b[38;2;171;9;255mtext\x1b[39m",
            "\x1b[48;2;171;9;255mtext\x1b[49m",
        ),
        (
            json!("root"),
            "\x1b[38;5;24mtext\x1b[39m",
            "\x1b[48;5;24mtext\x1b[49m",
        ),
    ]
}

#[test]
fn theme_accepts_authored_color_forms() {
    for (value, foreground, background) in authored_color_cases() {
        let theme = load_doc(&uniform(&value, json!({ "root": 24 }))).unwrap();
        assert_eq!(
            theme.fg(&ThemeColor::Accent, "text").unwrap(),
            foreground,
            "{value}"
        );
        assert_eq!(
            theme
                .bg(&maestro_theme::ThemeBg::SelectedBg, "text")
                .unwrap(),
            background,
            "{value}"
        );
    }
}

#[test]
fn theme_accepts_integral_json_number_spellings() {
    for (spelling, expected) in [
        ("0.0", 0),
        ("1.0", 1),
        ("255.0", 255),
        ("1e2", 100),
        ("-0", 0),
    ] {
        let text = uniform(&json!("@@"), json!({}))
            .to_string()
            .replace("\"@@\"", spelling);
        let theme = load(&text, Some(ColorMode::Truecolor)).unwrap();
        assert_eq!(
            fg(&theme, "accent"),
            format!("\x1b[38;5;{expected}m"),
            "{spelling}"
        );
    }
}

#[cfg(test)]
/// Replace one member of a JSON document, or remove it when `value` is `None`.
fn set(doc: &mut Value, path: &[&str], value: Option<Value>) {
    let (last, parents) = path.split_last().unwrap();
    let parent = parents.iter().fold(&mut *doc, |node, key| &mut node[*key]);
    match value {
        Some(value) => parent[*last] = value,
        None => {
            parent.as_object_mut().unwrap().remove(*last);
        }
    }
}

/// What a selected field accepts.
#[derive(Clone, Copy, PartialEq)]
enum Domain {
    Text,
    Color,
    Object,
    Nothing,
}

/// Every value kind with its literal and the domain that admits it.
const KINDS: [(&str, &str, Domain); 10] = [
    ("null", "null", Domain::Nothing),
    ("string", "\"x\"", Domain::Text),
    ("empty", "\"\"", Domain::Text),
    ("integer", "7", Domain::Color),
    ("fraction", "1.5", Domain::Nothing),
    ("negative", "-1", Domain::Nothing),
    ("overrange", "256", Domain::Nothing),
    ("bool", "true", Domain::Nothing),
    ("array", "[]", Domain::Nothing),
    ("object", "{}", Domain::Object),
];

/// Selected fields: path, accepted domain and whether the member may be absent.
const FIELDS: [(&[&str], Domain, bool); 10] = [
    (&["name"], Domain::Text, false),
    (&["$schema"], Domain::Text, true),
    (&["vars"], Domain::Object, true),
    (&["colors"], Domain::Nothing, false),
    (&["colors", "accent"], Domain::Color, false),
    (&["vars", "cyan"], Domain::Color, true),
    (&["export"], Domain::Object, true),
    (&["export", "pageBg"], Domain::Color, true),
    (&["export", "cardBg"], Domain::Color, true),
    (&["export", "infoBg"], Domain::Color, true),
];

#[test]
fn theme_validates_selected_field_domains() {
    for (path, domain, optional) in FIELDS {
        let mut base = dark();
        base["export"] = json!({});
        let mut cases = vec![("missing", None, optional)];
        for (kind, literal, kind_domain) in KINDS {
            let admitted = matches!(
                (domain, kind_domain),
                (Domain::Color, Domain::Text | Domain::Color)
                    | (Domain::Text, Domain::Text)
                    | (Domain::Object, Domain::Object)
            );
            cases.push((kind, Some(serde_json::from_str(literal).unwrap()), admitted));
        }
        for (kind, value, admitted) in cases {
            let mut doc = base.clone();
            set(&mut doc, path, value);
            let message = load_doc(&doc)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert_eq!(
                !message.starts_with("Invalid theme"),
                admitted,
                "{path:?} {kind}: {message}"
            );
        }
    }
    for literal in ["null", "\"x\"", "\"\"", "7", "true", "[]", "{}"] {
        assert!(
            failure(literal).starts_with("Invalid theme \"fixture\":"),
            "root {literal}"
        );
    }
    assert!(failure("").starts_with("Failed to parse theme fixture:"));
}

/// The complete message for a set of missing color tokens.
fn missing_message(names: &[&str]) -> String {
    let bullets: Vec<_> = names.iter().map(|name| format!("  - {name}")).collect();
    format!(
        "Invalid theme \"fixture\":\n\nMissing required color tokens:\n{}\n\nPlease add these colors to your theme's \"colors\" object.\nSee the built-in themes (dark.json, light.json) for reference values.",
        bullets.join("\n")
    )
}

#[test]
fn theme_groups_missing_tokens_in_sorted_order() {
    for name in required_colors() {
        let mut doc = dark();
        set(&mut doc, &["colors", &name], None);
        assert_eq!(failure(&doc.to_string()), missing_message(&[&name]));
    }
    let mut doc = dark();
    for name in ["warning", "accent", "border"] {
        set(&mut doc, &["colors", name], None);
    }
    assert_eq!(
        failure(&doc.to_string()),
        missing_message(&["accent", "border", "warning"])
    );
    doc["colors"] = json!({});
    let mut all = required_colors();
    all.sort();
    let all: Vec<_> = all.iter().map(String::as_str).collect();
    assert_eq!(failure(&doc.to_string()), missing_message(&all));
}

#[test]
fn theme_keeps_missing_and_other_error_groups() {
    let mut doc = dark();
    set(&mut doc, &["colors", "accent"], None);
    set(&mut doc, &["colors", "warning"], None);
    doc["name"] = Value::Null;
    doc["colors"]["border"] = json!(true);
    let message = failure(&doc.to_string());
    let missing = message
        .find("Missing required color tokens:\n  - accent\n  - warning\n")
        .unwrap();
    let other = message.find("\n\nOther errors:\n").unwrap();
    assert!(missing < other);
    assert!(message.starts_with("Invalid theme \"fixture\":\n\nMissing"));
    assert!(message[other..].contains("\n  - /name: "));
    assert!(message[other..].contains("\n  - /colors/border: "));
}

#[test]
fn theme_accepts_runtime_extensions_without_published_schema_restrictions() {
    let mut root = dark();
    root["extra"] = json!({ "nested": ["retained", true] });
    assert_eq!(
        fg(&load_doc(&root).unwrap(), "accent"),
        "\x1b[38;2;138;190;183m"
    );
    let mut colors = dark();
    colors["colors"]["extraColor"] = json!("#010203");
    assert_eq!(
        fg(&load_doc(&colors).unwrap(), "extraColor"),
        "\x1b[38;2;1;2;3m"
    );
    let mut export = dark();
    export["export"] = json!({ "any": true });
    assert_eq!(
        fg(&load_doc(&export).unwrap(), "accent"),
        "\x1b[38;2;138;190;183m"
    );
}

#[cfg(test)]
/// Every plane prefix of a theme in which all colors are `value`.
fn uniform_prefixes(vars: Value, value: &str) -> Vec<String> {
    let theme = load_doc(&uniform(&json!(value), vars)).unwrap();
    required_colors()
        .iter()
        .map(|name| {
            if name.ends_with("Bg") {
                bg(&theme, name)
            } else {
                fg(&theme, name)
            }
        })
        .collect()
}

#[test]
fn theme_resolves_aliases_per_color_without_false_cycles() {
    let chain = uniform_prefixes(json!({ "a": "b", "b": "c", "c": "#123456" }), "a");
    assert!(chain.iter().all(|prefix| prefix.ends_with("2;18;52;86m")));
    assert!(
        chain.contains(&"\x1b[48;2;18;52;86m".to_owned())
            && chain.contains(&"\x1b[38;2;18;52;86m".to_owned())
    );
    let indexed = uniform_prefixes(json!({ "a": "b", "b": 4 }), "a");
    assert!(indexed.iter().all(|prefix| prefix.ends_with(";5;4m")));
    let default = uniform_prefixes(json!({ "a": "" }), "a");
    assert!(
        default
            .iter()
            .all(|prefix| prefix == "\x1b[39m" || prefix == "\x1b[49m")
    );
    let unused = uniform_prefixes(json!({ "a": "#ff0000", "unused": "unused" }), "a");
    assert!(unused.iter().all(|prefix| prefix.ends_with("2;255;0;0m")));
    let spaced = uniform_prefixes(json!({ "two words": "#010203" }), "two words");
    assert!(spaced.iter().all(|prefix| prefix.ends_with("2;1;2;3m")));
    let untrimmed = uniform_prefixes(
        json!({ " anchor ": "#123456", "anchor": "#abcdef" }),
        " anchor ",
    );
    assert!(
        untrimmed
            .iter()
            .all(|prefix| prefix.ends_with("2;18;52;86m"))
    );
    let text = uniform(
        &json!("alias"),
        json!({ "alias": "#010203", "Alias": "#abcdef" }),
    )
    .to_string()
    .replacen("\"alias\":", "\"al\\u0069as\":", 1);
    let escaped = load(&text, Some(ColorMode::Truecolor)).unwrap();
    assert_eq!(fg(&escaped, "accent"), "\x1b[38;2;1;2;3m");
}

#[test]
fn theme_reports_missing_and_cyclic_variables() {
    let cases = [
        (
            json!({}),
            "missing",
            "Variable reference not found: missing",
        ),
        (json!({ "a": "b" }), "a", "Variable reference not found: b"),
        (
            json!({ "a": "a" }),
            "a",
            "Circular variable reference detected: a",
        ),
        (
            json!({ "a": "b", "b": "a" }),
            "a",
            "Circular variable reference detected: a",
        ),
        (
            json!({ "a": "b", "b": "c", "c": "b" }),
            "a",
            "Circular variable reference detected: b",
        ),
    ];
    for (vars, value, message) in cases {
        let error = load_doc(&uniform(&json!(value), vars)).unwrap_err();
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn theme_resolves_every_alias_before_ansi_conversion() {
    let mut doc = dark();
    doc["colors"]["accent"] = json!("#gg0000");
    doc["colors"]["toolOutput"] = json!("absent");
    assert_eq!(
        failure(&doc.to_string()),
        "Variable reference not found: absent"
    );
}

#[test]
fn maestro_theme_rejects_partial_hex_channels() {
    for value in ["#1g2g3g", "#fZ00aa", "#+10000", "#-10000", "# 10000"] {
        let mut doc = dark();
        doc["colors"]["accent"] = json!(value);
        assert_eq!(
            failure(&doc.to_string()),
            format!("Invalid hex color: {value}")
        );
    }
}

#[test]
fn theme_rejects_invalid_hex_and_unresolved_names() {
    let cases = [
        ("#", "Invalid hex color: #"),
        ("##123456", "Invalid hex color: ##123456"),
        ("#12345", "Invalid hex color: #12345"),
        ("#1234567", "Invalid hex color: #1234567"),
        ("#gg0000", "Invalid hex color: #gg0000"),
        ("#00gg00", "Invalid hex color: #00gg00"),
        ("#0000gg", "Invalid hex color: #0000gg"),
        ("#\u{e9}\u{e9}0000", "Invalid hex color: #\u{e9}\u{e9}0000"),
        (" #112233", "Variable reference not found:  #112233"),
        ("#112233 ", "Invalid hex color: #112233 "),
    ];
    for (value, message) in cases {
        let mut doc = dark();
        doc["colors"]["accent"] = json!(value);
        assert_eq!(failure(&doc.to_string()), message);
    }
}

#[test]
fn theme_last_duplicate_members_win() {
    let text = dark().to_string().replacen(
        "\"accent\":\"accent\"",
        "\"accent\":null,\"accent\":\"#010203\"",
        1,
    );
    assert!(text.contains("\"accent\":null,"));
    let theme = load(&text, Some(ColorMode::Truecolor)).unwrap();
    assert_eq!(fg(&theme, "accent"), "\x1b[38;2;1;2;3m");
}

#[test]
fn theme_retains_mode_and_authored_metadata() {
    let text = dark().to_string();
    let operations = Controlled::new(&text, &[("TERM", "xterm")]);
    let theme = load_theme_from_path(
        "spelled/../name.json",
        Some(ColorMode::Truecolor),
        &operations,
    )
    .unwrap();
    assert_eq!(theme.name(), Some("dark"));
    assert_eq!(theme.source_path(), Some("spelled/../name.json"));
    assert_eq!(theme.get_color_mode(), ColorMode::Truecolor);
    let implicit = load_theme_from_path(
        "p",
        None,
        &Controlled::new(&text, &[("COLORTERM", "truecolor")]),
    )
    .unwrap();
    assert_eq!(implicit.get_color_mode(), ColorMode::Truecolor);
    let limited =
        load_theme_from_path("p", None, &Controlled::new(&text, &[("TERM", "linux")])).unwrap();
    assert_eq!(limited.get_color_mode(), ColorMode::Color256);
    assert_eq!(fg(&limited, "accent"), "\x1b[38;5;109m");
    let bare = maestro_theme::Theme::new(
        [],
        [],
        ColorMode::Color256,
        maestro_theme::ThemeOptions::default(),
    )
    .unwrap();
    assert_eq!(
        (bare.name(), bare.source_path(), bare.get_color_mode()),
        (None, None, ColorMode::Color256)
    );
}

#[test]
fn theme_reads_ambient_color_settings_only_after_valid_input() {
    let valid = dark().to_string();
    let linux = [("TERM", "linux")];
    let cases: [(&str, Option<ColorMode>, &[&str]); 4] = [
        ("{", None, &["read:p"]),
        ("[]", None, &["read:p"]),
        (&valid, Some(ColorMode::Truecolor), &["read:p"]),
        (
            &valid,
            None,
            &["read:p", "env:COLORTERM", "env:WT_SESSION", "env:TERM"],
        ),
    ];
    for (content, mode, reads) in cases {
        let operations = Controlled::new(content, &linux);
        let _ = load_theme_from_path("p", mode, &operations);
        assert_eq!(*operations.log.borrow(), reads, "{content:.12}");
    }
    let absent = missing();
    assert!(load_theme_from_path("p", None, &absent).is_err());
    assert_eq!(*absent.log.borrow(), ["read:p"]);
}

#[test]
fn theme_native_and_controlled_loading_share_the_public_operation() {
    let mut doc = dark();
    doc["name"] = json!(" custom name ");
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let file = directory.join("theme-native-load.json");
    let path = file.to_str().unwrap();
    std::fs::write(&file, doc.to_string()).unwrap();
    let native =
        load_theme_from_path(path, Some(ColorMode::Color256), &NativeThemeOperations).unwrap();
    let controlled = Controlled::new(&doc.to_string(), &[]);
    let same = load_theme_from_path(path, Some(ColorMode::Color256), &controlled).unwrap();
    assert_eq!(*controlled.log.borrow(), [format!("read:{path}")]);
    assert_eq!(native.name(), Some(" custom name "));
    assert_eq!(native.source_path(), Some(path));
    assert_eq!(
        native.fg(&ThemeColor::Accent, "x").unwrap(),
        same.fg(&ThemeColor::Accent, "x").unwrap()
    );
    assert_eq!(
        native.fg(&ThemeColor::Accent, "x").unwrap(),
        "\x1b[38;5;109mx\x1b[39m"
    );

    let mut bytes = dark()
        .to_string()
        .replace("\"dark\"", "\"a@@b\"")
        .into_bytes();
    let at = bytes.windows(2).position(|pair| pair == b"@@").unwrap();
    bytes.splice(at..at + 2, [0xff]);
    std::fs::write(&file, bytes).unwrap();
    let lossy =
        load_theme_from_path(path, Some(ColorMode::Truecolor), &NativeThemeOperations).unwrap();
    assert_eq!(lossy.name(), Some("a\u{fffd}b"));
    assert_eq!(
        NativeThemeOperations
            .read_to_string(directory.join("absent.json").to_str().unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn theme_labels_json_parse_errors_without_relabelling_io() {
    let bom = format!("\u{feff}{}", dark());
    for content in ["", "{", bom.as_str()] {
        let error = load_theme_from_path("p", None, &Controlled::new(content, &[])).unwrap_err();
        assert!(
            error.to_string().starts_with("Failed to parse theme p: "),
            "{content:.8}"
        );
    }
    for content in ["null", "[]"] {
        let error = load_theme_from_path("p", None, &Controlled::new(content, &[])).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Invalid theme \"p\":\n\n\nOther errors:\n  - /: "),
            "{content}"
        );
    }
    let error = load_theme_from_path("p", None, &missing()).unwrap_err();
    assert!(!error.to_string().contains("Failed to parse"));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(cause.kind(), ErrorKind::NotFound);
}
