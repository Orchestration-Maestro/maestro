//! Shared scalar text semantics for replay and argument coercion.

pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(|c| {
        matches!(c,
            '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
            '\u{205f}' | '\u{3000}' | '\u{feff}'
        )
    })
}

pub(crate) fn number_string(number: &serde_json::Number) -> Option<String> {
    let value = number.as_f64()?;
    if value == 0.0 {
        return Some("0".into());
    }
    let decimal = serde_json::Number::from_f64(value)?.to_string();
    let (sign, unsigned) = if let Some(unsigned) = decimal.strip_prefix('-') {
        ("-", unsigned)
    } else {
        ("", decimal.as_str())
    };
    let (mantissa, exponent) = unsigned.split_once('e').unwrap_or((unsigned, "0"));
    let exponent: i32 = exponent.parse().ok()?;
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let leading = digits.len() - digits.trim_start_matches('0').len();
    let digits = digits.trim_start_matches('0').trim_end_matches('0');
    let position = point + exponent - leading as i32;
    let length = digits.len() as i32;
    let rendered = if position > 0 && position <= 21 {
        if position >= length {
            format!("{digits}{}", "0".repeat((position - length) as usize))
        } else {
            let (whole, fraction) = digits.split_at(position as usize);
            format!("{whole}.{fraction}")
        }
    } else if position > -6 && position <= 0 {
        format!("0.{}{digits}", "0".repeat((-position) as usize))
    } else {
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        format!("{first}{fraction}e{:+}", position - 1)
    };
    Some(format!("{sign}{rendered}"))
}
