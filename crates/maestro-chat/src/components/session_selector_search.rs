//! Session query parsing, matching and result ordering.

use maestro_app::is_whitespace_scalar;

/// An ordered query term.
#[derive(Debug, PartialEq)]
pub enum SearchToken {
    /// An ordered-character search.
    Fuzzy(String),
    /// A phrase matched contiguously after normalization.
    Phrase(String),
}

/// A token query or a compiled expression with its parsing failure.
pub enum ParsedSearchQuery {
    /// Terms in their authored order.
    Tokens(Vec<SearchToken>),
    /// A compiled expression or its diagnostic.
    Regex(Result<regress::Regex, String>),
}

/// Parse quoted phrases, fuzzy terms or a lowercase `re:` expression.
#[must_use]
pub fn parse_search_query(query: &str) -> ParsedSearchQuery {
    let query = query.trim_matches(is_whitespace_scalar);
    if let Some(pattern) = query.strip_prefix("re:") {
        let pattern = pattern.trim_matches(is_whitespace_scalar);
        return ParsedSearchQuery::Regex(if pattern.is_empty() {
            Err("Empty regex".into())
        } else {
            regress::Regex::with_flags(pattern, "i").map_err(|error| error.to_string())
        });
    }
    let mut tokens = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (offset, character) in query.char_indices() {
        if character == '"' || (!quoted && is_whitespace_scalar(character)) {
            push_token(&mut tokens, &query[start..offset], quoted);
            start = offset + character.len_utf8();
            if character == '"' {
                quoted = !quoted;
            }
        }
    }
    if quoted {
        return ParsedSearchQuery::Tokens(
            query
                .split(is_whitespace_scalar)
                .filter(|text| !text.is_empty())
                .map(|text| SearchToken::Fuzzy(text.into()))
                .collect(),
        );
    }
    push_token(&mut tokens, &query[start..], false);
    ParsedSearchQuery::Tokens(tokens)
}

/// Retain a nonblank span with its quoting classification.
fn push_token(tokens: &mut Vec<SearchToken>, text: &str, quoted: bool) {
    let text = text.trim_matches(is_whitespace_scalar);
    if !text.is_empty() {
        tokens.push(if quoted {
            SearchToken::Phrase(text.into())
        } else {
            SearchToken::Fuzzy(text.into())
        });
    }
}

/// Whether a session has a nonblank name, using the shared whitespace predicate.
#[must_use]
pub fn has_session_name(session: &maestro_app::SessionInfo) -> bool {
    session
        .name
        .as_ref()
        .is_some_and(|name| !name.trim_matches(is_whitespace_scalar).is_empty())
}

/// A match decision with its lower-is-better ranking score.
#[derive(Debug, PartialEq)]
pub struct MatchResult {
    /// Whether every query term matched.
    pub matches: bool,
    /// Accumulated term score, or zero on failure.
    pub score: f64,
}

/// Normalize a phrase by lowercasing before collapsing whitespace.
fn normalize_phrase(text: &str) -> String {
    text.to_lowercase()
        .split(is_whitespace_scalar)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Score a first-match byte offset in UTF-16 units.
fn offset_score(text: &str, offset: usize) -> f64 {
    text[..offset]
        .encode_utf16()
        .fold(0.0, |count, _| count + 1.0)
        * 0.1
}

/// Match only the session identity, name, message text and working directory.
#[must_use]
pub fn match_session(
    session: &maestro_app::SessionInfo,
    parsed: &ParsedSearchQuery,
) -> MatchResult {
    let id = match &session.id {
        maestro_app::Presence::Present(value) => value.as_str().unwrap_or_default(),
        _ => "",
    };
    let text = format!(
        "{id} {} {} {}",
        session.name.as_deref().unwrap_or_default(),
        session.all_messages_text,
        session.cwd
    );
    let failed = MatchResult {
        matches: false,
        score: 0.0,
    };
    let tokens = match parsed {
        ParsedSearchQuery::Regex(result) => {
            return result
                .as_ref()
                .ok()
                .and_then(|regex| regex.find(&text))
                .map_or(failed, |found| MatchResult {
                    matches: true,
                    score: offset_score(&text, found.start()),
                });
        }
        ParsedSearchQuery::Tokens(tokens) => tokens,
    };
    let mut normalized = None;
    let mut score = 0.0;
    for token in tokens {
        match token {
            SearchToken::Phrase(phrase) => {
                let normalized = normalized.get_or_insert_with(|| normalize_phrase(&text));
                let phrase = normalize_phrase(phrase);
                if phrase.is_empty() {
                    continue;
                }
                let Some(offset) = normalized.find(&phrase) else {
                    return failed;
                };
                score += offset_score(normalized, offset);
            }
            SearchToken::Fuzzy(query) => {
                let found = maestro_tui::fuzzy_match(query, &text);
                if !found.matches {
                    return failed;
                }
                score += found.score;
            }
        }
    }
    MatchResult {
        matches: true,
        score,
    }
}

/// Result ordering for a session query.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Relevance ordering for nonblank searches.
    Threaded,
    /// Incoming record order.
    Recent,
    /// Ascending score, then newest modification.
    Relevance,
}

/// Independent session-name membership policy.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum NameFilter {
    /// Include sessions regardless of name.
    #[default]
    All,
    /// Include only sessions with nonblank names.
    Named,
}

/// Return original records selected by name and query.
///
/// Blank queries preserve name-filtered input order. Nonblank threaded and
/// relevance searches rank by score, then newest modification; equal ranks
/// retain input order and duplicate records. Invalid expressions yield no matches.
#[must_use]
pub fn filter_and_sort_sessions<'a>(
    sessions: &'a [maestro_app::SessionInfo],
    query: &str,
    sort_mode: SortMode,
    name_filter: NameFilter,
) -> Vec<&'a maestro_app::SessionInfo> {
    let eligible = sessions
        .iter()
        .filter(|session| name_filter == NameFilter::All || has_session_name(session));
    if query.trim_matches(is_whitespace_scalar).is_empty() {
        return eligible.collect();
    }
    let parsed = parse_search_query(query);
    if matches!(parsed, ParsedSearchQuery::Regex(Err(_))) {
        return Vec::new();
    }
    let mut scored: Vec<_> = eligible
        .filter_map(|session| {
            let result = match_session(session, &parsed);
            result.matches.then_some((session, result.score))
        })
        .collect();
    if sort_mode != SortMode::Recent {
        scored.sort_by(|(left, left_score), (right, right_score)| {
            left_score
                .partial_cmp(right_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .modified
                        .partial_cmp(&left.modified)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }
    scored.into_iter().map(|(session, _)| session).collect()
}
