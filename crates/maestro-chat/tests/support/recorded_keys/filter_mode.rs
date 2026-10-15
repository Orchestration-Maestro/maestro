//! Recorded tree category vocabulary.
use maestro_chat::FilterMode;
/// Converts only the accepted recorded filter vocabulary.
pub fn mode(value: &str) -> FilterMode {
    match value {
        "default" => FilterMode::Default,
        "no-tools" => FilterMode::NoTools,
        "user-only" => FilterMode::UserOnly,
        "labeled-only" => FilterMode::LabeledOnly,
        "all" => FilterMode::All,
        _ => panic!("unrecognized fixture mode"),
    }
}
