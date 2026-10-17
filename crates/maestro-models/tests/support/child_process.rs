//! Case-based child helpers, re-exported from the checked test-process owner.

pub(crate) use maestro_test_process::*;

#[cfg(test)]
mod tests {
    #[test]
    fn mismatched_child_selector_is_rejected() {
        assert!(super::capture("not_this_test", "mismatch", &[]).is_err());
    }
}
