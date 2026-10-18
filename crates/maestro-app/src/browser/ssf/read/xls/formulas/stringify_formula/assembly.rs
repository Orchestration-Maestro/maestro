//! Retained expression splits and composable case-fold summaries.
/// A handle retains output and a typed split around its last sheet separator.
#[derive(Clone, Copy)]
pub(super) struct Expression {
    /// Index of the retained fragment.
    node: usize,
    /// Last qualifier split, if a separator occurs.
    qualified: Option<Qualified>,
}
/// A split at the last separator in complete expression text.
#[derive(Clone, Copy)]
struct Qualified {
    /// Complete text before the last separator.
    prefix: usize,
    /// Complete text after the last separator.
    endpoint: usize,
}
/// A retained child with its surrounding cased context.
struct Child {
    /// Index of the retained fragment.
    node: usize,
    /// Nearest preceding non-case-ignorable context.
    before: Option<bool>,
    /// Nearest following non-case-ignorable context.
    after: Option<bool>,
}
/// Owned leaf text or an ordered sequence of child handles.
enum Fragment {
    /// Leaf spelling and its Unicode fold views.
    Text {
        /// Original spelling retained for output.
        raw: String,
        /// Context-dependent folded views with sentinel boundaries.
        lower: Vec<String>,
    },
    /// Ordered children with retained surrounding context.
    Join(Vec<Child>),
}
/// Composable lowercase length, fingerprints and surrounding context.
#[derive(Clone, Copy, Default)]
struct Fold {
    /// Lowercase byte length.
    len: usize,
    /// Fingerprint base raised to the lowercase byte length.
    power: u64,
    /// Fingerprints for the four incoming and outgoing cased contexts.
    hash: [u64; 4],
    /// First non-case-ignorable cased classification.
    first: Option<bool>,
    /// Last non-case-ignorable cased classification.
    last: Option<bool>,
}
/// One owned fragment with its raw and folded summaries.
struct Node {
    /// Owned text or retained children.
    part: Fragment,
    /// Original byte length.
    length: usize,
    /// Composable lowercase summary.
    fold: Fold,
}
/// Flat ownership for depth-independent composition and destruction.
pub(super) struct Assembly {
    /// Single owner for retained fragments.
    nodes: Vec<Node>,
    /// Process-random fingerprint base.
    base: u64,
}
impl Default for Assembly {
    fn default() -> Self {
        use std::hash::BuildHasher;
        /// One randomized base shared by all formula assemblies.
        static BASE: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| {
            std::collections::hash_map::RandomState::new().hash_one("formula-prefix") | 0x0101
        });
        Self {
            nodes: vec![Node {
                part: Fragment::Text {
                    raw: String::new(),
                    lower: Vec::new(),
                },
                length: 0,
                fold: Fold {
                    power: 1,
                    ..Fold::default()
                },
            }],
            base: *BASE,
        }
    }
}
impl Assembly {
    /// Test injection changes only the rejection filter, never equality verification.
    fn collisions() -> bool {
        #[cfg(test)]
        {
            tests::FORCE_COLLISIONS.get()
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    /// Introduce a locally normalized leaf without scanning earlier operands.
    fn raw(&mut self, raw: String) -> usize {
        let length = raw.len();
        charge(0, 3 * raw.len());
        let (first, last) = significant(&raw);
        let lower = if raw.is_ascii() {
            Vec::new()
        } else if raw.contains('Σ') {
            (0..4)
                .map(|i| {
                    folded(&format!(
                        "{}{}{}",
                        if i & 2 == 0 { ' ' } else { 'A' },
                        raw,
                        if i & 1 == 0 { ' ' } else { 'A' }
                    ))
                })
                .collect::<Vec<_>>()
        } else {
            vec![folded(&format!(" {raw} "))]
        };
        charge(0, length);
        let (lower_len, power, hash) = fingerprint(&raw, &lower, self.base);
        let fold = Fold {
            len: lower_len,
            power,
            hash,
            first,
            last,
        };
        let node = self.nodes.len();
        self.nodes.push(Node {
            part: Fragment::Text { raw, lower },
            length,
            fold,
        });
        node
    }
    /// Compose summaries, removing empty and single-child wrappers.
    fn concat(&mut self, mut parts: Vec<usize>) -> usize {
        parts.retain(|i| self.nodes[*i].length != 0);
        if parts.is_empty() {
            return 0;
        }
        if parts.len() == 1 {
            return parts[0];
        }
        let mut total = Fold {
            power: 1,
            ..Fold::default()
        };
        let mut length = 0;
        let mut children = Vec::with_capacity(parts.len());
        for node in parts {
            let f = self.nodes[node].fold;
            children.push(Child {
                node,
                before: total.last,
                after: None,
            });
            total = concat_fold(total, f);
            length += self.nodes[node].length;
        }
        let mut next = None;
        for c in children.iter_mut().rev() {
            c.after = next;
            next = self.nodes[c.node].fold.first.or(next);
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            part: Fragment::Join(children),
            length,
            fold: total,
        });
        node
    }
    /// Split a newly introduced leaf at its last separator once.
    pub(super) fn text(&mut self, mut text: String) -> Expression {
        if text.is_empty() {
            return Expression {
                node: 0,
                qualified: None,
            };
        }
        charge(text.len(), 0);
        let separator = text.rfind('!');
        charge(0, text.len());
        if let Some(at) = separator {
            let tail = text.split_off(at + 1);
            text.pop();
            charge(tail.len() + 1, 0);
            let prefix = self.raw(text);
            let bang = self.raw("!".into());
            let endpoint = self.raw(tail);
            let node = self.concat(vec![prefix, bang, endpoint]);
            Expression {
                node,
                qualified: Some(Qualified { prefix, endpoint }),
            }
        } else {
            Expression {
                node: self.raw(text),
                qualified: None,
            }
        }
    }
    /// Retain the last qualified child split through concatenation.
    pub(super) fn join(&mut self, parts: &[Expression]) -> Expression {
        let qualified = parts
            .iter()
            .rposition(|p| p.qualified.is_some())
            .and_then(|i| {
                let q = parts[i].qualified?;
                let mut prefix = parts[..i].iter().map(|p| p.node).collect::<Vec<_>>();
                prefix.push(q.prefix);
                let mut endpoint = vec![q.endpoint];
                endpoint.extend(parts[i + 1..].iter().map(|p| p.node));
                Some(Qualified {
                    prefix: self.concat(prefix),
                    endpoint: self.concat(endpoint),
                })
            });
        let node = self.concat(parts.iter().map(|p| p.node).collect());
        Expression { node, qualified }
    }
    /// Compose delimiters through the same retained owner.
    pub(super) fn surround(
        &mut self,
        operand: Expression,
        before: String,
        after: String,
    ) -> Expression {
        let before = self.text(before);
        let after = self.text(after);
        self.join(&[before, operand, after])
    }
    /// Visit lowercase leaves with surrounding context without copying prefixes.
    fn lower_parts(&self, node: usize) -> impl Iterator<Item = &str> {
        let mut pending = vec![(node, false, false)];
        std::iter::from_fn(move || self.next_lower(&mut pending))
    }
    /// Resolve the next lowercase leaf with its inherited cased context.
    fn next_lower(&self, pending: &mut Vec<(usize, bool, bool)>) -> Option<&str> {
        while let Some((node, left, right)) = pending.pop() {
            match &self.nodes[node].part {
                Fragment::Text { raw, lower } => {
                    return Some(lower_view(
                        raw,
                        lower,
                        usize::from(left) * 2 + usize::from(right),
                    ));
                }
                Fragment::Join(parts) => pending.extend(parts.iter().rev().map(|child| {
                    (
                        child.node,
                        child.before.unwrap_or(left),
                        child.after.unwrap_or(right),
                    )
                })),
            }
        }
        None
    }
    /// Reject by summary, then verify matching fingerprints byte for byte.
    fn equal(&self, a: usize, b: usize) -> bool {
        let (a_fold, b_fold) = (self.nodes[a].fold, self.nodes[b].fold);
        if a_fold.len != b_fold.len || (!Self::collisions() && a_fold.hash[0] != b_fold.hash[0]) {
            return false;
        }
        // A match is never accepted on its fingerprint alone.
        self.lower_parts(a)
            .flat_map(str::bytes)
            .map(|b| b.to_ascii_lowercase())
            .inspect(|_| charge(0, 1))
            .eq(self
                .lower_parts(b)
                .flat_map(str::bytes)
                .map(|b| b.to_ascii_lowercase())
                .inspect(|_| charge(0, 1)))
    }
    /// Remove only a matching, nonempty right qualifier.
    pub(super) fn range_right(&self, left: Expression, right: Expression) -> Expression {
        if let (Some(a), Some(b)) = (left.qualified, right.qualified)
            && self.nodes[a.prefix].length > 0
            && self.nodes[b.prefix].length > 0
            && self.equal(a.prefix, b.prefix)
        {
            return Expression {
                node: b.endpoint,
                qualified: None,
            };
        }
        right
    }
    /// Emit retained original spelling once without recursive traversal.
    pub(super) fn finish(&self, expression: Expression) -> String {
        let mut result = String::with_capacity(self.nodes[expression.node].length);
        let mut pending = vec![expression.node];
        while let Some(node) = pending.pop() {
            match &self.nodes[node].part {
                Fragment::Text { raw, .. } => {
                    charge(raw.len(), 0);
                    result.push_str(raw);
                }
                Fragment::Join(parts) => pending.extend(parts.iter().rev().map(|c| c.node)),
            }
        }
        result
    }
}
/// Combine the four context-dependent fingerprints in constant work.
fn concat_fold(a: Fold, b: Fold) -> Fold {
    let hash = std::array::from_fn(|i| {
        let l = i & 2 != 0;
        let r = i & 1 != 0;
        let ar = b.first.unwrap_or(r);
        let bl = a.last.unwrap_or(l);
        a.hash[usize::from(l) * 2 + usize::from(ar)]
            .wrapping_mul(b.power)
            .wrapping_add(b.hash[usize::from(bl) * 2 + usize::from(r)])
    });
    Fold {
        len: a.len + b.len,
        power: a.power.wrapping_mul(b.power),
        hash,
        first: a.first.or(b.first),
        last: b.last.or(a.last),
    }
}
/// Lowercase a local context probe and account both temporary strings.
fn folded(raw: &str) -> String {
    let lower = raw.to_lowercase();
    charge(0, raw.len());
    charge(raw.len() + lower.len(), 0);
    lower
}
/// Discover Unicode cased context through standard-library lowercase probes.
fn significant(raw: &str) -> (Option<bool>, Option<bool>) {
    if raw.is_ascii() {
        charge(0, raw.len());
        let mut significant = raw
            .bytes()
            .filter(|b| !matches!(*b, b'\'' | b'.' | b':' | b'^' | b'`'))
            .map(|b| b.is_ascii_alphabetic());
        let first = significant.next();
        let last = significant.next_back().or(first);
        return (first, last);
    }
    charge(0, 16);
    let fa = folded(&format!("AΣ{raw}A")).starts_with("aς");
    let fu = folded(&format!("AΣ{raw} ")).starts_with("aσ");
    let first = if fa {
        Some(false)
    } else if fu {
        Some(true)
    } else {
        None
    };
    let la = folded(&format!("A{raw}Σ")).ends_with('ς');
    let lu = folded(&format!(" {raw}Σ")).ends_with('ς');
    let last = if la == lu { Some(la) } else { None };
    (first, last)
}

/// Account logical text writes and byte visits, including case-fold temporaries.
fn charge(written: usize, scanned: usize) {
    let _ = (written, scanned);
    #[cfg(test)]
    {
        super::super::super::tests::ASSEMBLY_WORK.with(|n| n.set(n.get() + written));
        super::super::super::tests::ASSEMBLY_SCAN.with(|n| n.set(n.get() + scanned));
    }
}

/// Summarize only newly introduced leaf bytes.
fn fingerprint(raw: &str, lower: &[String], base: u64) -> (usize, u64, [u64; 4]) {
    if lower.is_empty() {
        let (mut value, mut power) = (0u64, 1u64);
        for b in raw.bytes() {
            value = value
                .wrapping_mul(base)
                .wrapping_add(u64::from(b.to_ascii_lowercase()) + 1);
            power = power.wrapping_mul(base);
        }
        charge(0, raw.len());
        (raw.len(), power, [value; 4])
    } else {
        let bytes = &lower[0].as_bytes()[1..lower[0].len() - 1];
        let power = bytes.iter().fold(1u64, |v, _| v.wrapping_mul(base));
        let hash = std::array::from_fn(|i| {
            let s = &lower[if lower.len() == 1 { 0 } else { i }];
            s.as_bytes()[1..s.len() - 1].iter().fold(0u64, |v, &b| {
                v.wrapping_mul(base).wrapping_add(u64::from(b) + 1)
            })
        });
        charge(0, 5 * bytes.len());
        (bytes.len(), power, hash)
    }
}

/// Select a Unicode context view or the allocation-free ASCII spelling.
fn lower_view<'a>(raw: &'a str, lower: &'a [String], context: usize) -> &'a str {
    lower
        .get(context)
        .or_else(|| lower.first())
        .map_or(raw, |text| &text[1..text.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::Assembly;
    thread_local! {
        /// Exercise exact verification even when fingerprints would reject the operands.
        pub(super) static FORCE_COLLISIONS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    #[test]
    fn fingerprint_collisions_never_change_range_outputs() {
        use crate::browser::ssf::{self, Ssf};
        FORCE_COLLISIONS.set(true);
        let mut assembly = Assembly::default();
        let first = assembly.raw("Data".into());
        let different = assembly.raw("Else".into());
        assert!(!assembly.equal(first, different));
        let same = assembly.raw("dAtA".into());
        assert!(assembly.equal(first, same));
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ssf/legacy_range_prefixes.json"
        )))
        .unwrap();
        for row in rows.as_array().unwrap() {
            let file = row["file"].as_str().unwrap();
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/workbooks")
                    .join(file),
            )
            .unwrap();
            let book = ssf::read(&bytes, file, &mut Ssf::default()).unwrap();
            assert_eq!(
                book.sheets[row["sheet"].as_str().unwrap()]
                    .as_ref()
                    .unwrap()
                    .cells["A1"]
                    .f
                    .as_deref(),
                Some(row["formula"].as_str().unwrap()),
                "{file}"
            );
        }
        FORCE_COLLISIONS.set(false);
    }
}
