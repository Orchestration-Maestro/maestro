//! Callback brand admission and rendering witnesses.
use super::*;

#[test]
fn oauth_pages_use_replaceable_brand_data() {
    let signal = include_str!("../../../../../assets/brand/packs/signal.json");
    let flat = include_str!("../../../../../assets/brand/mark-flat.svg");
    for (pack, svg, fonts) in [
        (BRAND, MARK, ["Barlow Condensed", "Barlow"]),
        (signal, flat, ["Space Grotesk", "Space Grotesk"]),
    ] {
        let page =
            render_page("Authentication successful", "body witness", None, pack, svg).unwrap();
        let data: serde_json::Value = serde_json::from_str(pack).unwrap();
        let roles = if svg == MARK {
            vec![
                "text",
                "muted",
                "background",
                "mark-ring",
                "mark-m",
                "mark-node",
                "mark-star",
                "mark-live",
            ]
        } else {
            vec![
                "text",
                "muted",
                "background",
                "mark-ring",
                "mark-m",
                "mark-node",
                "mark-star",
            ]
        };
        for role in roles {
            let alias = data["modes"]["dark"]["colors"][role].as_str().unwrap();
            let color = data["palette"][alias].as_str().unwrap();
            if ["text", "muted", "background"].contains(&role) {
                assert!(page.contains(&format!("--{role}: {color};")));
            } else {
                assert!(page.contains(&format!("\"{color}\"")));
            }
        }
        for (role, family) in [("display", fonts[0]), ("body", fonts[1])] {
            assert!(page.contains(&format!(
                "--font-{role}: \"{family}\", system-ui, sans-serif;"
            )));
        }
        assert!(page.contains("--font-mono: \"JetBrains Mono\", ui-monospace, monospace;"));
        let pack = PackData::parse(pack).unwrap();
        let filled = pack
            .presentation(BrandMode::Dark)
            .unwrap()
            .fill_mark(svg)
            .unwrap();
        assert!(page.contains(&filled));
        assert!(page.contains("<h1>Authentication successful</h1>"));
    }
    check_alternate_brand(signal);
}

/// Check changed mark, wordmark and font stacks through the same renderer.
fn check_alternate_brand(signal: &str) {
    let mut alternate: serde_json::Value = serde_json::from_str(signal).unwrap();
    alternate["wordmark"] = serde_json::json!("&<new>\"'");
    alternate["fonts"]["display"]["family"] = serde_json::json!("Alternate Heading");
    alternate["fonts"]["body"]["family"] = serde_json::json!("Alternate Body");
    alternate["fonts"]["mono"]["fallbacks"] = serde_json::json!(["monospace", "ui-monospace"]);
    let page = render_page(
        "Authentication failed",
        "body witness",
        Some("details witness"),
        &alternate.to_string(),
        "<svg><title>{{wordmark}}</title><path fill=\"var(--mark-live)\" d=\"M0 0\"/></svg>",
    )
    .unwrap();
    for value in [
        "&amp;&lt;new&gt;&quot;&apos;",
        "\"Alternate Heading\"",
        "\"Alternate Body\"",
        "--font-mono: \"JetBrains Mono\", monospace, ui-monospace;",
        "<path fill=\"#B6F05A\" d=\"M0 0\"/>",
        "<h1>Authentication failed</h1>",
        "<p>body witness</p>",
        "<div class=\"details\">details witness</div>",
    ] {
        assert!(page.contains(value), "{value}");
    }
}
#[test]
fn oauth_pages_report_invalid_brand_data() {
    let native = maestro_brand::PackData::parse("{")
        .err()
        .unwrap()
        .to_string();
    let error = render_page("Authentication failed", "test", None, "{", MARK).unwrap_err();
    assert_eq!(
        error,
        DiagnosticErrorInfo {
            message: native,
            name: None,
            stack: None,
            code: None
        }
    );
    for (kind, expected) in [
        ("field", "Invalid brand pack: missing field `family`"),
        ("role", "Missing brand color role: text"),
        ("palette", "Missing brand palette color: dangling"),
    ] {
        let mut pack: serde_json::Value = serde_json::from_str(BRAND).unwrap();
        match kind {
            "field" => {
                pack["fonts"]["body"]
                    .as_object_mut()
                    .unwrap()
                    .shift_remove("family");
            }
            "role" => {
                pack["modes"]["dark"]["colors"]
                    .as_object_mut()
                    .unwrap()
                    .shift_remove("text");
            }
            _ => pack["modes"]["dark"]["colors"]["text"] = serde_json::json!("dangling"),
        }
        let error = render_page(
            "Authentication failed",
            "test",
            None,
            &pack.to_string(),
            MARK,
        )
        .unwrap_err();
        assert!(error.message.starts_with(expected), "{}", error.message);
        assert_eq!((error.name, error.code, error.stack), (None, None, None));
    }
}
#[test]
fn oauth_fonts_keep_generic_primary_families_bare() {
    let generics = [
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "ui-serif",
        "ui-sans-serif",
        "ui-monospace",
        "ui-rounded",
        "math",
        "emoji",
        "fangsong",
    ];
    for (role, sibling, family) in [
        ("display", "body", "Barlow"),
        ("body", "display", "Barlow Condensed"),
        ("mono", "body", "Barlow"),
    ] {
        for generic in generics {
            let mixed = format!("{}{}", generic[..1].to_uppercase(), &generic[1..]);
            for name in [generic.to_owned(), generic.to_uppercase(), mixed] {
                let mut pack: serde_json::Value = serde_json::from_str(BRAND).unwrap();
                pack["fonts"][role]["family"] = serde_json::json!(name);
                pack["fonts"][role]["fallbacks"] =
                    serde_json::json!([name, "monospace extra", " serif", "serif ", "", "Σ", "Σ"]);
                let page = render_page("title", "message", None, &pack.to_string(), MARK).unwrap();
                assert!(page.contains(&format!(
                    "--font-{role}: {name}, {name}, \"monospace extra\", \" serif\", \"serif \", \"\", \"Σ\", \"Σ\";"
                )), "generic primary must be bare for {role}");
                assert!(page.contains(&format!(
                    "--font-{sibling}: \"{family}\", system-ui, sans-serif;"
                )));
            }
        }
    }
}

/// A fresh complete pack for a callback witness.
fn pack_data() -> serde_json::Value {
    serde_json::from_str(BRAND).unwrap()
}

/// Render edited data through the path used by both callback exports.
fn page(pack: &serde_json::Value, svg: &str) -> Result<String, DiagnosticErrorInfo> {
    render_page("title", "message", Some("details"), &pack.to_string(), svg)
}

#[test]
fn oauth_fonts_escape_family_names_like_css() {
    let mut cases = vec![
        ("\"\\".to_owned(), "\\\"\\\\".to_owned()),
        ("\0".to_owned(), "\u{fffd}".to_owned()),
        ("<".to_owned(), "\\3c ".to_owned()),
        ("\u{7f}".to_owned(), "\\7f ".to_owned()),
        ("\u{85}\u{feff}é".to_owned(), "\u{85}\u{feff}é".to_owned()),
    ];
    for code in 1..=31 {
        cases.push((
            char::from_u32(code).unwrap().to_string(),
            format!("\\{code:x} "),
        ));
    }
    for role in ["display", "body", "mono"] {
        for (input, escaped) in &cases {
            let name = format!("x{input}f");
            let mut pack = pack_data();
            pack["fonts"][role]["family"] = serde_json::json!(name);
            pack["fonts"][role]["fallbacks"] = serde_json::json!([name, "serif"]);
            let html = page(&pack, MARK).unwrap();
            assert!(
                html.contains(&format!(
                    "--font-{role}: \"x{escaped}f\", \"x{escaped}f\", serif;"
                )),
                "CSS family escape for {role}"
            );
            assert!(html.contains("* { box-sizing: border-box; }"));
        }
    }
}

/// The required field shapes; named entries are checked separately by their consumers.
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

/// Set or remove one member without altering any sibling fields.
fn changed(pointer: &str, value: Option<serde_json::Value>) -> serde_json::Value {
    let mut pack = pack_data();
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let object = pack.pointer_mut(parent).unwrap().as_object_mut().unwrap();
    match value {
        Some(value) => {
            object.insert(key.to_owned(), value);
        }
        None => {
            assert!(object.shift_remove(key).is_some());
        }
    }
    pack
}

#[test]
fn oauth_pages_require_the_complete_brand_shape() {
    use serde_json::{Value, json};
    for (pointer, shape) in FIELDS {
        let wrong = match shape {
            's' => vec![json!(7), json!({}), json!([]), json!(true)],
            'n' => vec![json!("16"), json!({}), json!([]), json!(true)],
            'a' => vec![json!("x"), json!(7), json!({}), json!(true), json!([7])],
            _ => vec![json!([]), json!("x"), json!(7), json!(true)],
        };
        let mut attempts = vec![Some(Value::Null)];
        attempts.extend(wrong.into_iter().map(Some));
        if ![
            "/modes/dark",
            "/fonts/display",
            "/type/body",
            "/glyphs/mark",
        ]
        .contains(&pointer)
        {
            attempts.insert(0, None);
        }
        for attempt in attempts {
            assert!(
                page(&changed(pointer, attempt), MARK).is_err(),
                "required field {pointer}"
            );
        }
    }
    let mut pack = pack_data();
    pack["extension"] = json!({"deep": [[[1]]]});
    pack["fonts"]["display"]["extension"] = Value::Null;
    assert!(page(&pack, MARK).is_ok());
    let deep = format!("{{\"unread\":{}0{}}}", "[".repeat(140), "]".repeat(140));
    let error = render_page("title", "message", None, &deep, MARK).unwrap_err();
    assert!(error.message.contains("recursion limit exceeded"));
}

#[test]
fn oauth_pages_reject_positional_pack_records() {
    use serde_json::json;
    let base = pack_data();
    let cases = [
        ("/fonts/display", json!(["F", []])),
        ("/fonts/display", json!(["F", "OFL", "u", []])),
        ("/modes/dark", json!([base["modes"]["dark"]["colors"]])),
        ("/type/body", json!(["body", 16, 1.5, 400])),
        ("/mark", json!(["m.svg", {}, 32, 16, 0.1, 0.7])),
        ("/glyphs/mark", json!(["*", "accent"])),
        ("/terminal", json!([{}, {}])),
    ];
    for (pointer, value) in cases {
        assert!(
            page(&changed(pointer, Some(value)), MARK).is_err(),
            "object-only {pointer}"
        );
    }
    let positional = json!([
        base["name"],
        base["wordmark"],
        base["tagline"],
        base["palette"],
        base["modes"],
        base["fonts"],
        base["type"],
        base["spacing"],
        base["radii"],
        base["mark"],
        base["glyphs"],
        base["terminal"]
    ]);
    assert!(page(&positional, MARK).is_err(), "object-only root");
}

#[test]
fn oauth_pages_reject_invalid_selected_colors() {
    use serde_json::json;
    for color in [
        "",
        "#12345",
        "#1234567",
        "#12gg56",
        "abcdef",
        "#１２３４５６",
        "red",
    ] {
        let pack = changed("/palette/oxide", Some(json!(color)));
        assert!(
            page(&pack, MARK).is_err(),
            "selected palette color must be hex"
        );
    }
    for color in ["#abcdef", "#ABCDEF"] {
        let pack = changed("/palette/forge-black", Some(json!(color)));
        assert!(
            page(&pack, MARK)
                .unwrap()
                .contains(&format!("--background: {color};"))
        );
    }
}

#[test]
fn oauth_pages_resolve_every_selected_palette_alias() {
    for role in ["panel", "background"] {
        let pointer = format!("/modes/dark/colors/{role}");
        let pack = changed(&pointer, Some(serde_json::json!("absent")));
        let error = page(&pack, "var(--").unwrap_err();
        assert!(
            error.message.contains("palette"),
            "selected aliases precede mark filling"
        );
    }
}

#[test]
fn oauth_pages_validate_brand_measurements() {
    use serde_json::json;
    for pointer in ["/type/body/size", "/spacing/md", "/radii/control"] {
        for value in [-0.5, -1.0] {
            assert!(
                page(&changed(pointer, Some(json!(value))), MARK).is_err(),
                "measurement {pointer}"
            );
        }
        for value in [0.0, -0.0, 0.25] {
            assert_eq!(
                page(&changed(pointer, Some(json!(value))), MARK).is_ok(),
                pointer != "/type/body/size" || value > 0.0,
                "measurement boundary {pointer}"
            );
        }
    }
    let mut pack = pack_data();
    pack["spacing"]["a~/b"] = json!(-0.25);
    let error = page(&pack, MARK).unwrap_err();
    assert!(error.message.contains("/spacing/a~0~1b"));
    pack["spacing"]["a~/b"] = json!(0.25);
    pack["type"]["body"]["line-height"] = json!(-1.25);
    pack["type"]["body"]["weight"] = json!(-0.5);
    pack["mark"]["minimum-size"] = json!(-1.0);
    assert!(page(&pack, MARK).is_ok());
}

#[test]
fn oauth_pages_validate_typography_font_aliases() {
    let pack = changed("/type/body/font", Some(serde_json::json!("unknown")));
    assert!(
        page(&pack, MARK).is_err(),
        "type alias must resolve before page fonts"
    );
    for role in ["display", "body", "mono"] {
        let mut pack = changed(&format!("/fonts/{role}"), None);
        pack["type"] = serde_json::json!({});
        let error = page(&pack, MARK).unwrap_err();
        assert!(error.message.contains(role), "page font {role}");
    }
}

#[test]
fn oauth_pages_validate_glyph_and_terminal_aliases() {
    for pointer in [
        "/glyphs/mark/color",
        "/terminal/ansi/black",
        "/terminal/truecolor/text",
    ] {
        let pack = changed(pointer, Some(serde_json::json!("unknown")));
        assert!(page(&pack, MARK).is_err(), "alias {pointer}");
    }
    let mut pack = pack_data();
    pack["glyphs"]["extra"] = serde_json::json!({"symbol":"X", "color":"text"});
    pack["terminal"]["ansi"]["extra"] = serde_json::json!("muted");
    pack["terminal"]["truecolor"]["extra"] = serde_json::json!("background");
    assert!(page(&pack, MARK).is_ok());
}

#[test]
fn oauth_pages_reject_unterminated_mark_colors() {
    for template in ["var(--", "var(--absent)", "var(--)"] {
        assert!(
            page(&pack_data(), template).is_err(),
            "reached mark token must resolve"
        );
    }
    let pack = changed("/wordmark", Some(serde_json::json!("var(--")));
    assert!(page(&pack, "{{wordmark}}").unwrap().contains("var(--"));
}

#[test]
fn oauth_pages_keep_the_last_repeated_member() {
    let compact = pack_data().to_string();
    for (from, good, bad, expected) in [
        (
            r#""wordmark":"Maestro""#,
            r#""wordmark":7,"wordmark":"Later""#,
            r#""wordmark":"Earlier","wordmark":7""#,
            "<title>Later</title>",
        ),
        (
            r#""family":"Barlow Condensed""#,
            r#""family":7,"family":"Later""#,
            r#""family":"Earlier","family":7""#,
            "--font-display: \"Later\"",
        ),
        (
            r##""forge-black":"#0E0B09""##,
            r##""forge-black":7,"forge\u002dblack":"#abcdef""##,
            r##""forge-black":"#abcdef","forge\u002dblack":7""##,
            "--background: #abcdef;",
        ),
    ] {
        assert!(compact.contains(from));
        let text = compact.replacen(from, good, 1);
        let html = render_page(
            "title",
            "message",
            None,
            &text,
            "<title>{{wordmark}}</title>",
        )
        .unwrap();
        assert!(
            html.contains(expected),
            "last-valid duplicate survives typing"
        );
        let text = compact.replacen(from, bad, 1);
        assert!(render_page("title", "message", None, &text, MARK).is_err());
    }
    let mut pack = pack_data();
    pack["palette"]["é"] = serde_json::json!("#112233");
    pack["palette"]["e\u{301}"] = serde_json::json!("#445566");
    pack["modes"]["dark"]["colors"]["text"] = serde_json::json!("é");
    pack["modes"]["dark"]["colors"]["muted"] = serde_json::json!("e\u{301}");
    let html = page(&pack, MARK).unwrap();
    assert!(html.contains("--text: #112233;"));
    assert!(html.contains("--muted: #445566;"));
}

#[test]
fn oauth_pages_fill_marks_without_rescanning_inserted_values() {
    let mut pack = pack_data();
    pack["wordmark"] = serde_json::json!("&<\"' var(--bad) currentColor {{wordmark}}");
    let template =
        "<svg>{{wordmark}}{{wordmark}}var(--text)currentColor var(--background) {véc}</svg>";
    let html = page(&pack, template).unwrap();
    assert!(
        html.contains(concat!(
            "<svg>&amp;&lt;&quot;&apos; var(--bad) currentColor {{wordmark}}",
            "&amp;&lt;&quot;&apos; var(--bad) currentColor {{wordmark}}",
            "#F2E8DC#F2E8DC #0E0B09 {véc}</svg>"
        )),
        "original template tokens only"
    );
    pack["wordmark"] = serde_json::json!("");
    assert!(
        page(&pack, "<svg>{{wordmark}}{{wordmark}}</svg>")
            .unwrap()
            .contains("<svg></svg>")
    );
    assert!(
        page(&pack, "")
            .unwrap()
            .contains("<div class=\"logo\"></div>")
    );
    for asset in [
        MARK,
        include_str!("../../../../../assets/brand/mark-flat.svg"),
        include_str!("../../../../../assets/brand/mark-mono.svg"),
        include_str!("../../../../../assets/brand/mark-small.svg"),
    ] {
        let html = page(&pack_data(), asset).unwrap();
        assert!(!html.contains("var(--mark-"));
        assert!(!html.contains("currentColor"));
        assert!(!html.contains("{{wordmark}}"));
    }
}

#[test]
fn oauth_pages_preserve_brand_failure_order() {
    use serde_json::json;
    let mut pack = changed("/tagline", Some(json!(null)));
    pack["modes"]["dark"]["colors"]["background"] = json!("missing");
    assert!(
        page(&pack, MARK)
            .unwrap_err()
            .message
            .contains("invalid type"),
        "decode wins"
    );
    pack = changed("/spacing/md", Some(json!(-0.5)));
    pack["modes"] = json!({});
    assert!(
        page(&pack, MARK)
            .unwrap_err()
            .message
            .contains("measurement"),
        "measurement wins"
    );
    let failures = [
        (
            "/modes/dark/colors/background",
            "palette-missing",
            "palette",
        ),
        ("/type/body/font", "font-missing", "font-missing"),
        ("/glyphs/mark/color", "glyph-missing", "glyph-missing"),
        ("/terminal/ansi/black", "ansi-missing", "ansi-missing"),
        (
            "/terminal/truecolor/text",
            "truecolor-missing",
            "truecolor-missing",
        ),
    ];
    for start in 0..failures.len() {
        let mut pack = pack_data();
        for (pointer, value, _) in &failures[start..] {
            *pack.pointer_mut(pointer).unwrap() = json!(value);
        }
        assert!(
            page(&pack, "var(--")
                .unwrap_err()
                .message
                .contains(failures[start].2),
            "projection failure phase {start}"
        );
    }
    check_page_failure_order();
}

/// Page color and font failures precede template substitution.
fn check_page_failure_order() {
    use serde_json::json;
    let mut pack = pack_data();
    pack["glyphs"] = json!({});
    pack["terminal"] = json!({"ansi":{}, "truecolor":{}});
    pack["type"] = json!({});
    pack["modes"]["dark"]["colors"]
        .as_object_mut()
        .unwrap()
        .shift_remove("text");
    pack["fonts"]
        .as_object_mut()
        .unwrap()
        .shift_remove("display");
    assert!(
        page(&pack, "var(--")
            .unwrap_err()
            .message
            .contains("color role: text"),
        "page colors win"
    );
    pack["modes"]["dark"]["colors"]["text"] = json!("bone");
    assert!(
        page(&pack, "var(--")
            .unwrap_err()
            .message
            .contains("font: display"),
        "page fonts win"
    );
}

#[test]
fn oauth_pages_ignore_unselected_references() {
    use serde_json::json;
    let mut pack = pack_data();
    pack["palette"]["unread"] = json!("not a hex color");
    pack["modes"]["light"]["colors"]["text"] = json!("missing");
    pack["mark"]["template"] = json!("a/../literal.svg");
    pack["type"] = json!({});
    pack["glyphs"] = json!({});
    pack["terminal"] = json!({"ansi":{}, "truecolor":{}});
    assert!(page(&pack, MARK).is_ok());
    pack["modes"]["light"]["colors"]["text"] = json!(7);
    assert!(page(&pack, MARK).is_err(), "unselected shapes still decode");
    pack["modes"]["light"]["colors"]["text"] = json!("missing");
    pack["palette"]["unread"] = json!(null);
    assert!(page(&pack, MARK).is_err(), "unread map values still decode");
}
