//! Wildcard matching with Git's path rules.

/// How a pattern compares with its candidate.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rules {
    /// Whether a single `*` and `?` stop at `/`; `**` always crosses it.
    pub pathname: bool,
    /// Whether ASCII letters compare without case.
    pub ignore_case: bool,
}

/// One unit of a compiled pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    /// A character that must appear.
    Literal(char),
    /// Exactly one character.
    One,
    /// Any run of characters, `/` included only when `crossing`.
    Run {
        /// Whether the run may contain `/`.
        crossing: bool,
    },
}

/// Whether `pattern` matches all of `text`, in time proportional to their product.
pub(super) fn wildmatch(pattern: &str, text: &str, rules: Rules) -> bool {
    let fold = |ch: char| {
        if rules.ignore_case {
            ch.to_ascii_lowercase()
        } else {
            ch
        }
    };
    let text: Vec<char> = text.chars().map(fold).collect();
    let mut reachable = vec![false; text.len() + 1];
    reachable[0] = true;
    for token in tokens(pattern, rules, fold) {
        reachable = advance(&reachable, &text, token, rules);
    }
    reachable[text.len()]
}

/// Compile a pattern into literal, single-character and run tokens.
fn tokens(pattern: &str, rules: Rules, fold: impl Fn(char) -> char) -> Vec<Token> {
    let mut compiled = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        compiled.push(match ch {
            '?' => Token::One,
            '*' => {
                let mut stars = 1;
                while chars.next_if_eq(&'*').is_some() {
                    stars += 1;
                }
                Token::Run {
                    crossing: stars > 1 || !rules.pathname,
                }
            }
            other => Token::Literal(fold(other)),
        });
    }
    compiled
}

/// Text positions reachable after one more token.
fn advance(reachable: &[bool], text: &[char], token: Token, rules: Rules) -> Vec<bool> {
    let mut next = vec![false; reachable.len()];
    match token {
        Token::Literal(expected) => {
            for (index, &ch) in text.iter().enumerate() {
                next[index + 1] = reachable[index] && ch == expected;
            }
        }
        Token::One => {
            for (index, &ch) in text.iter().enumerate() {
                next[index + 1] = reachable[index] && !(rules.pathname && ch == '/');
            }
        }
        Token::Run { crossing } => {
            next[0] = reachable[0];
            for (index, &ch) in text.iter().enumerate() {
                next[index + 1] = reachable[index + 1] || (next[index] && (crossing || ch != '/'));
            }
        }
    }
    next
}
