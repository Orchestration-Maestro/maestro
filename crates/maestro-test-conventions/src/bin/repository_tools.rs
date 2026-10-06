//! Launches private repository development commands.
#[path = "../repository_tools/mod.rs"]
mod repository_tools;

fn main() -> std::process::ExitCode {
    repository_tools::main()
}
