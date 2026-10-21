//! Bare addresses admitted only at reached native inline cursors.
use crate::MarkedOptions;
use regress::Regex;
use std::sync::LazyLock;
/// Email local-part characters shared by admission and failed-run consumption.
macro_rules! email_local_part {
    () => {
        "[A-Za-z0-9._+-]"
    };
}

/// Web, earlier-grammar web and email candidate sources with their flags; earlier caller
/// grammars fold the complete protocol and web prefix, and emails are scanned
/// independently of rejected web spans.
const SOURCES: [(&str, &str); 3] = [
    (
        r"(?:(?:[hH][tT][tT][pP][sS]?|[fF][tT][pP])://|www\.)[A-Za-z0-9-]+[^\s<]*",
        "",
    ),
    (r"(?:(?:https?|ftp)://|www\.)[A-Za-z0-9-]+[^\s<]*", "i"),
    (
        concat!(
            email_local_part!(),
            r"+(@)[a-zA-Z0-9-_]+(?:\.[a-zA-Z0-9-_]*[a-zA-Z0-9])+(?![-_])"
        ),
        "",
    ),
];
/// Anchored web and email rules, with a shared failed local-part run alternative.
static START: LazyLock<[Option<Regex>; 3]> = LazyLock::new(|| {
    std::array::from_fn(|index| {
        let (source, flags) = SOURCES[index];
        let source = if index == 2 {
            format!("({source})|{}+", email_local_part!())
        } else {
            source.to_owned()
        };
        Regex::with_flags(&format!("^(?:{source})"), flags).ok()
    })
});
/// Reuse the selected URL suffix grammar.
static BACKPEDAL: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?:[^?!.,:;*_'"~()&]+|\([^)]*\)|&(?![a-zA-Z0-9]+;$)|[?!.,:;*_'"~)]+(?!$))+"#).ok()
});
/// Admit one bare address at the reached cursor, caching failed email-local runs.
pub(crate) fn address(
    text: &str,
    options: MarkedOptions,
    cursor: usize,
    email_skip: &mut usize,
) -> Option<(usize, String)> {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.address_checks += 1);
    let web = usize::from(matches!(
        options,
        MarkedOptions::Terminal | MarkedOptions::Export
    ));
    if let Some(matched) = START[web].as_ref().and_then(|rule| rule.find(text)) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.address_bytes += matched.end());
        return Some(accepted(&text[matched.range()], false));
    }
    if cursor < *email_skip {
        return None;
    }
    let matched = START[2].as_ref()?.find(text)?;
    if matched.group(1).is_some() {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.address_bytes += matched.end());
        return Some(accepted(&text[matched.range()], true));
    }
    *email_skip = cursor + matched.end();
    None
}
/// Retain an admitted candidate's label length and authored destination.
fn accepted(authored: &str, email: bool) -> (usize, String) {
    let label = if email { authored } else { suffix(authored) };
    let href = if email {
        format!("mailto:{label}")
    } else if label.starts_with("www.") {
        format!("http://{label}")
    } else {
        label.to_owned()
    };
    (label.len(), href)
}
/// Backpedal repeatedly until the grammar accepts the complete remaining URL prefix.
fn suffix(candidate: &str) -> &str {
    let mut label = candidate;
    loop {
        let next = BACKPEDAL
            .as_ref()
            .and_then(|rule| rule.find(label))
            .map_or("", |matched| &label[matched.range()]);
        if next == label {
            return label;
        }
        label = next;
    }
}
