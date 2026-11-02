//! Stored values, the cycle, the shown mode and icons.
use super::*;

#[test]
fn absent_and_empty_values_show_the_system_appearance() {
    assert_eq!(initial_theme(None), "system");
    assert_eq!(initial_theme(Some("")), "system");
    assert_eq!(initial_theme(Some("dark")), "dark");
    assert_eq!(initial_theme(Some("custom")), "custom");
}

#[test]
fn the_cycle_counts_system_as_light_unless_it_is_included() {
    let steps = |theme, include| {
        let first = next_theme(theme, include);
        (first, next_theme(first, include))
    };
    assert_eq!(steps("system", false), ("dark", "light"));
    assert_eq!(steps("system", true), ("light", "dark"));
    assert_eq!(steps("light", true), ("dark", "system"));
    assert_eq!(steps("dark", true), ("system", "light"));
    assert_eq!(steps("custom", true), ("light", "dark"));
    assert_eq!(steps("custom", false), ("light", "dark"));
}

#[test]
fn only_dark_or_a_dark_platform_in_system_mode_shows_dark() {
    assert!(is_dark("dark", false));
    assert!(is_dark("system", true));
    assert!(!is_dark("system", false));
    assert!(!is_dark("custom", true), "an unknown value shows light");
    assert_eq!(
        ["light", "dark", "system", "custom"].map(theme_icon),
        ["Sun", "Moon", "Monitor", "Sun"]
    );
}
