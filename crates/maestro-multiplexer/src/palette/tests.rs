use super::*;
#[cfg(target_os = "linux")]
use maestro_app::daemon::Config;
use maestro_app::daemon::THEME_NAMES;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Every token of a palette by field name, in debug notation.
fn token_map(palette: &Palette) -> BTreeMap<String, String> {
    let tokens = [
        ("accent", palette.accent),
        ("panel_bg", palette.panel_bg),
        ("sidebar_bg", palette.sidebar_bg),
        ("active_row_bg", palette.active_row_bg),
        ("selection_bg", palette.selection_bg),
        ("surface0", palette.surface0),
        ("surface1", palette.surface1),
        ("surface_dim", palette.surface_dim),
        ("overlay0", palette.overlay0),
        ("overlay1", palette.overlay1),
        ("text", palette.text),
        ("subtext0", palette.subtext0),
        ("mauve", palette.mauve),
        ("green", palette.green),
        ("yellow", palette.yellow),
        ("red", palette.red),
        ("blue", palette.blue),
        ("teal", palette.teal),
        ("peach", palette.peach),
    ];
    tokens
        .into_iter()
        .map(|(name, color)| (name.to_owned(), format!("{color:?}")))
        .collect()
}

fn rgb_luminance(color: Color) -> f64 {
    let Color::Rgb(r, g, b) = color else {
        panic!("expected an RGB colour, got {color:?}");
    };
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (a, b) = (rgb_luminance(a), rgb_luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Built-in palettes with a colour of their own, unlike the terminal theme.
fn coloured_palettes() -> impl Iterator<Item = (&'static str, Palette)> {
    THEME_NAMES
        .iter()
        .filter(|name| **name != "terminal")
        .map(|name| (*name, Palette::from_name(name).unwrap()))
}

#[test]
fn palette_every_built_in_name_resolves() {
    for name in THEME_NAMES {
        assert!(Palette::from_name(name).is_some(), "{name}");
    }
}

#[test]
fn palette_active_row_stays_visible_against_matching_terminal_background() {
    for (name, palette) in coloured_palettes() {
        let background = contrast_ratio(palette.panel_bg, palette.active_row_bg);
        assert!(background >= 1.05, "{name}: {background:.2}:1");
        let text = contrast_ratio(palette.text, palette.active_row_bg);
        assert!(text >= 3.0, "{name}: {text:.2}:1");
    }
}

#[test]
fn palette_selection_row_stays_distinct_from_background_and_active_row() {
    for (name, palette) in coloured_palettes() {
        let background = contrast_ratio(palette.panel_bg, palette.selection_bg);
        assert!(background >= 1.05, "{name}: {background:.2}:1");
        let text = contrast_ratio(palette.text, palette.selection_bg);
        assert!(text >= 3.0, "{name}: {text:.2}:1");
        assert_ne!(palette.selection_bg, palette.active_row_bg, "{name}");
    }
}

#[test]
fn palette_built_in_themes_leave_sidebar_background_unset() {
    for name in THEME_NAMES {
        let palette = Palette::from_name(name).unwrap();
        assert_eq!(palette.sidebar_bg, Color::Reset, "{name}");
    }
}

#[test]
fn palette_custom_colors_override_built_in_tokens() {
    let custom = CustomThemeColors {
        sidebar_bg: Some("#181825".to_owned()),
        active_row_bg: Some("#313244".to_owned()),
        selection_bg: Some("#45475a".to_owned()),
        ..CustomThemeColors::default()
    };
    let palette = Palette::catppuccin().with_overrides(&custom);
    assert_eq!(palette.sidebar_bg, Color::Rgb(24, 24, 37));
    assert_eq!(palette.active_row_bg, Color::Rgb(49, 50, 68));
    assert_eq!(palette.selection_bg, Color::Rgb(69, 71, 90));
    assert_eq!(palette.accent, Palette::catppuccin().accent);
}

#[test]
fn palette_overrides_reach_every_token() {
    let value = || Some("#010203".to_owned());
    let custom = CustomThemeColors {
        accent: value(),
        panel_bg: value(),
        sidebar_bg: value(),
        active_row_bg: value(),
        selection_bg: value(),
        surface0: value(),
        surface1: value(),
        surface_dim: value(),
        overlay0: value(),
        overlay1: value(),
        text: value(),
        subtext0: value(),
        mauve: value(),
        green: value(),
        yellow: value(),
        red: value(),
        blue: value(),
        teal: value(),
        peach: value(),
    };
    let tokens = token_map(&Palette::vesper().with_overrides(&custom));
    assert_eq!(tokens.len(), 19);
    assert!(
        tokens.values().all(|color| color == "Rgb(1, 2, 3)"),
        "{tokens:?}"
    );
}

#[test]
fn palette_light_aliases_resolve() {
    for name in ["light", "latte", "tokyo-day", "onelight", "lotus", "dawn"] {
        assert!(Palette::from_name(name).is_some(), "{name}");
    }
}

#[test]
fn palette_parse_color_accepts_reset_aliases() {
    for value in ["reset", "default", "none", "transparent"] {
        assert_eq!(parse_color(value), Color::Reset, "{value}");
    }
}

#[test]
fn palette_unknown_color_defaults_to_cyan() {
    assert_eq!(parse_color("chartreuse"), Color::Cyan);
    assert_eq!(parse_color(""), Color::Cyan);
}

#[test]
fn palette_parse_color_keeps_non_ascii_hex_as_an_unknown_name() {
    assert_eq!(parse_color("#aé€"), Color::Cyan);
    assert_eq!(parse_color("#é1"), Color::Cyan);
}

#[cfg(target_os = "linux")]
#[test]
fn palette_unknown_theme_falls_back_to_its_default() {
    let runtime = ThemeRuntimeConfig::new(&Config::default(), true);
    assert_eq!(
        resolve_palette_for_theme_name("catppucin", "tokyo-night", &runtime),
        Palette::tokyo_night()
    );
    assert_eq!(
        resolve_palette_for_theme_name("catppucin", "lattee", &runtime),
        Palette::catppuccin()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn palette_resolution_applies_overrides_then_legacy_accent() {
    let mut config = Config::default();
    config.theme.custom = Some(CustomThemeColors {
        text: Some("#0a0b0c".to_owned()),
        ..CustomThemeColors::default()
    });
    config.ui.accent = "red".to_owned();
    let runtime = ThemeRuntimeConfig::new(&config, true);
    let palette = resolve_palette_for_theme_name("nord", "catppuccin", &runtime);
    assert_eq!(palette.text, Color::Rgb(10, 11, 12));
    assert_eq!(palette.accent, Color::Red);
    assert_eq!(palette.panel_bg, Palette::nord().panel_bg);
}

/// Reference results for colour parsing, theme names and built-in tokens.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaletteCorpus {
    parse_color: Vec<ParsedColor>,
    theme_names: Vec<ThemeName>,
    palettes: Vec<BuiltInPalette>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParsedColor {
    input: String,
    color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeName {
    input: String,
    canonical: Option<String>,
    resolves: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuiltInPalette {
    name: String,
    palette: BTreeMap<String, String>,
}

fn corpus() -> PaletteCorpus {
    serde_json::from_str(include_str!("../../tests/fixtures/palette-corpus.json")).unwrap()
}

#[test]
fn palette_parse_color_matches_reference_cases() {
    let cases = corpus().parse_color;
    assert_eq!(cases.len(), 44);
    for case in cases {
        assert_eq!(
            format!("{:?}", parse_color(&case.input)),
            case.color,
            "{:?}",
            case.input
        );
    }
}

#[test]
fn palette_theme_names_match_reference_cases() {
    use maestro_app::daemon::canonical_theme_name;
    let cases = corpus().theme_names;
    assert_eq!(cases.len(), 25);
    for case in cases {
        assert_eq!(
            canonical_theme_name(&case.input),
            case.canonical.as_deref(),
            "{:?}",
            case.input
        );
        assert_eq!(
            Palette::from_name(&case.input).is_some(),
            case.resolves,
            "{:?}",
            case.input
        );
    }
}

#[test]
fn palette_built_in_tokens_match_reference() {
    let palettes = corpus().palettes;
    let names: Vec<&str> = palettes
        .iter()
        .map(|palette| palette.name.as_str())
        .collect();
    assert_eq!(names, THEME_NAMES);
    for expected in palettes {
        let palette = Palette::from_name(&expected.name).unwrap();
        assert_eq!(token_map(&palette), expected.palette, "{}", expected.name);
    }
}
