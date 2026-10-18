//! Strict recorded input envelopes shared by native and browser witnesses.
use maestro_app::Presence;
use maestro_app::browser::docx_preview::{
    IDomNumbering, numbering::NumberingPart, parse_xml_string,
};
use serde::Deserialize;
use serde_json::Value;

/// One consumed behavior and complete observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case<T = Input> {
    /// Behavior association.
    pub test: String,
    /// Producer operands.
    pub input: T,
    /// Native outcome category.
    pub outcome: String,
    /// Complete ordered output.
    pub expected: Value,
}

/// Operation-specific producer operands.
#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub(super) enum Input {
    /// Class identifier operands.
    #[serde(rename = "numberingClass")]
    NumberingClass { args: [Value; 2], namespace: String },
    /// Counter identifier operands.
    #[serde(rename = "numberingCounter")]
    NumberingCounter { args: [Value; 2], namespace: String },
    /// CSS format operand.
    #[serde(rename = "numFormatToCssValue")]
    Format { args: [Value; 1] },
    /// Literal/counter content operands.
    #[serde(rename = "levelTextToContent")]
    Content { args: [Value; 4], namespace: String },
    /// Numbering XML and selector overrides.
    #[serde(rename = "numbering")]
    Numbering {
        /// Recorded levels operand.
        levels: String,
        /// Recorded namespace operand.
        namespace: Option<String>,
        #[serde(rename = "rootSelector")]
        /// Recorded root selector operand.
        root_selector: Option<String>,
    },
    /// Picture producer operands.
    #[serde(rename = "picture")]
    Picture {
        /// Recorded levels operand.
        levels: String,
        #[serde(rename = "pictureId")]
        /// Recorded picture id operand.
        picture_id: String,
    },
    /// Body and optional retained style input.
    #[cfg(target_arch = "wasm32")]
    #[serde(rename = "elements")]
    Elements {
        /// Recorded body operand.
        body: String,
        /// Recorded styles operand.
        styles: Option<String>,
        /// Recorded repeat operand.
        repeat: Option<bool>,
    },
    /// Native class operands.
    #[cfg(target_arch = "wasm32")]
    #[serde(rename = "class")]
    Class {
        #[serde(rename = "className")]
        /// Recorded class name operand.
        class_name: Option<String>,
        #[serde(rename = "styleName")]
        /// Recorded style name operand.
        style_name: Option<String>,
    },
    /// Ordered nullable declarations.
    #[cfg(target_arch = "wasm32")]
    #[serde(rename = "style")]
    Style {
        /// Recorded values operand.
        values: Vec<(String, Option<String>)>,
    },
    /// Retained style and concrete numbering XML.
    #[cfg(target_arch = "wasm32")]
    #[serde(rename = "prepare")]
    Prepare {
        /// Retained style definitions.
        styles: String,
        /// Concrete numbering levels.
        levels: String,
    },
}

/// Deserialize the whole corpus before selecting its associated behavior.
pub(super) fn cases(name: &str) -> Vec<Case> {
    serde_json::from_str::<Vec<Case<Value>>>(include_str!("../fixtures/elements-numbering.json"))
        .unwrap()
        .into_iter()
        .filter(|case| case.test == name)
        .map(|case| Case {
            test: case.test,
            input: serde_json::from_value(case.input).unwrap(),
            outcome: case.outcome,
            expected: case.expected,
        })
        .collect()
}

/// Decode the recorder's absent/string/null distinction.
pub(super) fn string(value: &Value) -> Presence<&str> {
    if value == &serde_json::json!({"absent": true}) {
        Presence::Missing
    } else if value.is_null() {
        Presence::Null
    } else {
        Presence::Present(value.as_str().unwrap())
    }
}

/// Decode the recorder's numeric boundary tags.
pub(super) fn number(value: &Value) -> Presence<f64> {
    if value == &serde_json::json!({"absent": true}) {
        Presence::Missing
    } else if value.is_null() {
        Presence::Null
    } else if let Some(tag) = value.get("number") {
        Presence::Present(match tag.as_str().unwrap() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-0" => -0.0,
            _ => unreachable!(),
        })
    } else {
        Presence::Present(value.as_f64().unwrap())
    }
}

/// Run the existing complete numbering producer, including picture resolution.
pub(super) fn numberings(levels: &str, picture: Option<&str>) -> Vec<IDomNumbering> {
    let picture = picture.map_or_else(String::new, |id| format!(r#"<w:numPicBullet w:numPicBulletId="1"><w:pict><v:shape style="width:8pt;height:8pt"><v:imagedata r:id="{id}"/></v:shape></w:pict></w:numPicBullet>"#));
    let source = format!(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{picture}<w:abstractNum w:abstractNumId="1">{levels}</w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
    );
    let xml = parse_xml_string(&source, false).unwrap();
    NumberingPart::parse_xml(&xml, xml.root(), false).dom_numberings
}
