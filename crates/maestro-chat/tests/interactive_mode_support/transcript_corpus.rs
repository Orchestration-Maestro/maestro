//! The recorded transcript corpus and its palette, shared by the tests inside the library
//! and the tests that drive the interactive mode from outside.
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, rc::Rc};

/// The recorded palette and every recorded transcript case.
const FIXTURES: &str = include_str!("../fixtures/interactive_transcript.json");

/// The recorded corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Corpus {
    /// Foreground and background palette indexes the recording used.
    pub(crate) palette: Palette,
    /// The recorded cases.
    cases: Vec<Case>,
}

/// The palette's colour indexes by role name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Palette {
    /// Foreground roles.
    pub(crate) foreground: BTreeMap<String, u8>,
    /// Background roles.
    pub(crate) background: BTreeMap<String, u8>,
}

/// One recorded case, read by its witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: Value,
    /// The recorded observation.
    expected: Value,
}

/// The recorded corpus.
pub(crate) fn corpus() -> Corpus {
    serde_json::from_str(FIXTURES).unwrap()
}

/// The recorded cases of `test`, in order, read as its operation and observation records.
pub(crate) fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    let cases: Vec<_> = corpus()
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty(), "missing group {test}");
    cases
}

/// The theme painting the recorded palette indexes.
pub(crate) fn palette() -> Rc<Theme> {
    let palette = corpus().palette;
    let foreground = palette
        .foreground
        .into_iter()
        .map(|(role, index)| (ThemeColor::Named(role), ColorValue::Index(index)));
    let background = palette
        .background
        .into_iter()
        .map(|(role, index)| (ThemeBg::Named(role), ColorValue::Index(index)));
    Rc::new(
        Theme::new(
            foreground,
            background,
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
