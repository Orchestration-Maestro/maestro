//! Public fuzzy matching observations.

use maestro_tui::fuzzy_match;

#[test]
fn fuzzy_scores_ordered_characters() {
    for &(query, text, matches, score, id) in FUZZY_SCORES_ORDERED_CHARACTERS_CASES {
        let found = fuzzy_match(query, text);
        assert_eq!((found.matches, found.score), (matches, score), "{id}");
    }
}

/// Matching inputs with exact independent scalar score observations.
const FUZZY_SCORES_ORDERED_CHARACTERS_CASES: &[(&str, &str, bool, f64, &str)] = &[
    ("", "anything", true, 0.0, "fm_empty"),
    ("longquery", "short", false, 0.0, "fm_long"),
    ("test", "test", true, -159.4, "fm_exact"),
    ("abc", "aXbXc", true, -10.4, "fm_ordered"),
    ("abc", "cba", false, 0.0, "fm_reversed"),
    ("ABC", "abc", true, -139.7, "fm_upper_query"),
    ("abc", "ABC", true, -139.7, "fm_upper_text"),
    (
        "foo",
        "foobar",
        true,
        -39.699_999_999_999_996,
        "fm_consecutive",
    ),
    (
        "foo",
        "f_o_o_bar",
        true,
        -30.400_000_000_000_002,
        "fm_scattered",
    ),
    ("fb", "foo-bar", true, -18.6, "fm_boundary"),
    ("fb", "afbx", true, -4.7, "fm_nonboundary"),
    (
        "codex52",
        "gpt-5.2-codex",
        true,
        -64.999_999_999_999_99,
        "fm_swap_alpha",
    ),
    (
        "52codex",
        "codex-5.2",
        true,
        -93.600_000_000_000_01,
        "fm_swap_numeric",
    ),
    ("a1", "a1 then 1a", true, -24.9, "fm_swap_primary"),
    ("abc123", "xxx", false, 0.0, "fm_swap_fails"),
    ("a1b", "b1a", false, 0.0, "fm_not_swappable"),
    ("\u{e9}", "e\u{301}", false, 0.0, "fm_combining"),
    (
        "\u{3bf}\u{3c2}",
        "\u{39f}\u{3a3}",
        true,
        -124.9,
        "fm_context_lower",
    ),
    ("i\u{307}", "\u{130}", true, -124.9, "fm_dotted_i"),
    ("\u{e9}1", "1\u{e9}", false, 0.0, "fm_unicode_swap"),
    ("12", "21", false, 0.0, "fm_digits_only"),
];

#[test]
fn fuzzy_scoring_does_not_join_unrelated_surrogate_halves() {
    for &(query, text, matches, score, id) in
        FUZZY_SCORING_DOES_NOT_JOIN_UNRELATED_SURROGATE_HALVES_CASES
    {
        let found = fuzzy_match(query, text);
        assert_eq!((found.matches, found.score), (matches, score), "{id}");
    }
}

/// Matching inputs with exact independent scalar score observations.
const FUZZY_SCORING_DOES_NOT_JOIN_UNRELATED_SURROGATE_HALVES_CASES: &[(
    &str,
    &str,
    bool,
    f64,
    &str,
)] = &[
    ("\u{1f600}", "\u{1f600}", true, -115.0, "fm_emoji_exact"),
    (
        "\u{1f600}",
        "\u{1f601}\u{1fa00}",
        false,
        0.0,
        "fm_emoji_false",
    ),
    ("b", "\u{1f600}b", true, 0.1, "fm_emoji_offset"),
];

#[test]
fn fuzzy_boundaries_use_the_shared_whitespace_set() {
    for &(query, text, matches, score, id) in FUZZY_BOUNDARIES_USE_THE_SHARED_WHITESPACE_SET_CASES {
        let found = fuzzy_match(query, text);
        assert_eq!((found.matches, found.score), (matches, score), "{id}");
    }
}

/// Matching inputs with exact independent scalar score observations.
const FUZZY_BOUNDARIES_USE_THE_SHARED_WHITESPACE_SET_CASES: &[(&str, &str, bool, f64, &str)] = &[
    ("b", "a b", true, -9.8, "boundary_20"),
    ("b", "a\u{9}b", true, -9.8, "boundary_9"),
    ("b", "a\u{a}b", true, -9.8, "boundary_a"),
    ("b", "a-b", true, -9.8, "boundary_2d"),
    ("b", "a_b", true, -9.8, "boundary_5f"),
    ("b", "a.b", true, -9.8, "boundary_2e"),
    ("b", "a/b", true, -9.8, "boundary_2f"),
    ("b", "a:b", true, -9.8, "boundary_3a"),
    ("b", "a\u{feff}b", true, -9.8, "boundary_feff"),
    ("b", "a\u{85}b", true, 0.2, "boundary_85"),
    ("b", "a\u{a0}b", true, -9.8, "boundary_a0"),
    ("b", "a\u{200b}b", true, 0.2, "boundary_200b"),
];

#[test]
fn fuzzy_filter_preserves_ranked_borrowed_items() {
    for (id, query, items, expected) in FILTER_CASES.iter().copied() {
        let found = maestro_tui::fuzzy_filter(items, query, |item| item.0.to_owned());
        let ids: Vec<_> = found.iter().map(|item| item.1).collect();
        assert_eq!(ids, expected, "{id}");
        for item in found {
            assert!(items.iter().any(|original| std::ptr::eq(item, original)));
        }
    }
}

#[test]
fn empty_filters_do_not_evaluate_the_projection() {
    let items = ["one"];
    let found = maestro_tui::fuzzy_filter(&items, " \u{feff}\t", |_| -> &str {
        panic!("projection must not run")
    });
    assert!(std::ptr::eq(found[0], &raw const items[0]));
}

/// Filtering inputs and their ranked original identities.
type FilterCase = (
    &'static str,
    &'static str,
    &'static [(&'static str, usize)],
    &'static [usize],
);

/// Independent recorded filter outputs.
const FILTER_CASES: &[FilterCase] = &[
    (
        "ff_empty",
        "",
        &[("apple", 0), ("banana", 1), ("cherry", 2)],
        &[0, 1, 2],
    ),
    (
        "ff_spaces",
        " \u{feff}\u{9}",
        &[("a", 0), ("b", 1)],
        &[0, 1],
    ),
    (
        "ff_filter",
        "an",
        &[("apple", 0), ("banana", 1), ("cherry", 2)],
        &[1],
    ),
    (
        "ff_quality",
        "app",
        &[("a_p_p", 0), ("app", 1), ("application", 2)],
        &[1, 2, 0],
    ),
    ("ff_exact", "cl", &[("clone", 0), ("cl", 1)], &[1, 0]),
    (
        "ff_multi",
        "a b",
        &[("ab", 0), ("a_b", 1), ("ba", 2), ("a", 3), ("b", 4)],
        &[1, 0, 2],
    ),
    (
        "ff_ties",
        "ab",
        &[("a-b", 0), ("a_b", 1), ("a.b", 2), ("a/b", 3), ("a:b", 4)],
        &[0, 1, 2, 3, 4],
    ),
    (
        "ff_bom",
        "a\u{feff}b",
        &[("ab", 0), ("ba", 1), ("a", 2), ("b", 3)],
        &[0, 1],
    ),
    ("ff_nel", "a\u{85}b", &[("ab", 0), ("a\u{85}b", 1)], &[1]),
    (
        "ff_duplicate",
        "a",
        &[("aa", 0), ("aa", 1), ("A", 2)],
        &[2, 0, 1],
    ),
    (
        "ff_projection",
        "foo",
        &[("foo", 1), ("bar", 2), ("foobar", 3)],
        &[1, 3],
    ),
];
