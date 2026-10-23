//! List numbering: counters, markers, resets, list formats, level text and picture bullets.
use super::HtmlRenderer;
use super::stylesheet::{contained_values, create_style_element, identifier, style_to_string};
use crate::docx_preview::PartRecords;
use crate::docx_preview::html_element::{HtmlElement, contained};
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{IDomNumbering, Numbering, NumberingPicBullet};
use std::fmt::Write as _;

impl HtmlRenderer<'_> {
    /// Name a paragraph's numbering class from the reference it carries.
    pub(super) fn numbering_class(&self, numbering: &Numbering) -> String {
        let id = match numbering.id.as_ref() {
            None => "undefined",
            Some(None) => "null",
            Some(Some(id)) => id,
        };
        let level = match numbering.level {
            None => "undefined".to_owned(),
            Some(None) => "null".to_owned(),
            Some(Some(level)) => spell_number(level),
        };
        numbering_class(&self.options().class_name, id, &level)
    }

    /// Write each level's marker, counter and list rules, then one `counter-reset` on the
    /// wrapper, or on the root without a wrapper; a level whose counter or bullet variable is
    /// not a plain identifier is skipped.
    pub(super) fn render_numbering(
        &mut self,
        numbering: &PartRecords<IDomNumbering>,
    ) -> HtmlElement {
        let options = self.options();
        let root_selector = if options.wrapper.in_wrapper {
            format!(".{}-wrapper", options.class_name)
        } else {
            ":root".into()
        };
        let mut css = String::new();
        let mut resets = Vec::new();
        for num in &numbering.records {
            if let Some(level) =
                self.render_numbering_level(num, numbering, &root_selector, &mut resets)
            {
                css.push_str(&level);
            }
        }
        if !resets.is_empty() {
            let resets = resets.join(" ");
            css.push_str(&style_to_string(
                &root_selector,
                [("counter-reset", Some(resets.as_str()))],
                None,
            ));
        }
        create_style_element(&css)
    }

    /// Write one level's rules, recording its counter reset; `None` skips the level.
    fn render_numbering_level(
        &mut self,
        num: &IDomNumbering,
        numbering: &PartRecords<IDomNumbering>,
        root_selector: &str,
        resets: &mut Vec<String>,
    ) -> Option<String> {
        let class_name = self.options().class_name.as_str();
        let counter = numbering_class(class_name, level_id(num), &spell_level(num.level));
        if !identifier(&counter) {
            return None;
        }
        let selector = format!("p.{counter}");
        let format = num_format_to_css_value(level_format(num));
        let mut css = String::new();
        let mut list_style_type = "none";
        if let Some(Some(bullet)) = &num.bullet {
            css = self.render_picture_numbering(&selector, bullet, numbering, root_selector)?;
        } else if let Some(text) = num
            .level_text
            .as_ref()
            .and_then(Option::as_deref)
            .filter(|text| !text.is_empty())
        {
            let start = ryu_js::Buffer::new()
                .format(num.start.unwrap_or(0.0) - 1.0)
                .to_owned();
            let reset = format!("{counter} {start}");
            if let Some(level) = num.level.filter(|level| *level > 0.0) {
                let parent = numbering_class(class_name, level_id(num), &spell_number(level - 1.0));
                let values = [("counter-set", Some(reset.as_str()))];
                css.push_str(&style_to_string(&format!("p.{parent}"), values, None));
            }
            let content = level_text_to_content(class_name, text, num.suff.as_deref(), num, format);
            let mut values = IndexMap::from([
                ("content", Some(content.as_str())),
                ("counter-increment", Some(counter.as_str())),
            ]);
            values.extend(num.r_style.iter().map(|(k, v)| (k.as_str(), v.as_deref())));
            let selector = format!("{selector}:before");
            css.push_str(&style_to_string(&selector, contained_values(values), None));
            resets.push(reset);
        } else {
            list_style_type = format;
        }
        let mut values = IndexMap::from([
            ("display", Some("list-item")),
            ("list-style-position", Some("inside")),
            ("list-style-type", Some(list_style_type)),
        ]);
        values.extend(num.p_style.iter().map(|(k, v)| (k.as_str(), v.as_deref())));
        css.push_str(&style_to_string(&selector, contained_values(values), None));
        Some(css)
    }

    /// Write a picture bullet's marker, which shows the image a variable names, and add the
    /// variable's rule once the image loads; `None` when the variable is not an identifier.
    fn render_picture_numbering(
        &mut self,
        selector: &str,
        bullet: &NumberingPicBullet,
        numbering: &PartRecords<IDomNumbering>,
        root_selector: &str,
    ) -> Option<String> {
        let src = bullet.src.as_deref().unwrap_or("null");
        let variable = format!("--{}-{src}", self.options().class_name).to_lowercase();
        if !identifier(&variable) {
            return None;
        }
        let background = format!("var({variable})");
        let values = [
            ("content", Some("' '")),
            ("display", Some("inline-block")),
            ("background", Some(background.as_str())),
        ];
        let suffix = bullet
            .style
            .as_deref()
            .filter(|style| style.split(';').all(contained));
        let css = style_to_string(&format!("{selector}:before"), values, suffix);
        if let Some(url) = self.load_numbering_image(numbering, bullet.src.as_deref()) {
            let rule = format!("{root_selector} {{ {variable}: url({url}) }}");
            self.resource_styles.push(create_style_element(&rule));
        }
        Some(css)
    }
}

/// Name a numbering class or counter from an identifier and a level already spelled as text
/// interpolation writes them.
fn numbering_class(class_name: &str, id: &str, level: &str) -> String {
    format!("{class_name}-num-{id}-{level}")
}

/// The identifier a level's own number carries, spelled as text interpolation writes it.
fn level_id(num: &IDomNumbering) -> &str {
    match num.id.as_ref() {
        None => "undefined",
        Some(None) => "null",
        Some(Some(id)) => id,
    }
}

/// Spell a level as text interpolation writes it, a level the document leaves unset counting
/// as null.
fn spell_level(level: Option<f64>) -> String {
    level.map_or_else(|| "null".into(), spell_number)
}

/// Spell a number as text interpolation writes it.
fn spell_number(level: f64) -> String {
    ryu_js::Buffer::new().format(level).to_owned()
}

/// The list format a level's own number names, spelled as text interpolation writes it.
fn level_format(num: &IDomNumbering) -> &str {
    match num.format.as_ref() {
        None => "undefined",
        Some(None) => "null",
        Some(Some(format)) => format,
    }
}

/// Replace each `%` followed by digits with the counter of the level it names, keeping the
/// other text, lone `%` included, as CSS string text, then add the suffix's separator.
fn level_text_to_content(
    class_name: &str,
    text: &str,
    suff: Option<&str>,
    num: &IDomNumbering,
    format: &str,
) -> String {
    let mut result = String::from("\"");
    let mut literal_start = 0;
    let mut search = 0;
    while let Some(position) = text[search..].find('%').map(|offset| search + offset) {
        let digits = text[position + 1..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        search = position + 1 + digits;
        if digits == 0 {
            continue;
        }
        append_literal(&mut result, &text[literal_start..position]);
        let level = text[position + 1..search]
            .parse::<f64>()
            .unwrap_or(f64::NAN)
            - 1.0;
        let counter = numbering_class(class_name, level_id(num), &spell_number(level));

        let _ = write!(result, "\"counter({counter}, {format})\"");
        literal_start = search;
    }
    append_literal(&mut result, &text[literal_start..]);
    result.push_str(match suff {
        Some("tab") => "\\9",
        Some("space") => "\\a0",
        _ => "",
    });
    result.push('"');
    result
}

/// Append text as the body of a CSS string already open.
fn append_literal(output: &mut String, text: &str) {
    let quoted = maestro_app::css::string(text);
    output.push_str(
        quoted
            .strip_prefix('"')
            .and_then(|quoted| quoted.strip_suffix('"'))
            .unwrap_or_default(),
    );
}

/// Map a list format to its CSS list style, keeping an unknown format as written.
fn num_format_to_css_value(format: &str) -> &str {
    match format {
        "bullet" => "disc",
        "lowerLetter" => "lower-alpha",
        "upperLetter" => "upper-alpha",
        "lowerRoman" => "lower-roman",
        "upperRoman" => "upper-roman",
        "decimalZero" => "decimal-leading-zero",
        "aiueo" | "aiueoFullWidth" => "katakana",
        "chineseCounting" | "chineseCountingThousand" => "simp-chinese-informal",
        "chineseLegalSimplified" => "simp-chinese-formal",
        "chosung" => "hangul-consonant",
        "ideographDigital" | "taiwaneseCounting" | "taiwaneseCountingThousand" => "cjk-ideographic",
        "ideographTraditional" => "cjk-heavenly-stem",
        "ideographLegalTraditional" => "trad-chinese-formal",
        "ideographZodiac" => "cjk-earthly-branch",
        "iroha" | "irohaFullWidth" => "katakana-iroha",
        "japaneseCounting" => "japanese-informal",
        "japaneseDigitalTenThousand" | "taiwaneseDigital" => "cjk-decimal",
        "japaneseLegal" => "japanese-formal",
        "thaiNumbers" => "thai",
        "koreanCounting" | "koreanDigital" => "korean-hangul-formal",
        "koreanDigital2" => "korean-hanja-informal",
        "hebrew1" | "hebrew2" => "hebrew",
        "hindiNumbers" => "devanagari",
        "ganada" => "hangul",
        other => other,
    }
}
