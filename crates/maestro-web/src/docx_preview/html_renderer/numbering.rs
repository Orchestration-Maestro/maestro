//! Ordered numbering identifiers and presentation.
use super::HtmlRenderer;
use maestro_app::Presence;
use std::fmt::Write as _;

impl HtmlRenderer {
    /// Construct an identifier preserving absent and null operands.
    pub(crate) fn numbering_class(&self, id: Presence<&str>, level: Presence<f64>) -> String {
        let id = scalar_text(id);
        let level = match level {
            Presence::Missing => "undefined".into(),
            Presence::Null => "null".into(),
            Presence::Present(value) => ryu_js::Buffer::new().format(value).to_owned(),
        };
        format!("{}-num-{id}-{level}", self.class_name)
    }

    /// Use the same namespace for counters and paragraph classes.
    pub(crate) fn numbering_counter(&self, id: Presence<&str>, level: Presence<f64>) -> String {
        self.numbering_class(id, level)
    }
}

impl HtmlRenderer {
    /// Borrow the named CSS mapping, retaining unknown and nullable values.
    pub(crate) fn num_format_to_css_value(format: Presence<&str>) -> Presence<&str> {
        let value = match format {
            Presence::Missing => return Presence::Missing,
            Presence::Null => return Presence::Null,
            Presence::Present(value) => value,
        };
        Presence::Present(match value {
            "none" => "none",
            "bullet" => "disc",
            "decimal" => "decimal",
            "lowerLetter" => "lower-alpha",
            "upperLetter" => "upper-alpha",
            "lowerRoman" => "lower-roman",
            "upperRoman" => "upper-roman",
            "decimalZero" => "decimal-leading-zero",
            "aiueo" | "aiueoFullWidth" => "katakana",
            "chineseCounting" | "chineseCountingThousand" => "simp-chinese-informal",
            "chineseLegalSimplified" => "simp-chinese-formal",
            "chosung" => "hangul-consonant",
            "ideographDigital" | "taiwaneseCounting" | "taiwaneseCountingThousand" => {
                "cjk-ideographic"
            }
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
        })
    }
}

impl HtmlRenderer {
    /// Interleave serialized literal segments with genuine ASCII counter placeholders.
    pub(crate) fn level_text_to_content(
        &self,
        text: &str,
        suffix: Option<&str>,
        id: Presence<&str>,
        format: Presence<&str>,
    ) -> String {
        let mut result = String::from("\"");
        let mut remaining = text;
        let mut literal_start = 0;
        while let Some(position) = remaining[literal_start..]
            .find('%')
            .map(|p| p + literal_start)
        {
            let digits = remaining[position + 1..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            if digits == 0 {
                literal_start = position + 1;
                continue;
            }
            append_literal(&mut result, &remaining[..position]);
            let level = remaining[position + 1..position + 1 + digits]
                .parse::<f64>()
                .unwrap_or(f64::NAN)
                - 1.0;
            let counter = self.numbering_counter(id, Presence::Present(level));
            let format = scalar_text(format);
            let _ = write!(result, "\"counter({counter}, {format})\"");
            remaining = &remaining[position + 1 + digits..];
            literal_start = 0;
        }
        append_literal(&mut result, remaining);
        result.push_str(match suffix {
            Some("tab") => "\\9",
            Some("space") => "\\a0",
            _ => "",
        });
        result.push('"');
        result
    }
}

/// Append the shared serializer's literal body inside an already open CSS string.
fn append_literal(output: &mut String, text: &str) {
    let quoted = maestro_app::css::string(text);
    output.push_str(&quoted[1..quoted.len() - 1]);
}

use indexmap::IndexMap;
use maestro_app::browser::docx_preview::IDomNumbering;

impl HtmlRenderer {
    /// Update the same retained last style occurrence subsequently used by paragraphs.
    pub(crate) fn prodess_numberings(&mut self, numberings: &[IDomNumbering]) {
        for num in numberings {
            let Some(name) = num
                .p_style_name
                .as_ref()
                .and_then(|v| v.as_deref())
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let Some(&index) = self.style_map.get(name) else {
                continue;
            };
            if let Some(numbering) = self.styles[index]
                .paragraph_props
                .as_mut()
                .and_then(|p| p.numbering.as_mut())
            {
                numbering.level = Some(num.level);
            }
        }
    }

    /// Emit static list CSS without starting resource work.
    pub(crate) fn render_numbering(
        &self,
        numberings: &[IDomNumbering],
        root_selector: &str,
    ) -> String {
        let mut result = String::new();
        let mut resets = Vec::new();
        for num in numberings {
            self.render_numbering_level(num, &mut result, &mut resets);
        }
        if !resets.is_empty() {
            let resets = resets.join(" ");
            result.push_str(&Self::style_to_string(
                root_selector,
                [("counter-reset", Some(resets.as_str()))],
                None,
            ));
        }
        result
    }

    /// Emit the static picture placeholder without loading its resource.
    fn render_picture_numbering(
        &self,
        selector: &str,
        bullet: &maestro_app::browser::docx_preview::NumberingPicBullet,
    ) -> String {
        let variable = format!(
            "--{}-{}",
            self.class_name,
            bullet.src.as_deref().unwrap_or("null")
        )
        .to_lowercase();
        let background = format!("var({variable})");
        Self::style_to_string(
            &format!("{selector}:before"),
            [
                ("content", Some("' '")),
                ("display", Some("inline-block")),
                ("background", Some(background.as_str())),
            ],
            bullet.style.as_deref(),
        )
    }

    /// Compose one marker followed by its paragraph declarations.
    fn render_numbering_level(
        &self,
        num: &IDomNumbering,
        output: &mut String,
        resets: &mut Vec<String>,
    ) {
        let id = presence(num.id.as_ref().map(|v| v.as_deref()).as_ref());
        let selector = format!("p.{}", self.numbering_class(id, presence(Some(&num.level))));
        let format = Self::num_format_to_css_value(presence(
            num.format.as_ref().map(|v| v.as_deref()).as_ref(),
        ));
        let mut list_type = Some("none");
        if let Some(bullet) = num.bullet.as_ref().and_then(Option::as_ref) {
            output.push_str(&self.render_picture_numbering(&selector, bullet));
        } else if let Some(text) = num
            .level_text
            .as_ref()
            .and_then(|v| v.as_deref())
            .filter(|s| !s.is_empty())
        {
            let counter = self.numbering_counter(id, presence(Some(&num.level)));
            let start = num.start.unwrap_or(f64::NAN) - 1.0;
            let reset = format!("{counter} {}", ryu_js::Buffer::new().format(start));
            if num.level.is_some_and(|level| level > 0.0) {
                let parent = self.numbering_class(id, presence(Some(&num.level.map(|v| v - 1.0))));
                output.push_str(&Self::style_to_string(
                    &format!("p.{parent}"),
                    [("counter-set", Some(reset.as_str()))],
                    None,
                ));
            }
            resets.push(reset);
            let content = self.level_text_to_content(text, num.suff.as_deref(), id, format);
            let mut values = IndexMap::from([
                ("content", Some(content.as_str())),
                ("counter-increment", Some(counter.as_str())),
            ]);
            values.extend(num.r_style.iter().map(|(k, v)| (k.as_str(), v.as_deref())));
            output.push_str(&Self::style_to_string(
                &format!("{selector}:before"),
                values,
                None,
            ));
        } else {
            list_type = Some(scalar_text(format));
        }
        let mut values = IndexMap::from([
            ("display", Some("list-item")),
            ("list-style-position", Some("inside")),
            ("list-style-type", list_type),
        ]);
        values.extend(num.p_style.iter().map(|(k, v)| (k.as_str(), v.as_deref())));
        output.push_str(&Self::style_to_string(&selector, values, None));
    }
}

/// Convert an encountered nullable field without losing omission.
pub(super) fn presence<T: Copy>(value: Option<&Option<T>>) -> Presence<T> {
    value.map_or(Presence::Missing, |value| {
        value.map_or(Presence::Null, Presence::Present)
    })
}

/// Borrow the interpolation spelling of a nullable string scalar.
fn scalar_text(value: Presence<&str>) -> &str {
    match value {
        Presence::Missing => "undefined",
        Presence::Null => "null",
        Presence::Present(value) => value,
    }
}
