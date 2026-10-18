//! Typed operands retained after binary framing has been consumed.
/// One decoded formula operand.
pub(in super::super) enum Ptg {
    /// Explicit omitted argument.
    MissArg,
    /// Shared or array anchor address.
    Exp(crate::browser::ssf::CellAddress),
    /// Book-local external name.
    NameX(i16, u32),
    /// Sheet-qualified cell reference.
    Ref3d(u16, RgceLoc),
    /// Sheet-qualified area reference.
    Area3d(u16, RgceArea),
    /// Defined-name index.
    Name(u32),
    /// Constant matrix in row-major order.
    Array(Vec<Vec<SerAr>>),
    /// Deferred literal whitespace.
    Space(PtgAttrSpace),
    /// Nonprinting framing or optimized sum.
    Control(PtgControl),
    /// Function with an explicit argument count.
    Func(FunctionId, u8),
    /// Prefix or postfix operator.
    Unary(PtgUnary),
    /// Binary operator.
    Binary(PtgBinOp),
    /// Area relative to the consuming formula cell.
    AreaN(RgceArea),
    /// Cell relative to the consuming formula cell.
    RefN(RgceLoc),
    /// Area reference.
    Area(RgceArea),
    /// Invalid cell or area reference.
    RefErr,
    /// Cell reference.
    Ref(RgceLoc),
    /// Unsigned integer literal.
    Int(u16),
    /// Binary64 literal.
    Num(f64),
    /// Quoted Unicode literal.
    Str(String),
    /// Boolean literal.
    Bool(bool),
    /// Stored error code.
    Err(u8),
}

/// Binary operator in reverse-Polish expression order.
#[derive(Clone, Copy)]
pub(in super::super) enum PtgBinOp {
    /// Arithmetic, comparison or concatenation operator.
    Scalar(&'static str),
    /// Reference intersection.
    Isect,
    /// Reference union.
    Union,
    /// Reference range.
    Range,
}
/// Independently fixed or relative coordinate component.
#[derive(Clone, Copy)]
pub(in super::super) enum Axis {
    /// Fixed grid coordinate.
    Absolute(u32),
    /// Relative grid coordinate or signed displacement.
    Relative(i32),
}
/// A binary cell location with independently selected axes.
pub(in super::super) struct RgceLoc {
    /// Row axis.
    pub(in super::super) r: Axis,
    /// Column axis.
    pub(in super::super) c: Axis,
}

/// Single-operand expression delimiters.
pub(in super::super) enum PtgUnary {
    /// Unary positive sign.
    Uplus,
    /// Unary negative sign.
    Uminus,
    /// Percent suffix.
    Percent,
    /// Explicit parentheses.
    Paren,
}

/// Select one immutable function descriptor table.
#[derive(Clone, Copy)]
pub(in super::super) enum FunctionId {
    /// Worksheet function.
    Worksheet(u16),
    /// Macro command function.
    Command(u16),
}

/// Inclusive coordinate endpoints of a binary area operand.
pub(in super::super) struct RgceArea {
    /// First endpoint.
    pub(super) s: RgceLoc,
    /// Last endpoint.
    pub(super) e: RgceLoc,
}

/// Controls that survive parsing for extra-data framing or expression assembly.
pub(in super::super) enum PtgControl {
    /// Nonprinting token.
    Noop,
    /// Deferred-spacing jump.
    Goto,
    /// Optimized one-argument sum.
    Sum,
    /// Memory operand with an extra-data rectangle list.
    MemArea,
}

/// Binary whitespace positioning policy.
#[derive(Clone, Copy)]
pub(in super::super) enum SpaceType {
    /// Prefix spaces.
    BeforeSpace,
    /// Prefix carriage returns.
    BeforeReturn,
    /// Spaces before the opening parenthesis.
    BeforeParenSpace,
    /// Carriage returns before the opening parenthesis.
    BeforeParenReturn,
    /// Spaces before the closing parenthesis or after an ordinary operand.
    AfterSpace,
    /// Carriage returns before the closing parenthesis or after an ordinary operand.
    AfterReturn,
    /// Unused positioning mode.
    Ignored,
}
/// One deferred whitespace attribute.
#[derive(Clone, Copy)]
pub(in super::super) struct PtgAttrSpace {
    /// Position and character kind.
    pub(super) kind: SpaceType,
    /// Number of characters.
    pub(super) count: u8,
}

/// Scalar kinds admitted by a binary constant matrix.
pub(in super::super) enum SerAr {
    /// Empty member.
    Nil,
    /// Numeric member.
    Num(f64),
    /// Text member.
    Str(String),
    /// Boolean member.
    Bool(bool),
    /// Stored error member.
    Err(u8),
}
