//! Assembly of owned expression text from typed binary operands.
use super::super::super::blob::{ParseError, Result};
use super::types::{
    Axis, FunctionId, Ptg, PtgAttrSpace, PtgBinOp, PtgControl, PtgUnary, RgceArea, RgceLoc, SerAr,
    SpaceType,
};
use crate::browser::ssf::{
    CellAddress, CellValue, encode_col, format_cell::error_label, write_num::spelling,
};
/// Assemble an expression without its leading equals sign.
pub(super) fn stringify(
    tokens: &[Ptg],
    position: CellAddress,
    books: &super::SupBooks,
    encoding: super::super::strings::Encoding,
) -> Result<String> {
    let mut stack = Vec::new();
    let mut space = None;
    for token in tokens {
        let text = match token {
            Ptg::Space(value) => {
                space = Some(*value);
                continue;
            }
            Ptg::Control(PtgControl::Goto) => continue,
            Ptg::Control(PtgControl::Sum) => format!("SUM({})", pop(&mut stack)?),
            Ptg::Control(_) => {
                apply_space(&mut stack, space.take())?;
                continue;
            }
            Ptg::Binary(op) => {
                binary(&mut stack, *op, &mut space)?;
                apply_space(&mut stack, space.take())?;
                continue;
            }
            Ptg::Unary(op) => {
                let operand = pop(&mut stack)?;
                unary(op, &operand, &mut space)
            }
            Ptg::Func(id, count) => function(&mut stack, *id, *count)?,
            _ => literal(token, position, books, encoding)?,
        };
        stack.push(text);
        apply_space(&mut stack, space.take())?;
    }
    let result = stack.into_iter().next().unwrap_or_default();
    Ok(match result.as_str() {
        "TRUE" => "true".to_owned(),
        "FALSE" => "false".to_owned(),
        _ => result,
    })
}
/// Remove one operand while retaining typed failures for malformed stacks.
fn pop(stack: &mut Vec<String>) -> Result<String> {
    stack.pop().ok_or(ParseError::Invalid("formula stack"))
}
/// Combine two operands in their source order.
fn binary(stack: &mut Vec<String>, op: PtgBinOp, space: &mut Option<PtgAttrSpace>) -> Result<()> {
    let mut right = pop(stack)?;
    let mut left = pop(stack)?;
    if matches!(op, PtgBinOp::Scalar(_))
        && let Some(space) = space.take()
        && matches!(space.kind, SpaceType::BeforeSpace | SpaceType::BeforeReturn)
    {
        left.push_str(&whitespace(space));
    }
    if matches!(op, PtgBinOp::Range)
        && let (Some((first, _)), Some((last, _))) = (left.rsplit_once('!'), right.rsplit_once('!'))
        && first.to_lowercase() == last.to_lowercase()
    {
        let start = right.rfind('!').map_or(0, |i| i + 1);
        right.drain(..start);
    }
    let separator = match op {
        PtgBinOp::Scalar(s) => s,
        PtgBinOp::Isect => " ",
        PtgBinOp::Union => ",",
        PtgBinOp::Range => ":",
    };
    stack.push(format!("{left}{separator}{right}"));
    Ok(())
}
/// Render independent absolute-axis markers.
fn reference(loc: &RgceLoc) -> String {
    let (r, row_fixed) = axis(loc.r);
    let (c, column_fixed) = axis(loc.c);
    format!("{column_fixed}{}{row_fixed}{}", encode_col(c), r + 1)
}
/// Select coordinate and marker without changing its binary identity.
fn axis(axis: Axis) -> (u32, &'static str) {
    match axis {
        Axis::Absolute(n) => (n, "$"),
        Axis::Relative(n) => (n.unsigned_abs(), ""),
    }
}

/// Render whole-row ranges without redundant column endpoints.
fn area_text(area: &RgceArea) -> String {
    if matches!(area.s.c, Axis::Absolute(0)) && matches!(area.e.c, Axis::Absolute(255)) {
        let (first, fixed) = axis(area.s.r);
        let (last, end_fixed) = axis(area.e.r);
        return format!("{fixed}{}:{end_fixed}{}", first + 1, last + 1);
    }
    format!("{}:{}", reference(&area.s), reference(&area.e))
}

/// Apply a consuming cell only to relative axes, then wrap to the binary grid.
fn shifted_reference(loc: &RgceLoc, position: CellAddress) -> String {
    let shifted = RgceLoc {
        r: shifted_axis(loc.r, position.r, 65536),
        c: shifted_axis(loc.c, position.c, 256),
    };
    reference(&shifted)
}
/// Resolve one signed displacement into its finite grid.
fn shifted_axis(axis: Axis, origin: u32, modulus: i64) -> Axis {
    match axis {
        Axis::Absolute(n) => Axis::Absolute(n),
        Axis::Relative(n) => Axis::Relative(
            i32::try_from((i64::from(n) + i64::from(origin)).rem_euclid(modulus))
                .unwrap_or_default(),
        ),
    }
}

/// Materialize only the whitespace character selected by the attribute.
fn whitespace(space: PtgAttrSpace) -> String {
    match space.kind {
        SpaceType::BeforeReturn | SpaceType::BeforeParenReturn | SpaceType::AfterReturn => "\r",
        SpaceType::Ignored => "",
        _ => " ",
    }
    .repeat(usize::from(space.count))
}
/// Consume deferred whitespace at the next ordinary display boundary.
fn apply_space(stack: &mut Vec<String>, space: Option<PtgAttrSpace>) -> Result<()> {
    if let Some(space) = space {
        let operand = pop(stack)?;
        let text = whitespace(space);
        let text = match space.kind {
            SpaceType::AfterSpace | SpaceType::AfterReturn => format!("{operand}{text}"),
            SpaceType::BeforeSpace | SpaceType::BeforeReturn => format!("{text}{operand}"),
            _ => operand,
        };
        stack.push(text);
    }
    Ok(())
}
/// Parentheses consume their own four explicit whitespace placements.
fn unary(op: &PtgUnary, operand: &str, space: &mut Option<PtgAttrSpace>) -> String {
    match op {
        PtgUnary::Uplus => format!("+{operand}"),
        PtgUnary::Uminus => format!("-{operand}"),
        PtgUnary::Percent => format!("{operand}%"),
        PtgUnary::Paren => {
            let (mut left, mut right) = (String::new(), String::new());
            if let Some(space) = space.take() {
                match space.kind {
                    SpaceType::BeforeParenSpace | SpaceType::BeforeParenReturn => {
                        left = whitespace(space);
                    }
                    SpaceType::AfterSpace | SpaceType::AfterReturn => right = whitespace(space),
                    _ => {}
                }
            }
            format!("{left}({operand}{right})")
        }
    }
}

/// Serialize a constant matrix with the binary grammar's row separators.
fn array_text(rows: &[Vec<SerAr>]) -> String {
    let rows: Vec<_> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| match value {
                    SerAr::Nil => String::new(),
                    SerAr::Num(n) => spelling(*n),
                    SerAr::Str(text) => format!("\"{}\"", text.replace('"', "\"\"")),
                    SerAr::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
                    SerAr::Err(code) => error_label(&CellValue::Number(f64::from(*code)))
                        .unwrap_or("")
                        .to_owned(),
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();
    format!("{{{}}}", rows.join(";"))
}

/// Render a scalar or coordinate operand without modifying expression state.
fn literal(
    token: &Ptg,
    position: CellAddress,
    books: &super::SupBooks,
    encoding: super::super::strings::Encoding,
) -> Result<String> {
    Ok(match token {
        Ptg::MissArg => String::new(),
        Ptg::Exp(anchor) => format!("{},{}", anchor.r, anchor.c),
        Ptg::NameX(book, name) => books.external(*book, *name, encoding)?,
        Ptg::Ref3d(index, loc) => {
            format!("{}!{}", books.sheet(*index, encoding)?, reference(loc))
        }
        Ptg::Area3d(index, area) => {
            format!("{}!{}", books.sheet(*index, encoding)?, area_text(area))
        }
        Ptg::Name(index) => books.name(*index),
        Ptg::Array(rows) => array_text(rows),
        Ptg::RefN(loc) => shifted_reference(loc, position),
        Ptg::AreaN(area) => format!(
            "{}:{}",
            shifted_reference(&area.s, position),
            shifted_reference(&area.e, position)
        ),
        Ptg::Area(area) => area_text(area),
        Ptg::RefErr => "#REF!".to_owned(),
        Ptg::Ref(loc) => reference(loc),
        Ptg::Int(n) => n.to_string(),
        Ptg::Num(n) => spelling(*n),
        Ptg::Str(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Ptg::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        Ptg::Err(code) => error_label(&CellValue::Number(f64::from(*code)))
            .unwrap_or("")
            .to_owned(),
        _ => return Err(ParseError::Invalid("formula operand")),
    })
}
/// Remove a function's arguments from the stack in their original order.
fn function(stack: &mut Vec<String>, id: FunctionId, count: u8) -> Result<String> {
    let (name, _) = super::functions::function(id).ok_or(ParseError::Invalid("function"))?;
    let start = stack
        .len()
        .checked_sub(usize::from(count))
        .ok_or(ParseError::Invalid("function arguments"))?;
    let mut args: Vec<_> = stack.drain(start..).collect();
    let name = if name == "User" && !args.is_empty() {
        args.remove(0)
    } else {
        name.to_owned()
    };
    Ok(format!("{name}({})", args.join(",")))
}
