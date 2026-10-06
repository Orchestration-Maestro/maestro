//! Persistent workspace build launcher.
#[path = "../dev_watch.rs"]
mod dev_watch;

fn main() -> std::process::ExitCode {
    dev_watch::main()
}
