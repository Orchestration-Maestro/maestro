use super::{TextPiece, fragments};

#[derive(Clone, Copy)]
enum Osc8Terminator {
    Bel,
    St,
}
impl Osc8Terminator {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bel => "\x07",
            Self::St => "\x1b\\",
        }
    }
}
struct ActiveHyperlink {
    params: String,
    url: String,
    terminator: Osc8Terminator,
}
#[derive(Default)]
pub(super) struct AnsiCodeTracker {
    attributes: [bool; 8],
    fg_color: Option<String>,
    bg_color: Option<String>,
    hyperlink: Option<ActiveHyperlink>,
}
impl AnsiCodeTracker {
    fn reset(&mut self) {
        self.attributes.fill(false);
        self.fg_color = None;
        self.bg_color = None;
    }

    pub(super) fn process(&mut self, code: &str) {
        if let Some(body) = code.strip_prefix("\x1b]8;") {
            let terminator = if body.ends_with('\x07') {
                Osc8Terminator::Bel
            } else {
                Osc8Terminator::St
            };
            if let Some((params, url)) = body
                .strip_suffix(terminator.as_str())
                .and_then(|body| body.split_once(';'))
            {
                self.hyperlink = (!url.is_empty()).then(|| ActiveHyperlink {
                    params: params.into(),
                    url: url.into(),
                    terminator,
                });
            }
            return;
        }
        if !code.ends_with('m') {
            return;
        }
        let Some(body) = code.match_indices("\x1b[").find_map(|(offset, _)| {
            let body = code[offset + 2..].split_once('m')?.0;
            body.bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b';')
                .then_some(body)
        }) else {
            return;
        };
        if body.is_empty() {
            self.reset();
            return;
        }
        self.process_sgr(body);
    }

    fn process_sgr(&mut self, body: &str) {
        let mut parameters = body.split(';');
        while let Some(parameter) = parameters.next() {
            let Ok(value) = parameter.parse::<u16>() else {
                continue;
            };
            if matches!(value, 38 | 48) && self.process_color(value, parameter, &mut parameters) {
                continue;
            }
            self.process_attribute(value);
        }
    }

    fn process_color<'a>(
        &mut self,
        value: u16,
        parameter: &'a str,
        parameters: &mut std::str::Split<'a, char>,
    ) -> bool {
        let count = match parameters.clone().next() {
            Some("5") => 2,
            Some("2") => 4,
            _ => return false,
        };
        if parameters.clone().take(count).count() != count {
            return false;
        }
        let color = std::iter::once(parameter)
            .chain(parameters.by_ref().take(count))
            .collect::<Vec<_>>()
            .join(";");
        *if value == 38 {
            &mut self.fg_color
        } else {
            &mut self.bg_color
        } = Some(color);
        true
    }

    fn process_attribute(&mut self, value: u16) {
        match value {
            0 => self.reset(),
            1..=5 => self.attributes[usize::from(value - 1)] = true,
            7..=9 => self.attributes[usize::from(value - 2)] = true,
            21 => self.attributes[0] = false,
            22 => {
                self.attributes[0] = false;
                self.attributes[1] = false;
            }
            23..=25 => self.attributes[usize::from(value - 21)] = false,
            27..=29 => self.attributes[usize::from(value - 22)] = false,
            39 => self.fg_color = None,
            49 => self.bg_color = None,
            30..=37 | 90..=97 => self.fg_color = Some(value.to_string()),
            40..=47 | 100..=107 => self.bg_color = Some(value.to_string()),
            _ => {}
        }
    }

    pub(super) fn update(&mut self, text: &str) {
        for piece in fragments(text) {
            if let TextPiece::Ansi(code) = piece {
                self.process(code);
            }
        }
    }

    pub(super) fn get_active_codes(&self) -> String {
        let codes: Vec<_> = self
            .attributes
            .iter()
            .zip(["1", "2", "3", "4", "5", "7", "8", "9"])
            .filter_map(|(active, code)| active.then_some(code))
            .chain(self.fg_color.as_deref())
            .chain(self.bg_color.as_deref())
            .collect();
        let mut result = if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        };
        if let Some(link) = &self.hyperlink {
            result.push_str("\x1b]8;");
            result.push_str(&link.params);
            result.push(';');
            result.push_str(&link.url);
            result.push_str(link.terminator.as_str());
        }
        result
    }

    pub(super) fn get_line_end_reset(&self) -> String {
        let mut result = if self.attributes[3] {
            "\x1b[24m".into()
        } else {
            String::new()
        };
        self.close_hyperlink(&mut result);
        result
    }

    pub(super) fn close_hyperlink(&self, result: &mut String) {
        if let Some(link) = &self.hyperlink {
            result.push_str("\x1b]8;;");
            result.push_str(link.terminator.as_str());
        }
    }
}
