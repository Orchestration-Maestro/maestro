//! Instrumented child emits a real profile before its parent awaits group cleanup.

use std::{env, io::Write, process::Command, thread};

/// Waits for the finite profile producer before announcing readiness on shared output.
fn main() {
    if env::args().nth(1).as_deref() == Some("exit") {
        return;
    }
    assert!(
        Command::new(env::current_exe().unwrap())
            .arg("exit")
            .status()
            .unwrap()
            .success()
    );
    println!("maestro-profile-child-exited");
    std::io::stdout().flush().unwrap();
    loop {
        thread::park();
    }
}
