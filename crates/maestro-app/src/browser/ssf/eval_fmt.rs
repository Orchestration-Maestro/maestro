//! Selected spreadsheet section evaluation.

#[cfg(test)]
use super::{
    DateCode, DateToken, FormatError, GeneralValue, NumberToken, general,
    general::is_general,
    is_date::{is_date, is_elapsed, meridiem_pattern},
    parse_date_code::{Calendar, parse_date_code_with_calendar},
    write_date, write_num,
};

mod numbers;
#[cfg(test)]
use numbers::render_numbers;

/// A field's role before rendering.
#[cfg(test)]
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text,
    Space,
    Digits,
    Number,
    Question,
    Parenthesis,
    Date(DateToken),
    General,
    Meridiem,
}

/// Owned pattern or rendered text with its interpretation.
#[cfg(test)]
struct Piece {
    kind: Kind,
    text: String,
}

/// Scalar and lazy calendar observations for one section.
#[cfg(test)]
struct Context<'a> {
    value: GeneralValue<'a>,
    epoch: bool,
    sections: u8,
    date: Option<DateCode>,
    calendar: Calendar,
    twelve: bool,
}

#[cfg(test)]
impl Context<'_> {
    /// Numeric input supplied by the section selector.
    fn number(&self) -> f64 {
        if let GeneralValue::Number(n) = self.value {
            n
        } else {
            0.0
        }
    }
    /// Capture the first requested calendar, leaving later markers inert.
    fn date(&mut self, mode: Calendar) -> bool {
        if self.date.is_none() {
            self.calendar = mode;
            self.date = parse_date_code_with_calendar(self.number(), self.epoch, mode);
        }
        self.date.is_some()
    }
    /// Exact text-placeholder spelling, independent of General precision.
    fn text(&self) -> String {
        match self.value {
            GeneralValue::Number(n) => ryu_js::Buffer::new().format(n).to_owned(),
            _ => general(self.value),
        }
    }
}

/// Evaluate one selected section without choosing a section or resolving a table.
#[cfg(test)]
pub(super) fn eval_fmt(
    format: &str,
    value: GeneralValue<'_>,
    date1904: bool,
    sections: u8,
) -> Result<String, FormatError> {
    let mut context = Context {
        value,
        epoch: date1904,
        sections,
        date: None,
        calendar: Calendar::Gregorian,
        twelve: false,
    };
    let Some(mut pieces) = tokenize(format, &mut context)? else {
        return Ok(String::new());
    };
    let digits = resolve_time(&mut pieces, &mut context);
    for piece in &mut pieces {
        if piece.kind == Kind::Meridiem {
            piece.text = meridiem_label(&piece.text, &context);
            piece.kind = Kind::Text;
        }
        if let Kind::Date(kind) = piece.kind
            && let Some(date) = &context.date
        {
            piece.text = write_date(kind, &piece.text, date, digits)?;
            piece.kind = Kind::Text;
        }
    }
    render_numbers(&mut pieces, &context)?;
    Ok(pieces.into_iter().map(|p| p.text).collect())
}

/// Scan scalar boundaries while borrowing the original format spelling.
#[cfg(test)]
fn tokenize(format: &str, context: &mut Context<'_>) -> Result<Option<Vec<Piece>>, FormatError> {
    let mut pieces = Vec::new();
    let mut offset = 0;
    let mut previous = None;
    while offset < format.len() {
        let remaining = &format[offset..];
        let c = remaining
            .chars()
            .next()
            .ok_or(FormatError::UnterminatedString)?;
        let (mut kind, text, consumed) = if "BbMDYHSEmdyhsegAa上".contains(c) {
            let Some(field) = date_field(remaining, c, context) else {
                return Ok(None);
            };
            field
        } else if c == '.' && context.date.is_some() {
            let end = 1 + remaining[1..].bytes().take_while(|b| *b == b'0').count();
            (
                Kind::Date(DateToken::Second),
                remaining[..end].to_owned(),
                end,
            )
        } else if c == '[' {
            block(remaining, format, context)?
        } else {
            literal(remaining, c, context)?
        };
        if kind == Kind::Date(DateToken::Month) && previous == Some(DateToken::Hour24) {
            kind = Kind::Date(DateToken::Minute);
        }
        if let Kind::Date(token) = kind {
            previous = Some(token);
        }
        if "BbAa上".contains(c) {
            previous = None;
        }
        if c == '[' && kind == Kind::Date(DateToken::Elapsed) {
            previous = match text.chars().nth(1) {
                Some('h') => Some(DateToken::Hour24),
                Some('m') => Some(DateToken::Minute),
                _ => previous,
            };
        }
        if kind == Kind::Date(DateToken::Elapsed) && context.date.is_none() {
            return Ok(None);
        }
        offset += consumed;
        if c == '*' || (c == '[' && kind == Kind::Text && text.is_empty()) {
            continue;
        }
        pieces.push(Piece { kind, text });
    }
    Ok(Some(pieces))
}

/// Consume quoted, escaped, skipped, placeholder and numeric fragments.
#[cfg(test)]
fn literal(
    input: &str,
    c: char,
    context: &Context<'_>,
) -> Result<(Kind, String, usize), FormatError> {
    let width = c.len_utf8();
    let following = input[width..].chars().next();
    let operand = width + following.map_or(0, char::len_utf8);
    match c {
        '"' => {
            let end = input[1..]
                .find('"')
                .ok_or(FormatError::UnterminatedString)?
                + 1;
            Ok((Kind::Text, input[1..end].to_owned(), end + 1))
        }
        '\\' => Ok((
            if matches!(following, Some('(' | ')')) {
                Kind::Parenthesis
            } else {
                Kind::Text
            },
            following.map_or_else(String::new, |v| v.to_string()),
            operand,
        )),
        '_' => Ok((Kind::Text, " ".to_owned(), operand)),
        '*' => Ok((
            Kind::Text,
            String::new(),
            if matches!(following, Some(' ' | '*')) {
                operand
            } else {
                width
            },
        )),
        '@' => Ok((Kind::Text, context.text(), width)),
        'G' if is_general(input, 0) => Ok((Kind::General, String::new(), 7)),
        '0' | '#' | '.' | '?' | '1'..='9' => Ok(number_fragment(input, c)),
        ' ' => Ok((Kind::Space, " ".to_owned(), width)),
        '(' | ')' => Ok((
            if context.sections == 1 {
                Kind::Text
            } else {
                Kind::Parenthesis
            },
            c.to_string(),
            width,
        )),
        _ if ",$-+/():!^&'~{}<>=€acfijklopqrtuvwxzP".contains(c) => {
            Ok((Kind::Text, c.to_string(), width))
        }
        _ => Err(FormatError::UnrecognizedCharacter(c)),
    }
}

/// Discard metadata while preserving currency payloads for numeric formats.
#[cfg(test)]
fn block(
    input: &str,
    format: &str,
    context: &mut Context<'_>,
) -> Result<(Kind, String, usize), FormatError> {
    let end = input.find(']').ok_or(FormatError::UnterminatedBlock)? + 1;
    if is_elapsed(&input[..end]) {
        context.date(Calendar::Gregorian);
        return Ok((
            Kind::Date(DateToken::Elapsed),
            input[..end].to_ascii_lowercase(),
            end,
        ));
    }
    let text = if is_date(format) {
        ""
    } else {
        input[..end].split_once('$').map_or("", |(_, currency)| {
            currency
                .split(['-', '[', ']'])
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("$")
        })
    };
    Ok((Kind::Text, text.to_owned(), end))
}

/// Capture a calendar before interpreting a date run or meridiem label.
#[cfg(test)]
fn date_field(input: &str, c: char, context: &mut Context<'_>) -> Option<(Kind, String, usize)> {
    let lower = c.to_ascii_lowercase();
    if lower == 'b' && matches!(input.as_bytes().get(1), Some(b'1' | b'2')) {
        let mode = if input.as_bytes()[1] == b'2' {
            Calendar::Hijri
        } else {
            Calendar::Gregorian
        };
        return context.date(mode).then(|| (Kind::Text, String::new(), 2));
    }
    if matches!(c, 'A' | 'a' | '上') {
        return meridiem(input, c, context);
    }
    if !context.date(Calendar::Gregorian) {
        return None;
    }
    let width = input
        .chars()
        .take_while(|n| n.to_ascii_lowercase() == lower)
        .map(char::len_utf8)
        .sum();
    let kind = match lower {
        'b' => DateToken::BuddhistYear,
        'y' => DateToken::Year,
        'm' => DateToken::Month,
        'd' => DateToken::Day,
        'h' => DateToken::Hour24,
        's' => DateToken::Second,
        'e' => DateToken::Era,
        _ => return Some((Kind::Text, input[..width].to_owned(), width)),
    };
    Some((Kind::Date(kind), input[..width].to_ascii_lowercase(), width))
}

/// Capture the meridiem spelling for rendering after clock rounding.
#[cfg(test)]
fn meridiem(input: &str, c: char, context: &mut Context<'_>) -> Option<(Kind, String, usize)> {
    context.date(Calendar::Gregorian);
    let pattern = meridiem_pattern(input);
    let Some(pattern) = pattern else {
        return Some((Kind::Text, c.to_string(), c.len_utf8()));
    };
    context.date.as_ref()?;
    context.twelve = true;
    Some((
        Kind::Meridiem,
        input[..pattern.len()].to_owned(),
        pattern.len(),
    ))
}

/// Render the retained meridiem spelling from the carried clock.
#[cfg(test)]
fn meridiem_label(input: &str, context: &Context<'_>) -> String {
    let afternoon = context.date.as_ref().is_some_and(|d| d.hour >= 12);
    match meridiem_pattern(input) {
        Some("A/P") => input
            .chars()
            .nth(if afternoon { 2 } else { 0 })
            .map_or_else(String::new, |c| c.to_string()),
        Some("AM/PM") => if afternoon { "PM" } else { "AM" }.to_owned(),
        _ => if afternoon { "下午" } else { "上午" }.to_owned(),
    }
}

/// Resolve month/minute context and round all clock fields at one precision.
#[cfg(test)]
fn resolve_time(pieces: &mut [Piece], context: &mut Context<'_>) -> u8 {
    let (mut following, mut clock, mut digits) = (None, false, 0);
    for piece in pieces.iter_mut().rev() {
        if let Kind::Date(mut token) = piece.kind {
            if token == DateToken::Month && following == Some(DateToken::Second) {
                token = DateToken::Minute;
            }
            if matches!(
                token,
                DateToken::Hour24 | DateToken::Minute | DateToken::Second | DateToken::Elapsed
            ) {
                clock = true;
            }
            if token == DateToken::Second && piece.text.starts_with('.') {
                digits = digits.max(u8::try_from(piece.text.len() - 1).unwrap_or(u8::MAX));
            }
            if matches!(
                token,
                DateToken::Hour24
                    | DateToken::Minute
                    | DateToken::Second
                    | DateToken::Year
                    | DateToken::Day
                    | DateToken::Era
            ) {
                following = Some(token);
            }
            if token == DateToken::Hour24 && context.twelve {
                token = DateToken::Hour12;
            }
            piece.kind = Kind::Date(token);
        }
    }
    if clock {
        round_time(context, digits);
    }
    digits
}

/// Carry clock rounding through the same serial parser and captured options.
#[cfg(test)]
fn round_time(context: &mut Context<'_>, digits: u8) {
    let Some(date) = context.date.as_mut() else {
        return;
    };
    if digits <= 3 {
        let scale = 10_f64.powi(i32::from(digits));
        date.fraction = super::write_num::round(date.fraction * scale) / scale;
    }
    if date.fraction >= 1.0 {
        date.fraction = 0.0;
        date.second += 1;
    }
    if date.second >= 60 {
        date.second = 0;
        date.minute += 1;
    }
    if date.minute >= 60 {
        date.minute = 0;
        date.hour += 1;
    }
    if date.hour >= 24 {
        context.date = parse_date_code_with_calendar(
            f64::from(date.serial_day + 1),
            context.epoch,
            context.calendar,
        );
    }
}

/// Scan one numeric run without interpreting its rendered value.
#[cfg(test)]
fn number_fragment(input: &str, c: char) -> (Kind, String, usize) {
    let kind = match c {
        '?' => Kind::Question,
        '1'..='9' => Kind::Digits,
        _ => Kind::Number,
    };
    let end = input
        .bytes()
        .take_while(|b| match kind {
            Kind::Question => *b == b'?',
            Kind::Digits => b.is_ascii_digit(),
            _ => b"0#?.,E+-%".contains(b),
        })
        .count();
    (kind, input[..end].to_owned(), end)
}
