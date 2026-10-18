//! Checked decoding of binary expression tokens.
use super::super::super::blob::{ParseError, Result};
use super::super::{
    records::Payload,
    strings::{self, Encoding},
};
use super::types::{
    Axis, FunctionId, Ptg, PtgAttrSpace, PtgBinOp, PtgControl, PtgUnary, RgceArea, RgceLoc, SerAr,
    SpaceType,
};
/// Decode one counted token sequence and its ordered extra blocks.
pub(super) fn parse(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<Vec<Ptg>> {
    let count = usize::from(payload.u16()?);
    if count == 0xffff {
        payload.skip(payload.remaining())?;
        return Ok(Vec::new());
    }
    let stop = payload
        .remaining()
        .checked_sub(count)
        .ok_or(ParseError::Truncated)?;
    let mut tokens = Vec::new();
    while payload.remaining() > stop {
        #[cfg(test)]
        super::super::tests::TOKEN_WORK.with(|work| work.set(work.get() + 1));
        let raw = payload.u8()?;
        let id = match raw {
            0x58 | 0x78 => 0x22,
            0x40..=0x7d => (raw & 0x1f) | 0x20,
            _ => raw,
        };
        let token = if id < 0x20 {
            literal(id, payload, encoding)?
        } else {
            operand(id, payload, encoding)?
        };
        if let Some(token) = token {
            tokens.push(token);
        } else {
            payload.skip(
                payload
                    .remaining()
                    .checked_sub(stop)
                    .ok_or(ParseError::Truncated)?,
            )?;
        }
    }
    if payload.remaining() != stop {
        return Err(ParseError::Truncated);
    }
    if encoding == Encoding::Unicode && payload.remaining() > 0 {
        extra(payload, &mut tokens)?;
    }
    Ok(tokens)
}
/// Decode unclassified operators, literals and control attributes.
fn literal(id: u8, payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<Option<Ptg>> {
    Ok(Some(match id {
        1 => Ptg::Exp(crate::browser::ssf::CellAddress {
            r: u32::from(payload.u16()?),
            c: u32::from(payload.u16()?),
        }),
        3..=0x11 => Ptg::Binary(binary(id)),
        0x12 => Ptg::Unary(PtgUnary::Uplus),
        0x13 => Ptg::Unary(PtgUnary::Uminus),
        0x14 => Ptg::Unary(PtgUnary::Percent),
        0x15 => Ptg::Unary(PtgUnary::Paren),
        0x16 => Ptg::MissArg,
        0x17 => Ptg::Str(strings::short_string(payload, encoding)?),
        0x18 => return Err(ParseError::Invalid("unsupported formula operand")),
        0x19 => attribute(payload)?,
        0x1a => skip(payload, 10, PtgControl::Noop)?,
        0x1b => skip(payload, 4, PtgControl::Noop)?,
        0x1c => Ptg::Err(payload.u8()?),
        0x1d => Ptg::Bool(payload.u8()? != 0),
        0x1e => Ptg::Int(payload.u16()?),
        0x1f => Ptg::Num(payload.f64()?),
        _ => return Ok(None),
    }))
}
/// Select the binary operator without rebuilding a descriptor table.
fn binary(id: u8) -> PtgBinOp {
    match id {
        0x0f => PtgBinOp::Isect,
        0x10 => PtgBinOp::Union,
        0x11 => PtgBinOp::Range,
        _ => PtgBinOp::Scalar(
            [
                "+", "-", "*", "/", "^", "&", "<", "<=", "=", ">=", ">", "<>",
            ][usize::from(id - 3)],
        ),
    }
}
/// Decode the typed operand families shared by all three class encodings.
fn operand(id: u8, payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<Option<Ptg>> {
    Ok(Some(match id {
        0x20 => {
            payload.skip(7)?;
            Ptg::Array(Vec::new())
        }
        0x21 | 0x22 => function(id, payload)?,
        0x23 => Ptg::Name(name(payload, encoding)?),
        0x24 => Ptg::Ref(location(payload, encoding)?),
        0x25 => Ptg::Area(area(payload, encoding)?),
        0x26 => skip(payload, 6, PtgControl::MemArea)?,
        0x29 | 0x2e | 0x2f => skip(payload, 2, PtgControl::Noop)?,
        0x2c => {
            let mut loc = location(payload, encoding)?;
            relative(&mut loc, encoding);
            Ptg::RefN(loc)
        }
        0x2d => Ptg::AreaN(area(payload, encoding)?),
        0x2a | 0x2b | 0x3c | 0x3d => reference_error(id, payload, encoding)?,
        0x39 => {
            let book = i16::from_le_bytes(payload.u16()?.to_le_bytes());
            if encoding != Encoding::Unicode {
                payload.skip(8)?;
            }
            Ptg::NameX(book, name(payload, encoding)?)
        }
        0x3a | 0x3b => {
            let sheet = payload.u16()?;
            if encoding != Encoding::Unicode {
                payload.skip(12)?;
            }
            if id == 0x3a {
                Ptg::Ref3d(sheet, location(payload, encoding)?)
            } else {
                Ptg::Area3d(sheet, area(payload, encoding)?)
            }
        }
        _ => return Ok(None),
    }))
}
/// Read version-specific name indices and their unused trailing framing.
fn name(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<u32> {
    if encoding == Encoding::Unicode {
        return payload.u32();
    }
    let index = u32::from(payload.u16()?);
    payload.skip(12)?;
    Ok(index)
}
/// Consume one fixed-size nonprinting control token.
fn skip(payload: &mut Payload<'_, '_>, count: usize, control: PtgControl) -> Result<Ptg> {
    payload.skip(count)?;
    Ok(Ptg::Control(control))
}
/// Decode function identity and argument count through the one descriptor owner.
fn function(id: u8, payload: &mut Payload<'_, '_>) -> Result<Ptg> {
    if id == 0x21 {
        let function = FunctionId::Worksheet(payload.u16()?);
        let (_, argc) =
            super::functions::function(function).ok_or(ParseError::Invalid("function"))?;
        return Ok(Ptg::Func(function, argc.unwrap_or(0)));
    }
    let argc = payload.u8()? & 0x7f;
    let id = payload.u16()?;
    let function = if id & 0x8000 == 0 {
        FunctionId::Worksheet(id)
    } else {
        FunctionId::Command(id & 0x7fff)
    };
    Ok(Ptg::Func(function, argc))
}
/// Consume the version-specific payload of an error reference.
fn reference_error(id: u8, payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<Ptg> {
    let length = match (id, encoding == Encoding::Unicode) {
        (0x2a, true) => 4,
        (0x2a, false) => 3,
        (0x2b, true) => 8,
        (0x2b, false) | (0x3c, true) => 6,
        (0x3c, false) => 17,
        (_, true) => 10,
        (_, false) => 20,
    };
    payload.skip(length)?;
    Ok(Ptg::RefErr)
}
/// Consume extra blocks once, in their token order.
fn extra(payload: &mut Payload<'_, '_>, tokens: &mut [Ptg]) -> Result<()> {
    for token in tokens {
        match token {
            Ptg::Array(rows) => *rows = array(payload)?,
            Ptg::Control(PtgControl::MemArea) => {
                let count = usize::from(payload.u16()?);
                payload.skip(count * 8)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Read a location whose flags occupy the column in newer workbooks.
fn location(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<RgceLoc> {
    let row = payload.u16()?;
    let wide = encoding == Encoding::Unicode;
    let column = if wide {
        payload.u16()?
    } else {
        u16::from(payload.u8()?)
    };
    Ok(coordinate(row, column, wide))
}

/// Read area endpoints in their row-row-column-column storage order.
fn area(payload: &mut Payload<'_, '_>, encoding: Encoding) -> Result<RgceArea> {
    let first = payload.u16()?;
    let last = payload.u16()?;
    let wide = encoding == Encoding::Unicode;
    let (c, end_c) = if wide {
        (payload.u16()?, payload.u16()?)
    } else {
        (u16::from(payload.u8()?), u16::from(payload.u8()?))
    };
    Ok(RgceArea {
        s: coordinate(first, c, wide),
        e: coordinate(last, end_c, wide),
    })
}
/// Split stored coordinates from the independent fixed-axis flags.
fn coordinate(row: u16, column: u16, wide: bool) -> RgceLoc {
    let flags = if wide { column } else { row };
    let r = if wide { row } else { row & 0x3fff };
    let c = if wide { column & 0x3fff } else { column };
    RgceLoc {
        r: if flags & 0x8000 == 0 {
            Axis::Absolute(u32::from(r))
        } else {
            Axis::Relative(i32::from(r))
        },
        c: if flags & 0x4000 == 0 {
            Axis::Absolute(u32::from(c))
        } else {
            Axis::Relative(i32::from(c))
        },
    }
}

/// Decode signed relative fields without changing absolute axes.
fn relative(loc: &mut RgceLoc, encoding: Encoding) {
    let (row_sign, row_modulus, column_sign, column_modulus) = if encoding == Encoding::Unicode {
        (0x0008_0000, 0x0010_0000, 0x2000, 0x4000)
    } else {
        (0x2000, 0x4000, 0x80, 0x100)
    };
    for (axis, sign, modulus) in [
        (&mut loc.r, row_sign, row_modulus),
        (&mut loc.c, column_sign, column_modulus),
    ] {
        if let Axis::Relative(value) = axis
            && *value >= sign
        {
            *value -= modulus;
        }
    }
}

/// Consume one attribute's jump or optimization framing.
fn attribute(payload: &mut Payload<'_, '_>) -> Result<Ptg> {
    let kind = payload.u8()?;
    if matches!(kind, 0x40 | 0x41) {
        let kind = match payload.u8()? {
            0 => SpaceType::BeforeSpace,
            1 => SpaceType::BeforeReturn,
            2 => SpaceType::BeforeParenSpace,
            3 => SpaceType::BeforeParenReturn,
            4 => SpaceType::AfterSpace,
            5 => SpaceType::AfterReturn,
            _ => SpaceType::Ignored,
        };
        return Ok(Ptg::Space(PtgAttrSpace {
            kind,
            count: payload.u8()?,
        }));
    }
    let value = payload.u16()?;
    if kind == 4 {
        payload.skip((usize::from(value) + 1) * 2)?;
    }
    Ok(Ptg::Control(match kind {
        8 => PtgControl::Goto,
        0x10 => PtgControl::Sum,
        _ => PtgControl::Noop,
    }))
}

/// Decode a counted row-major constant matrix after the token stream.
fn array(payload: &mut Payload<'_, '_>) -> Result<Vec<Vec<SerAr>>> {
    let columns = usize::from(payload.u8()?) + 1;
    let rows = usize::from(payload.u16()?) + 1;
    (0..rows)
        .map(|_| (0..columns).map(|_| scalar(payload)).collect())
        .collect()
}
/// Decode a matrix scalar without changing its context-specific spelling.
fn scalar(payload: &mut Payload<'_, '_>) -> Result<SerAr> {
    Ok(match payload.u8()? {
        0 => {
            payload.skip(8)?;
            SerAr::Nil
        }
        1 => SerAr::Num(payload.f64()?),
        2 => SerAr::Str(strings::unicode_string(payload, Encoding::Unicode)?),
        4 => {
            let value = payload.u8()? != 0;
            payload.skip(7)?;
            SerAr::Bool(value)
        }
        0x10 | 0x25 => {
            let code = payload.u8()?;
            payload.skip(7)?;
            SerAr::Err(code)
        }
        _ => return Err(ParseError::Invalid("array scalar")),
    })
}
