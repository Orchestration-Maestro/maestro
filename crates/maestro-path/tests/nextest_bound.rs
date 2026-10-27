//! Disposable qualification of the external test-runner deadline.

#[test]
fn nextest_terminates_and_names_overlong_test() {
    std::thread::sleep(std::time::Duration::from_secs(370));
}
