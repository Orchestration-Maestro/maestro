//! Address and hostname acceptance with internationalized label context.

use super::matches;

/// Check unquoted ASCII mailbox and domain grammar.
pub(super) fn email(value: &str) -> bool {
    matches(
        r"^(?!.*\.\.)[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*@[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*$",
        value,
        "i",
    )
}

/// Check unquoted mailbox grammar with Unicode letters and numbers.
pub(super) fn idn_email(value: &str) -> bool {
    matches(
        r"^(?!.*\.\.)[\p{L}\p{N}!#$%&'*+/=?^_`{|}~-]+(?:\.[\p{L}\p{N}!#$%&'*+/=?^_`{|}~-]+)*@[\p{L}\p{N}](?:[\p{L}\p{N}-]{0,61}[\p{L}\p{N}])?(?:\.[\p{L}\p{N}](?:[\p{L}\p{N}-]{0,61}[\p{L}\p{N}])?)*$",
        value,
        "iu",
    )
}

/// Check ASCII labels and whole-name limits, including contextual punycode.
pub(super) fn hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && !value.ends_with('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.is_ascii()
                && if label.to_ascii_lowercase().starts_with("xn--") {
                    international_label(label)
                } else {
                    matches(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$", label, "i")
                        && label.as_bytes().get(2..4) != Some(b"--")
                }
        })
}

/// Normalize internationalized names before checking label context and lengths.
pub(super) fn idn_hostname(value: &str) -> bool {
    let normalized = icu_normalizer::ComposingNormalizer::new_nfc().normalize(value);
    let canonical = normalized.replace(['\u{3002}', '\u{ff0e}', '\u{ff61}'], ".");
    !canonical.is_empty()
        && canonical.encode_utf16().count() <= 253
        && canonical.split('.').all(international_label)
}

/// Combine strict native IDNA acceptance with decoded-label context.
fn international_label(value: &str) -> bool {
    if value.is_empty()
        || value.encode_utf16().count() > 63
        || idna::domain_to_ascii_strict(value).is_err()
    {
        return false;
    }
    if value.to_ascii_lowercase().starts_with("xn--") {
        value
            .get(4..)
            .and_then(idna::punycode::decode_to_string)
            .is_some_and(|unicode| context(&unicode))
    } else {
        context(value)
    }
}

/// Check label-wide script, mark, hyphen and mixed-digit constraints.
fn context(label: &str) -> bool {
    let characters: Vec<_> = label.chars().collect();
    let Some(first) = characters.first() else {
        return false;
    };
    if matches(r"^\p{M}", &first.to_string(), "u")
        || first == &'-'
        || characters.last() == Some(&'-')
        || characters.get(2..4) == Some(&['-', '-'])
    {
        return false;
    }
    let has_japanese = matches(
        r"[\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Han}]",
        label,
        "u",
    );
    let arabic = characters
        .iter()
        .any(|character| ('\u{660}'..='\u{669}').contains(character));
    let extended = characters
        .iter()
        .any(|character| ('\u{6f0}'..='\u{6f9}').contains(character));
    if (arabic && extended) || (label.contains('\u{30fb}') && !has_japanese) {
        return false;
    }
    characters.iter().enumerate().all(|(index, character)| {
        let previous = index
            .checked_sub(1)
            .and_then(|index| characters.get(index))
            .copied();
        let next = characters.get(index + 1).copied();
        character_context(*character, previous, next)
    })
}

/// Check neighboring characters for contextual punctuation and joiners.
fn character_context(character: char, previous: Option<char>, next: Option<char>) -> bool {
    match character {
        '\u{b7}' => previous == Some('l') && next == Some('l'),
        '\u{375}' => next.is_some_and(|next| matches(r"\p{Script=Greek}", &next.to_string(), "u")),
        '\u{5f3}' | '\u{5f4}' => previous
            .is_some_and(|previous| matches(r"\p{Script=Hebrew}", &previous.to_string(), "u")),
        '\u{200d}' => previous.is_some_and(virama),
        '\u{640}' | '\u{7fa}' | '\u{302e}' | '\u{302f}' | '\u{3031}'..='\u{3035}' | '\u{303b}' => {
            false
        }
        _ => true,
    }
}

/// Recognize permitted preceding marks for a zero-width joiner.
fn virama(character: char) -> bool {
    matches!(
        character,
        '\u{94d}'
            | '\u{9cd}'
            | '\u{a4d}'
            | '\u{acd}'
            | '\u{b4d}'
            | '\u{bcd}'
            | '\u{c4d}'
            | '\u{ccd}'
            | '\u{d3b}'
            | '\u{d3c}'
            | '\u{d4d}'
            | '\u{dca}'
            | '\u{1b44}'
            | '\u{1baa}'
            | '\u{1bab}'
            | '\u{a9c0}'
            | '\u{11046}'
            | '\u{1107f}'
            | '\u{110b9}'
            | '\u{11133}'
            | '\u{11134}'
            | '\u{111c0}'
            | '\u{11235}'
            | '\u{1134d}'
            | '\u{11442}'
            | '\u{114c2}'
            | '\u{115bf}'
            | '\u{1163f}'
            | '\u{116b6}'
            | '\u{11c3f}'
            | '\u{11d44}'
            | '\u{11d45}'
    )
}
