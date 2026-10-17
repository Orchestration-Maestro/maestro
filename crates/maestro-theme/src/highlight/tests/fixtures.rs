//! Strict independently recorded detection expectations.
use super::super::{HighlightProfile, core};
use serde::Deserialize;

/// The two surface observations attached to each input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profiles<T> {
    /// Exported session result.
    pub viewer: T,
    /// Browser result.
    pub browser: T,
}

impl<T> Profiles<T> {
    /// The value for one surface.
    pub fn get(&self, profile: HighlightProfile) -> &T {
        match profile {
            HighlightProfile::ExportViewer => &self.viewer,
            HighlightProfile::CodeBlock => &self.browser,
        }
    }
}

/// Language and score of the second ranked candidate.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct RunnerUp {
    /// Canonical language, absent for plain.
    pub language: Option<String>,
    /// Exact accumulated score.
    pub relevance: f64,
}

/// Complete externally selected result.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(super) struct Expected {
    /// Canonical language, absent for plain.
    pub language: Option<String>,
    /// Exact accumulated score.
    pub relevance: f64,
    /// Whether parsing rejected the input.
    pub illegal: bool,
    /// Complete second-place observation.
    pub second_best: RunnerUp,
}

/// One unique original string and both profile observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    /// Stable witness name.
    pub id: String,
    /// Complete original input.
    pub code: String,
    /// Independently recorded profile results.
    pub observed: Profiles<Expected>,
}

/// Complete unique-input corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Corpus {
    /// Ordered registration names.
    pub registrations: Profiles<Vec<String>>,
    /// All unique detection queries.
    pub inputs: Vec<Input>,
}

/// Decode the independent result corpus.
pub(super) fn corpus() -> Corpus {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/automatic_highlighting/detector-corpus.json"
    ))
    .unwrap()
}

/// Assert the complete ranked result without dropping score or order.
pub(super) fn assert_result(profile: HighlightProfile, input: &Input) {
    let rows = core::detect(profile, &input.code).unwrap();
    let expected = input.observed.get(profile);
    let best = &rows[0];
    let second = &rows[1];
    assert_eq!(
        best.language,
        expected.language.as_deref(),
        "{} {profile:?}",
        input.id
    );
    assert_eq!(
        best.relevance().to_bits(),
        expected.relevance.to_bits(),
        "{} {profile:?}",
        input.id
    );
    assert_eq!(
        matches!(
            best.outcome,
            crate::highlight::core::types::Relevance::Illegal
        ),
        expected.illegal,
        "{} {profile:?}",
        input.id
    );
    assert_eq!(
        second.language,
        expected.second_best.language.as_deref(),
        "{} {profile:?}",
        input.id
    );
    assert_eq!(
        second.relevance().to_bits(),
        expected.second_best.relevance.to_bits(),
        "{} {profile:?}",
        input.id
    );
    assert!(
        !matches!(
            second.outcome,
            crate::highlight::core::types::Relevance::Illegal
        ),
        "{} {profile:?}",
        input.id
    );
}

/// A losing or winning candidate's complete scoring observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Trial {
    /// Registration evaluated.
    pub language: String,
    /// Exact relevance.
    pub relevance: f64,
    /// Illegal rejection state.
    pub illegal: bool,
}

/// All eligible candidates for one recorded query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Candidates {
    /// Surface registry.
    pub profile: String,
    /// Input in the unique-input corpus.
    pub input_id: String,
    /// Ordered complete candidate observations.
    pub trials: Vec<Trial>,
}

/// Decode a fixture's surface identity.
pub(super) fn profile(name: &str) -> HighlightProfile {
    match name {
        "viewer" => HighlightProfile::ExportViewer,
        "browser" => HighlightProfile::CodeBlock,
        _ => panic!("unknown fixture profile"),
    }
}

/// Build a real-registry engine using the one shared table publication.
pub(super) fn engine(profile: HighlightProfile) -> core::Engine<'static> {
    core::Engine {
        tables: core::tables(profile).unwrap(),
        languages: super::super::languages::registered(profile),
        profile,
    }
}

/// A source-defined grammar and its independently measured observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeWitness {
    /// Surface version.
    profile: String,
    /// Controlled grammar registration.
    grammar: String,
    /// Complete original input.
    code: String,
    /// Exact score.
    relevance: f64,
    /// Illegal rejection flag.
    illegal: bool,
}

/// One shared compilation of the source-defined mode grammars.
static MODE_TABLES: std::sync::LazyLock<core::Tables> = std::sync::LazyLock::new(|| {
    let tables = core::Tables::load(
        include_str!("../../../tests/fixtures/automatic_highlighting/mode-patterns.json"),
        include_str!("../../../tests/fixtures/automatic_highlighting/mode-keywords.json"),
        include_str!("../../../tests/fixtures/automatic_highlighting/mode-grammars.json"),
    )
    .unwrap();
    tables
        .prepare_roots(super::grammars::LANGUAGES.iter().filter_map(|l| l.root))
        .unwrap();
    tables
});

/// Controlled grammars run through the production interpreter.
pub(super) fn controlled(profile: HighlightProfile) -> core::Engine<'static> {
    core::Engine {
        tables: &MODE_TABLES,
        languages: super::grammars::LANGUAGES,
        profile,
    }
}

/// Assert all independently recorded rows assigned to these grammar owners.
pub(super) fn witnesses(names: &[&str]) {
    let rows: Vec<ModeWitness> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/automatic_highlighting/mode-witnesses.json"
    ))
    .unwrap();
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| names.contains(&row.grammar.as_str()))
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let engine = controlled(profile(&row.profile));
        let language = engine
            .languages
            .iter()
            .find(|l| l.name == row.grammar)
            .unwrap();
        let result = engine
            .evaluate(language, &row.code, None, core::IllegalPolicy::Reject)
            .score;
        assert_eq!(result.language, Some(row.grammar.as_str()));
        assert_eq!(
            result.relevance().to_bits(),
            row.relevance.to_bits(),
            "{} {}",
            row.grammar,
            row.profile
        );
        assert_eq!(
            matches!(
                result.outcome,
                crate::highlight::core::types::Relevance::Illegal
            ),
            row.illegal,
            "{} {}",
            row.grammar,
            row.profile
        );
    }
}

/// Assert named real-profile cases through the complete selected result.
pub(super) fn inputs(names: &[&str]) {
    let corpus = corpus();
    for name in names {
        let input = corpus
            .inputs
            .iter()
            .find(|input| input.id == *name)
            .unwrap();
        for profile in [HighlightProfile::ExportViewer, HighlightProfile::CodeBlock] {
            assert_result(profile, input);
        }
    }
}
