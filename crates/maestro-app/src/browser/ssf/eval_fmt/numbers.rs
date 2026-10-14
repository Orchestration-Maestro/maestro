//! Numeric field joining and redistribution.

#[cfg(test)]
use super::{Context, FormatError, Kind, NumberToken, Piece, general, write_num};

/// Render numeric fields through the shared writer.
#[cfg(test)]
pub(super) fn render_numbers(
    pieces: &mut [Piece],
    context: &Context<'_>,
) -> Result<(), FormatError> {
    let pattern = join_numbers(pieces, context);
    if pattern.is_empty() {
        return Ok(());
    }
    let mut shown_value = context.number();
    if shown_value < 0.0 && context.sections > 1 {
        shown_value = -shown_value;
    }
    let mut rendered = write_num(NumberToken::Number, &pattern, shown_value)?;
    let sign_moved = shown_value < 0.0
        && context.sections == 1
        && pieces.first().is_some_and(|p| p.kind == Kind::Text)
        && rendered.starts_with('-');
    if sign_moved {
        shown_value = -shown_value;
        rendered.remove(0);
        pieces[0].text.insert(0, '-');
    }
    let decimal = pieces
        .iter()
        .position(|p| numeric(p.kind) && p.text.contains('.'));
    if !rendered.contains('E') {
        if let Some(index) = decimal {
            distribute_decimal(pieces, index, &rendered);
        } else {
            distribute_integer(pieces, &rendered);
        }
    }
    for piece in pieces {
        if numeric(piece.kind) {
            piece.text = write_num(NumberToken::Number, &piece.text, shown_value)?;
            piece.kind = Kind::Text;
        }
    }
    Ok(())
}

/// Identify unrendered numeric placeholders.
#[cfg(test)]
fn numeric(kind: Kind) -> bool {
    matches!(kind, Kind::Number | Kind::Question)
}

/// Join fraction fragments before sharing one numeric writer result.
#[cfg(test)]
fn join_numbers(pieces: &mut [Piece], context: &Context<'_>) -> String {
    let mut pattern = String::new();
    for index in 0..pieces.len() {
        if pieces[index].kind == Kind::General {
            pieces[index].text = general(context.value);
            pieces[index].kind = Kind::Text;
        }
        if !numeric(pieces[index].kind) {
            continue;
        }
        let (left, right) = pieces.split_at_mut(index + 1);
        let mut fraction = left[index].text.contains('/');
        let count = right
            .iter()
            .enumerate()
            .take_while(|(i, p)| {
                fraction |= p.kind == Kind::Text && p.text == "/";
                let next = right.get(i + 1);
                matches!(p.kind, Kind::Question | Kind::Digits)
                    || (p.kind == Kind::Text && p.text == "/")
                    || (matches!(p.kind, Kind::Text | Kind::Space)
                        && next.is_some_and(|n| {
                            n.kind == Kind::Question
                                || (n.kind == Kind::Text && n.text == "/")
                                || (fraction && p.text == " " && n.kind == Kind::Digits)
                        }))
            })
            .count();
        for part in &mut right[..count] {
            left[index].text.push_str(&std::mem::take(&mut part.text));
            part.kind = Kind::Text;
        }
        pattern.push_str(&left[index].text);
    }
    pattern
}

/// Allocate a writer's integer text from the last field towards the first.
#[cfg(test)]
fn distribute_integer(pieces: &mut [Piece], rendered: &str) {
    let (mut remaining, mut first) = (rendered, None);
    for (index, piece) in pieces.iter_mut().enumerate().rev() {
        if !numeric(piece.kind) {
            continue;
        }
        let count = piece.text.len().min(remaining.len());
        let boundary = remaining.len() - count;
        piece.text = remaining[boundary..].to_owned();
        piece.kind = Kind::Text;
        remaining = &remaining[..boundary];
        first = Some(index);
    }
    if let Some(index) = first {
        pieces[index].text.insert_str(0, remaining);
    }
}

/// Distribute integer and fractional digits around literal interruptions.
#[cfg(test)]
fn distribute_decimal(pieces: &mut [Piece], decimal: usize, rendered: &str) {
    let point = rendered.find('.');
    let (mut integer, mut fraction) =
        point.map_or(("", rendered), |p| (&rendered[..p], &rendered[p + 1..]));
    let mut first = None;
    for (index, piece) in pieces[..=decimal].iter_mut().enumerate().rev() {
        if !numeric(piece.kind) {
            continue;
        }
        let prefix = if index == decimal {
            piece.text.find('.').unwrap_or(piece.text.len())
        } else {
            piece.text.len()
        };
        let mut text = String::new();
        for mask in piece.text[..prefix].chars().rev() {
            if matches!(mask, '0' | '#')
                && let Some(last) = integer.chars().next_back()
            {
                text.insert(0, last);
                integer = &integer[..integer.len() - last.len_utf8()];
            }
        }
        text.push_str(&piece.text[prefix..]);
        piece.text = text;
        piece.kind = Kind::Text;
        first = Some(index);
    }
    if let Some(index) = first {
        pieces[index].text.insert_str(0, integer);
    }
    for (index, piece) in pieces.iter_mut().enumerate().skip(decimal) {
        if index != decimal && !numeric(piece.kind) {
            continue;
        }
        let prefix = if index == decimal {
            piece.text.find('.').map_or(0, |p| p + 1)
        } else {
            0
        };
        let count = (piece.text.len() - prefix).min(fraction.len());
        piece.text.truncate(prefix);
        piece.text.push_str(&fraction[..count]);
        fraction = &fraction[count..];
        piece.kind = Kind::Text;
    }
}
