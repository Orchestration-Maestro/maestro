//! Terminal color construction and byte-level output.
#[cfg(test)]
mod support;

use maestro_theme::{
    ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions, load_theme_from_path,
};
use serde_json::json;
use support::{Controlled, dark, fg, load, required_colors};

/// Every typed foreground key with its literal theme-file name.
fn foreground_keys() -> Vec<(ThemeColor, &'static str)> {
    vec![
        (ThemeColor::Accent, "accent"),
        (ThemeColor::Border, "border"),
        (ThemeColor::BorderAccent, "borderAccent"),
        (ThemeColor::BorderMuted, "borderMuted"),
        (ThemeColor::Success, "success"),
        (ThemeColor::Error, "error"),
        (ThemeColor::Warning, "warning"),
        (ThemeColor::Muted, "muted"),
        (ThemeColor::Dim, "dim"),
        (ThemeColor::Text, "text"),
        (ThemeColor::ThinkingText, "thinkingText"),
        (ThemeColor::UserMessageText, "userMessageText"),
        (ThemeColor::CustomMessageText, "customMessageText"),
        (ThemeColor::CustomMessageLabel, "customMessageLabel"),
        (ThemeColor::ToolTitle, "toolTitle"),
        (ThemeColor::ToolOutput, "toolOutput"),
        (ThemeColor::MdHeading, "mdHeading"),
        (ThemeColor::MdLink, "mdLink"),
        (ThemeColor::MdLinkUrl, "mdLinkUrl"),
        (ThemeColor::MdCode, "mdCode"),
        (ThemeColor::MdCodeBlock, "mdCodeBlock"),
        (ThemeColor::MdCodeBlockBorder, "mdCodeBlockBorder"),
        (ThemeColor::MdQuote, "mdQuote"),
        (ThemeColor::MdQuoteBorder, "mdQuoteBorder"),
        (ThemeColor::MdHr, "mdHr"),
        (ThemeColor::MdListBullet, "mdListBullet"),
        (ThemeColor::ToolDiffAdded, "toolDiffAdded"),
        (ThemeColor::ToolDiffRemoved, "toolDiffRemoved"),
        (ThemeColor::ToolDiffContext, "toolDiffContext"),
        (ThemeColor::SyntaxComment, "syntaxComment"),
        (ThemeColor::SyntaxKeyword, "syntaxKeyword"),
        (ThemeColor::SyntaxFunction, "syntaxFunction"),
        (ThemeColor::SyntaxVariable, "syntaxVariable"),
        (ThemeColor::SyntaxString, "syntaxString"),
        (ThemeColor::SyntaxNumber, "syntaxNumber"),
        (ThemeColor::SyntaxType, "syntaxType"),
        (ThemeColor::SyntaxOperator, "syntaxOperator"),
        (ThemeColor::SyntaxPunctuation, "syntaxPunctuation"),
        (ThemeColor::ThinkingOff, "thinkingOff"),
        (ThemeColor::ThinkingMinimal, "thinkingMinimal"),
        (ThemeColor::ThinkingLow, "thinkingLow"),
        (ThemeColor::ThinkingMedium, "thinkingMedium"),
        (ThemeColor::ThinkingHigh, "thinkingHigh"),
        (ThemeColor::ThinkingXhigh, "thinkingXhigh"),
        (ThemeColor::BashMode, "bashMode"),
    ]
}

/// Every typed background key with its literal theme-file name.
fn background_keys() -> Vec<(ThemeBg, &'static str)> {
    vec![
        (ThemeBg::SelectedBg, "selectedBg"),
        (ThemeBg::UserMessageBg, "userMessageBg"),
        (ThemeBg::CustomMessageBg, "customMessageBg"),
        (ThemeBg::ToolPendingBg, "toolPendingBg"),
        (ThemeBg::ToolSuccessBg, "toolSuccessBg"),
        (ThemeBg::ToolErrorBg, "toolErrorBg"),
    ]
}

#[test]
fn theme_routes_every_required_key_to_its_ansi_plane() {
    let required = required_colors();
    let mut doc = dark();
    for (index, name) in required.iter().enumerate() {
        doc["colors"][name] = json!(index);
    }
    let theme = load(&doc.to_string(), Some(ColorMode::Truecolor)).unwrap();
    let position = |name: &str| {
        required
            .iter()
            .position(|candidate| candidate == name)
            .unwrap()
    };
    let foregrounds = foreground_keys();
    let backgrounds = background_keys();
    assert_eq!(foregrounds.len() + backgrounds.len(), 51);
    for (key, name) in &foregrounds {
        let expected = format!("\x1b[38;5;{}m", position(name));
        assert_eq!(theme.get_fg_ansi(key).unwrap(), expected, "{name}");
        assert!(theme.get_bg_ansi(&ThemeBg::Named((*name).into())).is_err());
    }
    for (key, name) in &backgrounds {
        let expected = format!("\x1b[48;5;{}m", position(name));
        assert_eq!(theme.get_bg_ansi(key).unwrap(), expected, "{name}");
        assert!(
            theme
                .get_fg_ansi(&ThemeColor::Named((*name).into()))
                .is_err()
        );
    }
}

#[test]
fn theme_color_key_identity_uses_the_key_text() {
    let theme = Theme::new(
        [(ThemeColor::Named("accent".into()), ColorValue::Index(3))],
        [(ThemeBg::Named("selectedBg".into()), ColorValue::Index(4))],
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )
    .unwrap();
    assert_eq!(
        theme.get_fg_ansi(&ThemeColor::Accent).unwrap(),
        "\x1b[38;5;3m"
    );
    assert_eq!(
        theme.get_bg_ansi(&ThemeBg::SelectedBg).unwrap(),
        "\x1b[48;5;4m"
    );
}

#[test]
fn theme_detects_color_mode_with_exact_precedence() {
    let text = dark().to_string();
    let cases: [(&[(&str, &str)], ColorMode); 19] = [
        (&[], ColorMode::Color256),
        (&[("TERM", "")], ColorMode::Color256),
        (&[("TERM", "dumb")], ColorMode::Color256),
        (&[("TERM", "linux")], ColorMode::Color256),
        (&[("TERM", "screen")], ColorMode::Color256),
        (&[("TERM", "screen-256color")], ColorMode::Color256),
        (&[("TERM", "screen.xterm-256color")], ColorMode::Color256),
        (&[("TERM", "screenish")], ColorMode::Truecolor),
        (&[("TERM", "xterm-256color")], ColorMode::Truecolor),
        (&[("TERM", "LINUX")], ColorMode::Truecolor),
        (
            &[("TERM", "xterm"), ("TERM_PROGRAM", "Apple_Terminal")],
            ColorMode::Color256,
        ),
        (
            &[("TERM", "xterm"), ("TERM_PROGRAM", "apple_terminal")],
            ColorMode::Truecolor,
        ),
        (
            &[("TERM", "linux"), ("WT_SESSION", "x")],
            ColorMode::Truecolor,
        ),
        (
            &[("TERM", "linux"), ("WT_SESSION", "")],
            ColorMode::Color256,
        ),
        (
            &[("TERM", "dumb"), ("COLORTERM", "truecolor")],
            ColorMode::Truecolor,
        ),
        (
            &[("TERM", "screen"), ("COLORTERM", "24bit")],
            ColorMode::Truecolor,
        ),
        (
            &[("TERM", "linux"), ("COLORTERM", "TRUECOLOR")],
            ColorMode::Color256,
        ),
        (
            &[
                ("TERM", "screen"),
                ("COLORTERM", "truecolor"),
                ("TERM_PROGRAM", "Apple_Terminal"),
            ],
            ColorMode::Truecolor,
        ),
        (
            &[("TERM", "dumb"), ("WT_SESSION", "0")],
            ColorMode::Truecolor,
        ),
    ];
    for (env, expected) in cases {
        let theme = load_theme_from_path("p", None, &Controlled::new(&text, env)).unwrap();
        assert_eq!(theme.get_color_mode(), expected, "{env:?}");
    }
}

#[test]
fn theme_quantizes_cube_gray_ties_and_saturation() {
    let cases: Vec<([u8; 3], String, String)> =
        serde_json::from_str(include_str!("support/quantization.json")).unwrap();
    for ([r, g, b], foreground, background) in cases {
        let color = ColorValue::String(format!("#{r:02x}{g:02x}{b:02x}"));
        let theme = Theme::new(
            [(ThemeColor::Accent, color.clone())],
            [(ThemeBg::SelectedBg, color)],
            ColorMode::Color256,
            ThemeOptions::default(),
        )
        .unwrap();
        assert_eq!(
            theme.get_fg_ansi(&ThemeColor::Accent).unwrap(),
            foreground,
            "RGB {r},{g},{b}"
        );
        assert_eq!(
            theme.get_bg_ansi(&ThemeBg::SelectedBg).unwrap(),
            background,
            "RGB {r},{g},{b}"
        );
    }
}

#[test]
fn theme_preserves_explicit_palette_indices() {
    for mode in [ColorMode::Truecolor, ColorMode::Color256] {
        for index in 0..=255u8 {
            let theme = Theme::new(
                [(ThemeColor::Accent, ColorValue::Index(index))],
                [(ThemeBg::SelectedBg, ColorValue::Index(index))],
                mode,
                ThemeOptions::default(),
            )
            .unwrap();
            assert_eq!(
                theme.get_fg_ansi(&ThemeColor::Accent).unwrap(),
                format!("\x1b[38;5;{index}m")
            );
            assert_eq!(
                theme.get_bg_ansi(&ThemeBg::SelectedBg).unwrap(),
                format!("\x1b[48;5;{index}m")
            );
        }
    }
}

#[test]
fn theme_wraps_text_without_nested_reset_or_newline_rewriting() {
    let texts = [
        "",
        "x",
        "a\nb",
        "a\r\nb",
        "a\tb",
        "\u{3bb}\u{1f642}e\u{301}",
        "a\x1b[39mb",
        "a\x1b[49mb",
        "\x1b[0m",
    ];
    let colors = [
        (ColorValue::String(String::new()), "\x1b[39m", "\x1b[49m"),
        (ColorValue::Index(3), "\x1b[38;5;3m", "\x1b[48;5;3m"),
        (
            ColorValue::String("#123456".into()),
            "\x1b[38;2;18;52;86m",
            "\x1b[48;2;18;52;86m",
        ),
    ];
    for (color, foreground, background) in colors {
        let theme = Theme::new(
            [(ThemeColor::Accent, color.clone())],
            [(ThemeBg::SelectedBg, color)],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap();
        for text in texts {
            assert_eq!(
                theme.fg(&ThemeColor::Accent, text).unwrap(),
                format!("{foreground}{text}\x1b[39m")
            );
            assert_eq!(
                theme.bg(&ThemeBg::SelectedBg, text).unwrap(),
                format!("{background}{text}\x1b[49m")
            );
        }
    }
}

#[test]
fn theme_reports_absent_constructed_color_keys() {
    let theme = Theme::new([], [], ColorMode::Truecolor, ThemeOptions::default()).unwrap();
    assert_eq!(
        theme.fg(&ThemeColor::Accent, "x").unwrap_err().to_string(),
        "Unknown theme color: accent"
    );
    assert_eq!(
        theme.bg(&ThemeBg::SelectedBg, "x").unwrap_err().to_string(),
        "Unknown theme background color: selectedBg"
    );
    assert_eq!(
        theme
            .get_fg_ansi(&ThemeColor::Accent)
            .unwrap_err()
            .to_string(),
        "Unknown theme color: accent"
    );
    assert_eq!(
        theme
            .get_bg_ansi(&ThemeBg::SelectedBg)
            .unwrap_err()
            .to_string(),
        "Unknown theme background color: selectedBg"
    );
}

/// Construct from string colors, returning the error text or the accent prefix.
fn construct(fg_entries: &[(&str, &str)], bg_entries: &[(&str, &str)]) -> Result<String, String> {
    let string = |value: &str| ColorValue::String(value.into());
    let theme = Theme::new(
        fg_entries
            .iter()
            .map(|(key, value)| (ThemeColor::Named((*key).into()), string(value))),
        bg_entries
            .iter()
            .map(|(key, value)| (ThemeBg::Named((*key).into()), string(value))),
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    Ok(fg(&theme, "accent"))
}

#[test]
fn theme_constructor_keeps_color_validation_order() {
    assert_eq!(
        construct(&[("accent", "#gg0000"), ("accent", "#010203")], &[]),
        Ok("\x1b[38;2;1;2;3m".into())
    );
    assert_eq!(
        construct(
            &[
                ("accent", "#112233"),
                ("border", "#00gg00"),
                ("accent", "#gg0000")
            ],
            &[]
        ),
        Err("Invalid hex color: #gg0000".into())
    );
    assert_eq!(
        construct(&[("accent", "word")], &[]),
        Err("Invalid color value: word".into())
    );
    assert_eq!(
        construct(&[], &[("selectedBg", "word")]),
        Err("Invalid color value: word".into())
    );
    assert_eq!(
        construct(&[("accent", "#gg0000")], &[("selectedBg", "#00gg00")]),
        Err("Invalid hex color: #gg0000".into())
    );
}
