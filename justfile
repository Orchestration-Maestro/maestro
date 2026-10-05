# Install the pinned tools and both commit hook stages.
setup:
    mise install
    mise exec -- prek install

# Formatting, lint, public documentation and workspace conventions.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps --locked
    cargo test -p maestro-test-conventions --locked

# Run all workspace tests.
test:
    cargo test --workspace --locked

# Run the same checks as continuous integration.
ci: check test

# Format the workspace, then re-stage the files staged for the commit.
format-staged:
    cargo fmt --all
    git diff --cached --name-only -z --diff-filter=ACMR | xargs -0 git add --

# The commit hook: format and re-stage, then run every check.
pre-commit: format-staged check
