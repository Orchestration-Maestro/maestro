use super::*;

#[test]
fn pane_default_color_reset_sequences_use_xterm_osc_numbers() {
    assert_eq!(
        osc_reset_default_color_sequence(DefaultColorKind::Foreground),
        "\x1b]110\x1b\\"
    );
    assert_eq!(
        osc_reset_default_color_sequence(DefaultColorKind::Background),
        "\x1b]111\x1b\\"
    );
}

#[test]
fn theme_state_and_emission_keep_exact_bytes() {
    let color = RgbColor {
        r: 1,
        g: 0xab,
        b: 0xff,
    };
    let mut theme = TerminalTheme::default();
    assert_eq!(theme.foreground, None);
    assert_eq!(theme.background, None);
    assert_eq!(theme.palette, [None; 256]);
    assert!(theme.is_empty());
    theme = theme
        .with_palette_color(0, color)
        .with_palette_color(255, color);
    assert_eq!(theme.palette[0], Some(color));
    assert_eq!(theme.palette[255], Some(color));
    assert_eq!(theme.palette[1], None);
    assert!(theme.is_empty());
    let replacement = RgbColor { r: 0, g: 0, b: 0 };
    theme = theme.with_palette_color(0, replacement);
    assert_eq!(theme.palette[0], Some(replacement));
    theme = theme.with_color(DefaultColorKind::Foreground, color);
    assert_eq!(theme.foreground, Some(color));
    assert_eq!(theme.background, None);
    assert!(!theme.is_empty());
    theme = theme.with_color(DefaultColorKind::Background, replacement);
    assert_eq!(theme.background, Some(replacement));
    assert!(
        !TerminalTheme::default()
            .with_color(DefaultColorKind::Background, color)
            .is_empty()
    );
    appearance_reports();
    color_emission();
}

fn appearance_reports() {
    assert_eq!(
        inferred_appearance(RgbColor {
            r: 127,
            g: 127,
            b: 127
        }),
        HostAppearance::Dark
    );
    assert_eq!(
        inferred_appearance(RgbColor {
            r: 128,
            g: 128,
            b: 128
        }),
        HostAppearance::Light
    );
    assert_eq!(HostAppearance::Dark.color_scheme_report(), b"\x1b[?997;1n");
    assert_eq!(HostAppearance::Light.color_scheme_report(), b"\x1b[?997;2n");
}

fn color_emission() {
    let color = RgbColor {
        r: 1,
        g: 0xab,
        b: 0xff,
    };
    let replacement = RgbColor { r: 0, g: 0, b: 0 };
    assert_eq!(HOST_COLOR_SCHEME_QUERY_SEQUENCE, "\x1b[?996n");
    assert_eq!(HOST_COLOR_SCHEME_REPORT_ENABLE_SEQUENCE, "\x1b[?2031h");
    assert_eq!(HOST_COLOR_SCHEME_REPORT_DISABLE_SEQUENCE, "\x1b[?2031l");
    assert_eq!(
        maestro_tui::terminal_theme::host_terminal_theme_query_sequence(false),
        "\x1b]10;?\x1b\\\x1b]11;?\x1b\\"
    );
    assert_eq!(
        osc_set_default_color_sequence(DefaultColorKind::Foreground, color),
        "\x1b]10;rgb:01/ab/ff\x1b\\"
    );
    assert_eq!(
        osc_set_default_color_sequence(DefaultColorKind::Background, color),
        "\x1b]11;rgb:01/ab/ff\x1b\\"
    );
    assert_eq!(
        osc_set_default_color_sequence(DefaultColorKind::Foreground, replacement),
        "\x1b]10;rgb:00/00/00\x1b\\"
    );
    assert_eq!(
        osc_reset_default_color_sequence(DefaultColorKind::Foreground),
        "\x1b]110\x1b\\"
    );
    assert_eq!(
        osc_reset_default_color_sequence(DefaultColorKind::Background),
        "\x1b]111\x1b\\"
    );
}
