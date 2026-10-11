//! Typed bracket-class parsing and scalar membership.

use std::sync::LazyLock;

/// A bracket parser's selected component syntax.
pub(super) enum ParsedClass {
    /// No closing delimiter; resume ordinary token parsing.
    Unclosed,
    /// One surviving ordinary member with no negation.
    Literal(char),
    /// A union predicate consuming one scalar.
    Class(Class),
    /// An invalid member selection that cannot match.
    Never,
}

/// Prepared scalar intervals and named predicates.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Class {
    /// Sorted disjoint inclusive intervals.
    ranges: Vec<(char, char)>,
    /// Named predicates contributing to the union.
    named: Vec<NamedClass>,
    /// Whether to complement the complete union.
    negated: bool,
}

impl Class {
    /// Evaluates prepared membership, then applies outer negation.
    pub(super) fn matches(&self, scalar: char) -> bool {
        let end = self.ranges.partition_point(|(start, _)| *start <= scalar);
        let member = end
            .checked_sub(1)
            .is_some_and(|i| scalar <= self.ranges[i].1)
            || self.named.iter().any(|named| named.matches(scalar));
        member ^ self.negated
    }

    /// Selects literal identity before normalizing interval membership.
    fn finish(mut self) -> ParsedClass {
        if self.ranges.is_empty() && self.named.is_empty() {
            return ParsedClass::Never;
        }
        if !self.negated && self.named.is_empty() && self.ranges.len() == 1 {
            let (start, end) = self.ranges[0];
            if start == end {
                return ParsedClass::Literal(start);
            }
        }
        self.named.sort_unstable();
        self.named.dedup();
        self.ranges.sort_unstable();
        let mut merged: Vec<(char, char)> = Vec::new();
        for (start, end) in self.ranges {
            if let Some(last) = merged.last_mut()
                && u32::from(start) <= u32::from(last.1) + 1
            {
                last.1 = last.1.max(end);
            } else {
                merged.push((start, end));
            }
        }
        self.ranges = merged;
        ParsedClass::Class(self)
    }
}

/// A named Unicode or ASCII member set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NamedClass {
    /// Letters, letter numbers and decimal digits.
    Alnum,
    /// Letters and letter numbers.
    Alpha,
    /// ASCII scalars.
    Ascii,
    /// Space separators and tab.
    Blank,
    /// Control characters.
    Cntrl,
    /// Decimal digits.
    Digit,
    /// Neither separators nor Other characters.
    Graph,
    /// Lowercase letters.
    Lower,
    /// Characters outside Unicode Other.
    Print,
    /// Punctuation.
    Punct,
    /// Separators and specified ASCII whitespace.
    Space,
    /// Uppercase letters.
    Upper,
    /// Letters, letter numbers, decimal digits and connector punctuation.
    Word,
    /// ASCII hexadecimal digits.
    Xdigit,
}

impl NamedClass {
    /// Recognizes the exact named suffix following an opening bracket.
    fn parse(tail: &str) -> Option<(Self, usize)> {
        let names = [
            (":alnum:]", Self::Alnum),
            (":alpha:]", Self::Alpha),
            (":ascii:]", Self::Ascii),
            (":blank:]", Self::Blank),
            (":cntrl:]", Self::Cntrl),
            (":digit:]", Self::Digit),
            (":graph:]", Self::Graph),
            (":lower:]", Self::Lower),
            (":print:]", Self::Print),
            (":punct:]", Self::Punct),
            (":space:]", Self::Space),
            (":upper:]", Self::Upper),
            (":word:]", Self::Word),
            (":xdigit:]", Self::Xdigit),
        ];
        names
            .into_iter()
            .find_map(|(text, name)| tail.starts_with(text).then_some((name, text.len())))
    }

    /// Tests one named predicate against scalar text.
    fn matches(self, scalar: char) -> bool {
        /// Fixed expressions indexed by the named-member vocabulary.
        static EXPRESSIONS: LazyLock<[Option<regress::Regex>; 14]> = LazyLock::new(|| {
            [
                r"^[\p{L}\p{Nl}\p{Nd}]$",
                r"^[\p{L}\p{Nl}]$",
                r"^[\x00-\x7f]$",
                r"^[\p{Zs}\t]$",
                r"^\p{Cc}$",
                r"^\p{Nd}$",
                r"^[^\p{Z}\p{C}]$",
                r"^\p{Ll}$",
                r"^[^\p{C}]$",
                r"^\p{P}$",
                r"^[\p{Z}\t\r\n\v\f]$",
                r"^\p{Lu}$",
                r"^[\p{L}\p{Nl}\p{Nd}\p{Pc}]$",
                r"^[A-Fa-f0-9]$",
            ]
            .map(|pattern| regress::Regex::with_flags(pattern, "u").ok())
        });
        let mut bytes = [0; 4];
        EXPRESSIONS[self as usize]
            .as_ref()
            .is_some_and(|re| re.find(scalar.encode_utf8(&mut bytes)).is_some())
    }
}

/// One member or the outer closing delimiter.
enum Member {
    /// A decoded scalar, including escaped delimiters.
    Scalar(char),
    /// A complete recognized named member.
    Named(NamedClass),
    /// The outer closing bracket.
    End,
}

/// Shared class grammar without membership normalization.
struct Members<'a> {
    /// Remaining authored bytes.
    tail: &'a str,
    /// Whether a member has preceded the next closing bracket.
    saw_start: bool,
    /// Whether the member union is complemented.
    negated: bool,
}

impl<'a> Members<'a> {
    /// Starts after the opening bracket and optional negation marker.
    fn new(pattern: &'a str) -> Self {
        let tail = &pattern[1..];
        let negated = tail.starts_with(['!', '^']);
        Self {
            tail: if negated { &tail[1..] } else { tail },
            saw_start: false,
            negated,
        }
    }

    /// Recognizes one complete member without evaluating its predicate.
    fn next(&mut self) -> Option<Member> {
        let scalar = take_scalar(&mut self.tail)?;
        if scalar == ']' && self.saw_start {
            return Some(Member::End);
        }
        self.saw_start = true;
        if scalar == '\\' {
            return take_scalar(&mut self.tail).map(Member::Scalar);
        }
        if scalar == '['
            && let Some((named, consumed)) = NamedClass::parse(self.tail)
        {
            self.tail = &self.tail[consumed..];
            return Some(Member::Named(named));
        }
        Some(Member::Scalar(scalar))
    }
}

/// Finds the outer closing bracket, shielding an unclosed region to component end.
pub(super) fn class_boundary(pattern: &str) -> usize {
    let mut members = Members::new(pattern);
    while let Some(member) = members.next() {
        if matches!(member, Member::End) {
            return pattern.len() - members.tail.len();
        }
    }
    pattern.len()
}

/// Parses a class at the start of a component suffix, returning consumed bytes.
pub(super) fn parse_class(pattern: &str) -> (ParsedClass, usize) {
    let mut members = Members::new(pattern);
    let mut class = Class {
        ranges: Vec::new(),
        named: Vec::new(),
        negated: members.negated,
    };
    let mut invalid = false;
    let mut range_start = None;
    while let Some(member) = members.next() {
        match member {
            Member::End => {
                let parsed = if invalid {
                    ParsedClass::Never
                } else {
                    class.finish()
                };
                return (parsed, pattern.len() - members.tail.len());
            }
            Member::Named(named) => {
                invalid |= range_start.take().is_some();
                class.named.push(named);
            }
            Member::Scalar(scalar) => {
                add_scalar(&mut class, &mut range_start, scalar, &mut members.tail);
            }
        }
    }
    if invalid {
        (ParsedClass::Never, pattern.len())
    } else {
        (ParsedClass::Unclosed, 0)
    }
}

/// Adds one ordinary member or completes a pending range.
fn add_scalar(class: &mut Class, start: &mut Option<char>, scalar: char, tail: &mut &str) {
    if let Some(first) = start.take() {
        if first <= scalar {
            class.ranges.push((first, scalar));
        }
    } else if tail.starts_with("-]") {
        class.ranges.extend([(scalar, scalar), ('-', '-')]);
        *tail = &tail[1..];
    } else if tail.starts_with('-') {
        *start = Some(scalar);
        *tail = &tail[1..];
    } else {
        class.ranges.push((scalar, scalar));
    }
}

/// Removes exactly one scalar from an authored suffix.
fn take_scalar(tail: &mut &str) -> Option<char> {
    let scalar = tail.chars().next()?;
    *tail = &tail[scalar.len_utf8()..];
    Some(scalar)
}
