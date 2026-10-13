//! Strict oracle fixture consumption.

use maestro_app::{Presence, SessionInfo};
use maestro_chat::*;
use serde::Deserialize;
use serde_json::Value;

/// A named operation and its independent expected observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    input: Operation,
    expected: Expected,
}

/// Inputs accepted by each fixture operation.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Operation {
    Parse {
        query: String,
    },
    Name {
        session: Record,
    },
    Match {
        session: Record,
        query: Option<String>,
        parsed: Option<Query>,
    },
    Filter {
        sessions: Vec<Record>,
        query: String,
        sort: String,
        filter: Option<String>,
    },
}

/// A fixture session, including metadata sentinels excluded from search.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    id: Presence<Value>,
    id_missing: bool,
    name: Option<String>,
    all_messages_text: String,
    cwd: String,
    modified: f64,
    created: f64,
    path: String,
    parent_session_path: Presence<Value>,
    first_message: String,
    message_count: usize,
}

impl Record {
    /// Build the owner's unchanged record with source fixture defaults.
    fn session(self) -> SessionInfo {
        SessionInfo {
            id: if self.id_missing {
                Presence::Missing
            } else if self.id.is_missing() {
                Presence::Present(Value::String("s".into()))
            } else {
                self.id
            },
            name: self.name,
            all_messages_text: self.all_messages_text,
            cwd: self.cwd,
            modified: self.modified,
            created: self.created,
            path: self.path,
            parent_session_path: self.parent_session_path,
            first_message: self.first_message,
            message_count: self.message_count,
        }
    }
}

/// A parsed-query snapshot, also used for direct caller-created queries.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Query {
    mode: String,
    tokens: Vec<Token>,
    regex: Option<bool>,
    #[serde(default)]
    error: Option<String>,
}

/// One ordered token in the independent snapshot.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "lowercase",
    deny_unknown_fields
)]
enum Token {
    Fuzzy(String),
    Phrase(String),
}

/// The operation-specific expected output.
#[derive(Deserialize)]
#[serde(untagged)]
enum Expected {
    Name(bool),
    Indices(Vec<usize>),
    Match(Score),
    Query(Query),
}

/// A match flag and independently recorded score.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Score {
    matches: bool,
    score: f64,
}

/// Compare one named group's public operations with its oracle observations.
pub fn run_group(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../fixtures/session_search.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        count += 1;
        match (case.input, case.expected) {
            (Operation::Parse { query }, Expected::Query(expected)) => {
                assert_eq!(snapshot(parse_search_query(&query)), expected);
            }
            (Operation::Name { session }, Expected::Name(expected)) => {
                assert_eq!(has_session_name(&session.session()), expected);
            }
            (
                Operation::Match {
                    session,
                    query,
                    parsed,
                },
                Expected::Match(expected),
            ) => {
                let parsed = query.map_or_else(
                    || manual_query(parsed.unwrap()),
                    |query| parse_search_query(&query),
                );
                let actual = match_session(&session.session(), &parsed);
                assert_eq!(actual.matches, expected.matches);
                assert_eq!(actual.score.to_bits(), expected.score.to_bits());
            }
            (
                Operation::Filter {
                    sessions,
                    query,
                    sort,
                    filter,
                },
                Expected::Indices(expected),
            ) => {
                let sessions: Vec<_> = sessions.into_iter().map(Record::session).collect();
                let actual = filter_and_sort_sessions(
                    &sessions,
                    &query,
                    sort_mode(&sort),
                    name_filter(filter.as_deref()),
                );
                let indices: Vec<_> = actual
                    .into_iter()
                    .map(|selected| {
                        sessions
                            .iter()
                            .position(|record| std::ptr::eq(record, selected))
                            .unwrap()
                    })
                    .collect();
                assert_eq!(indices, expected);
            }
            _ => panic!("fixture operation and expectation differ"),
        }
    }
    assert!(count > 0, "fixture group must contain observations");
}

/// Project parser output without deriving expectations from product helpers.
fn snapshot(parsed: ParsedSearchQuery) -> Query {
    match parsed {
        ParsedSearchQuery::Tokens(tokens) => Query {
            mode: "tokens".into(),
            regex: None,
            error: None,
            tokens: tokens
                .into_iter()
                .map(|token| match token {
                    SearchToken::Fuzzy(text) => Token::Fuzzy(text),
                    SearchToken::Phrase(text) => Token::Phrase(text),
                })
                .collect(),
        },
        ParsedSearchQuery::Regex(result) => Query {
            mode: "regex".into(),
            tokens: vec![],
            regex: result.as_ref().ok().map(|_| true),
            error: result.err().map(|error| {
                if error == "Empty regex" {
                    error
                } else {
                    "invalid-regex".into()
                }
            }),
        },
    }
}

/// Construct only the explicitly supplied manual-query branches.
fn manual_query(query: Query) -> ParsedSearchQuery {
    assert!(query.regex.is_none());
    assert!(query.error.is_none());
    match query.mode.as_str() {
        "regex" => {
            assert!(query.tokens.is_empty());
            ParsedSearchQuery::Regex(Err(String::new()))
        }
        "tokens" => ParsedSearchQuery::Tokens(
            query
                .tokens
                .into_iter()
                .map(|token| match token {
                    Token::Fuzzy(text) => SearchToken::Fuzzy(text),
                    Token::Phrase(text) => SearchToken::Phrase(text),
                })
                .collect(),
        ),
        _ => panic!("unknown fixture query mode"),
    }
}

/// Decode the fixture ordering policy.
fn sort_mode(value: &str) -> SortMode {
    match value {
        "recent" => SortMode::Recent,
        "threaded" => SortMode::Threaded,
        "relevance" => SortMode::Relevance,
        _ => panic!("unknown fixture sort mode"),
    }
}

/// Decode the independently selected name policy.
fn name_filter(value: Option<&str>) -> NameFilter {
    match value {
        None => NameFilter::default(),
        Some("all") => NameFilter::All,
        Some("named") => NameFilter::Named,
        _ => panic!("unknown fixture name filter"),
    }
}
