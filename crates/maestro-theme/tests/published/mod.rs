//! Published themes with distinct palette indexes, shared by the style and syntax tests.
use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, Theme, ThemeColor, ThemeDirectories, ThemeOptions, ThemeState,
};
use std::rc::Rc;

/// Every foreground key, in the order that fixes its palette index.
const KEYS: [&str; 45] = [
    "accent",
    "border",
    "borderAccent",
    "borderMuted",
    "success",
    "error",
    "warning",
    "muted",
    "dim",
    "text",
    "thinkingText",
    "userMessageText",
    "customMessageText",
    "customMessageLabel",
    "toolTitle",
    "toolOutput",
    "mdHeading",
    "mdLink",
    "mdLinkUrl",
    "mdCode",
    "mdCodeBlock",
    "mdCodeBlockBorder",
    "mdQuote",
    "mdQuoteBorder",
    "mdHr",
    "mdListBullet",
    "toolDiffAdded",
    "toolDiffRemoved",
    "toolDiffContext",
    "syntaxComment",
    "syntaxKeyword",
    "syntaxFunction",
    "syntaxVariable",
    "syntaxString",
    "syntaxNumber",
    "syntaxType",
    "syntaxOperator",
    "syntaxPunctuation",
    "thinkingOff",
    "thinkingMinimal",
    "thinkingLow",
    "thinkingMedium",
    "thinkingHigh",
    "thinkingXhigh",
    "bashMode",
];

/// Theme whose key at position `i` of [`KEYS`] has palette index `offset + i + 1`.
pub fn theme(offset: u8) -> Rc<Theme> {
    named(offset, None)
}

/// The same palette, registered under `name` when one is supplied.
pub fn named(offset: u8, name: Option<&str>) -> Rc<Theme> {
    let colors = KEYS
        .into_iter()
        .zip(offset + 1..)
        .map(|(key, index)| (ThemeColor::Named(key.to_owned()), ColorValue::Index(index)));
    let options = ThemeOptions {
        name: name.map(str::to_owned),
        ..ThemeOptions::default()
    };
    Rc::new(Theme::new(colors, [], ColorMode::Truecolor, options).unwrap())
}

/// A state that never published a theme and names no directories.
pub fn unpublished() -> ThemeState {
    ThemeState::new(ThemeDirectories {
        themes_dir: String::new(),
        custom_themes_dir: String::new(),
    })
}

/// A state with `first` published and a handle on its live slot.
pub fn published(first: &Rc<Theme>) -> (ThemeState, LiveTheme) {
    let state = unpublished();
    state.set_theme_instance(Rc::clone(first)).unwrap();
    let live = state.theme();
    (state, live)
}
