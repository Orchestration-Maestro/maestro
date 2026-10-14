//! Public history qualification witnesses.

use crate::history_handoffs::{chat::TestResult, qualification};

/// Recorded public observations for handoff table keeps distinct routes.
#[test]
fn maestro_handoff_table_keeps_distinct_routes() -> TestResult {
    qualification::assert_rows("maestro_handoff_table_keeps_distinct_routes")
}

/// Recorded public observations for handoff auth selection keeps upstream headers.
#[test]
fn maestro_handoff_auth_selection_keeps_upstream_headers() -> TestResult {
    qualification::assert_rows("maestro_handoff_auth_selection_keeps_upstream_headers")
}
