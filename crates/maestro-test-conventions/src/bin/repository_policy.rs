//! Executes trusted repository contribution policy metadata.
use maestro_test_conventions::contribution_policy;

fn main() {
    if let Err(error) = contribution_policy::execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
