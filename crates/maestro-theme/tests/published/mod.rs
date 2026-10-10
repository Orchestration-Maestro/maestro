//! Published themes with distinct palette indexes, shared by the style and syntax tests.
use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, Theme, ThemeColor, ThemeDirectories, ThemeInfo,
    ThemeOperations, ThemeOptions, ThemeState,
};
use std::io;
use std::rc::Rc;

/// Theme operations over an empty file system and environment.
struct NoFiles;

impl ThemeOperations for NoFiles {
    fn read_to_string(&self, _path: &str) -> io::Result<String> {
        Err(io::ErrorKind::NotFound.into())
    }

    fn environment(&self, _name: &str) -> Option<String> {
        None
    }

    fn exists(&self, _path: &str) -> bool {
        false
    }

    fn read_dir(&self, _path: &str) -> io::Result<Vec<String>> {
        Err(io::ErrorKind::NotFound.into())
    }

    fn sort_by_name(&self, _themes: &mut [ThemeInfo]) -> io::Result<()> {
        Ok(())
    }
}

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
    let colors = KEYS
        .into_iter()
        .zip(offset + 1..)
        .map(|(name, index)| (ThemeColor::Named(name.to_owned()), ColorValue::Index(index)));
    Rc::new(Theme::new(colors, [], ColorMode::Truecolor, ThemeOptions::default()).unwrap())
}

/// A state that never published a theme.
pub fn unpublished() -> ThemeState {
    ThemeState::new(
        ThemeDirectories {
            themes_dir: String::new(),
            custom_themes_dir: String::new(),
        },
        Rc::new(NoFiles),
    )
}

/// A state with `first` published and a handle on its live slot.
pub fn published(first: &Rc<Theme>) -> (ThemeState, LiveTheme) {
    let state = unpublished();
    state.set_theme_instance(Rc::clone(first)).unwrap();
    let live = state.theme();
    (state, live)
}
