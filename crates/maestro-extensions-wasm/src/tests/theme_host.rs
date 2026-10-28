//! Recorded host themes and literal theme requests shared by both hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::host::State;
use super::observed::Observed;
use super::renderer_host::Theme;
use super::ui_host::{Receipt, receipts_on};
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::Resource;

/// One recorded answer of a theme.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    /// Method the answer belongs to.
    pub method: String,
    /// Operands the answer was recorded for.
    pub operands: Value,
    /// The recorded result or error.
    pub answer: Value,
}

/// The recorded state of one theme.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Palette {
    /// Theme name.
    pub name: Option<String>,
    /// Theme file path.
    pub source_path: Option<String>,
    /// Theme provenance.
    pub source_info: Option<Value>,
    /// Color depth, in its literal spelling.
    pub mode: String,
    /// Recorded style and color answers.
    pub replies: Vec<Reply>,
}

/// The recorded themes and the provenance edit used by the tests.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Recorded themes by name.
    pub palettes: HashMap<String, Palette>,
    /// Provenance an author writes through a retained theme.
    pub edited: Value,
}

/// The recorded theme corpus.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("theme_corpus.json")).unwrap()
}

/// Themes the host currently publishes under each handed-out key.
#[derive(Default)]
pub struct Bank {
    /// Current state behind each key, created from the palette of the same name.
    themes: HashMap<String, Palette>,
}

/// Publication of a recorded palette under a theme key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    /// The key whose handles observe the publication.
    theme: String,
    /// The recorded palette published under it.
    palette: String,
}

impl Bank {
    /// Publishes a supplied palette under a theme key.
    pub fn publish(&mut self, key: &str, palette: Palette) {
        self.themes.insert(key.to_owned(), palette);
    }

    /// Publishes a recorded palette under a theme key.
    pub fn control(&mut self, text: &str) {
        let control: Control = serde_json::from_str(text).unwrap();
        let palette = corpus().palettes.remove(&control.palette).unwrap();
        self.publish(&control.theme, palette);
    }

    /// The current state behind `key`.
    pub fn theme(&mut self, key: &str) -> &mut Palette {
        self.themes
            .entry(key.to_owned())
            .or_insert_with(|| corpus().palettes.remove(key).unwrap())
    }
}

/// Records one request made on the theme `key` and answers it from that theme's current state.
pub fn request(
    observed: &mut Observed,
    key: &str,
    method: &str,
    operands: &Value,
) -> Result<Value, String> {
    observed.log(format!("theme {key} {method} {operands}"));
    let theme = observed.themes.theme(key);
    match method {
        "name" => Ok(json!(theme.name)),
        "sourcePath" => Ok(json!(theme.source_path)),
        "sourceInfo" => Ok(json!(theme.source_info)),
        "getColorMode" => Ok(json!(theme.mode)),
        "setSourceInfo" => {
            theme.source_info = operands[0]
                .as_str()
                .map(|text| serde_json::from_str(text).unwrap());
            Ok(Value::Null)
        }
        _ => {
            let reply = theme
                .replies
                .iter()
                .find(|reply| reply.method == method && reply.operands == *operands)
                .ok_or_else(|| format!("unrecorded theme request {method} {operands}"))?;
            match &reply.answer["error"] {
                Value::String(error) => Err(error.clone()),
                _ => Ok(reply.answer["ok"].clone()),
            }
        }
    }
}

/// Answers a text request on the theme `key`.
pub fn text(
    observed: &mut Observed,
    key: &str,
    method: &str,
    operands: &Value,
) -> Result<String, String> {
    request(observed, key, method, operands).map(|value| value.as_str().unwrap().to_owned())
}

/// Answers an optional text request on the theme `key`.
pub fn optional(
    observed: &mut Observed,
    key: &str,
    method: &str,
) -> Result<Option<String>, String> {
    request(observed, key, method, &json!([])).map(|value| value.as_str().map(str::to_owned))
}

/// Answers the provenance of the theme `key` as its JSON document.
pub fn source_info(observed: &mut Observed, key: &str) -> Result<Option<String>, String> {
    let value = request(observed, key, "sourceInfo", &json!([]))?;
    Ok((!value.is_null()).then(|| value.to_string()))
}

/// The theme requests recorded in `lines`, in time order.
pub fn receipts(lines: &[String]) -> Vec<Receipt<String>> {
    receipts_on("theme", lines)
}

/// The color depth named by its literal spelling.
fn mode(text: &str) -> host::ColorMode {
    match text {
        "truecolor" => host::ColorMode::Truecolor,
        _ => host::ColorMode::Color256,
    }
}

impl State {
    /// Answers a text request made through `this`.
    fn theme_text(
        &mut self,
        this: &Resource<Theme>,
        method: &str,
        operands: &Value,
    ) -> wasmtime::Result<Result<String, String>> {
        let key = self.table.get(this)?.0.clone();
        Ok(text(&mut self.observed, &key, method, operands))
    }

    /// Answers an optional text request made through `this`.
    fn theme_optional(
        &mut self,
        this: &Resource<Theme>,
        method: &str,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let key = self.table.get(this)?.0.clone();
        Ok(optional(&mut self.observed, &key, method))
    }
}

impl host::HostTheme for State {
    fn name(&mut self, this: Resource<Theme>) -> wasmtime::Result<Result<Option<String>, String>> {
        self.theme_optional(&this, "name")
    }
    fn source_path(
        &mut self,
        this: Resource<Theme>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        self.theme_optional(&this, "sourcePath")
    }
    fn source_info(
        &mut self,
        this: Resource<Theme>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let key = self.table.get(&this)?.0.clone();
        Ok(source_info(&mut self.observed, &key))
    }
    fn set_source_info(
        &mut self,
        this: Resource<Theme>,
        source: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(&this)?.0.clone();
        Ok(request(&mut self.observed, &key, "setSourceInfo", &json!([source])).map(drop))
    }
    fn fg(
        &mut self,
        this: Resource<Theme>,
        color: String,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "fg", &json!([color, text]))
    }
    fn bg(
        &mut self,
        this: Resource<Theme>,
        color: String,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "bg", &json!([color, text]))
    }
    fn bold(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "bold", &json!([text]))
    }
    fn italic(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "italic", &json!([text]))
    }
    fn underline(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "underline", &json!([text]))
    }
    fn inverse(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "inverse", &json!([text]))
    }
    fn strikethrough(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "strikethrough", &json!([text]))
    }
    fn get_fg_ansi(
        &mut self,
        this: Resource<Theme>,
        color: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "getFgAnsi", &json!([color]))
    }
    fn get_bg_ansi(
        &mut self,
        this: Resource<Theme>,
        color: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "getBgAnsi", &json!([color]))
    }
    fn get_color_mode(
        &mut self,
        this: Resource<Theme>,
    ) -> wasmtime::Result<Result<host::ColorMode, String>> {
        Ok(self
            .theme_text(&this, "getColorMode", &json!([]))?
            .map(|text| mode(&text)))
    }
    fn thinking_border(
        &mut self,
        this: Resource<Theme>,
        level: String,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "thinkingBorder", &json!([level, text]))
    }
    fn bash_mode_border(
        &mut self,
        this: Resource<Theme>,
        text: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.theme_text(&this, "bashModeBorder", &json!([text]))
    }
    fn drop(&mut self, this: Resource<Theme>) -> wasmtime::Result<()> {
        let key = self.table.get(&this)?.0.clone();
        self.observed.log(format!("theme-drop {key}"));
        self.release(this)
    }
}
