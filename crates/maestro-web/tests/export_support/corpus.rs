//! Shared complete rendering-corpus records.
pub use tests::*;
#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::Value;

    /// One producer operation and its recorded observations.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Row {
        /// Distinguishable corpus case.
        pub id: String,
        /// Invoked operation.
        pub operation: String,
        /// Complete raw arguments.
        pub input: Value,
        /// Owning behavioral witness.
        pub test: String,
        /// Success or reached error.
        pub branch: String,
        /// Complete expected observations.
        pub expected: Value,
    }

    /// Select one witness's cases, preserving repeated inputs under different operations.
    pub fn rows(test: &str, operation: &str) -> Vec<Row> {
        let rows: Vec<Row> =
            serde_json::from_str(include_str!("../fixtures/session_rendering.json")).unwrap();
        let rows: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
        assert!(!rows.is_empty());
        for row in &rows {
            assert_eq!(row.operation, operation, "{}", row.id);
        }
        rows
    }
}
