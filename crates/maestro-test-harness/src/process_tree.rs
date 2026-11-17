//! Unix test-command ownership that survives abrupt death of the test process.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid};

/// A command tree whose guardian stops it when the owning test disappears.
///
/// Descendants must remain in the command's process group. The command receives null
/// stdin; stdout and stderr retain the caller's configuration.
#[must_use = "dropping the guard stops its process group"]
#[derive(Debug)]
pub struct ProcessGroup {
    /// The guardian, with the sole ownership-pipe writer in its stdin handle.
    child: Child,
}

impl ProcessGroup {
    /// Prepare `program` under the native guardian; append the program's operands normally.
    ///
    /// # Errors
    /// Returns an error when nextest did not export the prepared native fixture.
    ///
    /// # Examples
    /// ```no_run
    /// use maestro_test_harness::ProcessGroup;
    /// let mut command = ProcessGroup::command("just")?;
    /// command.arg("dev");
    /// let mut watch = ProcessGroup::spawn(command)?;
    /// watch.stop()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub fn command(program: impl AsRef<OsStr>) -> io::Result<Command> {
        let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
            .ok_or_else(|| io::Error::other("nextest must supply MAESTRO_NATIVE_FIXTURE"))?;
        let mut command = Command::new(artifact);
        command.arg("--maestro-guard").arg(program);
        Ok(command)
    }

    /// Launch a command prepared by [`Self::command`], reserving stdin for its ownership pipe.
    ///
    /// # Errors
    /// Returns failures starting the guardian. Program launch failures are reported
    /// on stderr and through [`Self::stop`].
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        command.stdin(Stdio::piped()).process_group(0);
        Ok(Self {
            child: command.spawn()?,
        })
    }

    /// Close the ownership pipe and wait for group cleanup and guardian exit.
    ///
    /// Repeated calls return the same outcome without signalling another process.
    ///
    /// # Errors
    /// Returns failures waiting for the guardian or an unsuccessful guardian status.
    pub fn stop(&mut self) -> io::Result<()> {
        drop(self.child.stdin.take());
        let status = self.child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "test process guardian failed: {status}"
            )))
        }
    }
}

impl Drop for ProcessGroup {
    /// Use the same ownership release as an explicit stop.
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::error!(%error, "cannot stop test process group");
        }
    }
}

/// A live group retained until termination and leader reaping have both completed.
struct Tree {
    /// The unreaped leader keeps the group identity from being reused.
    child: Option<Child>,
}

impl Tree {
    /// Kill the group once, reap its leader and collect adopted members of the killed group.
    fn stop(&mut self) -> io::Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let group = Pid::from_child(child);
        match kill_process_group(group, Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
        child.wait()?;
        self.child.take();
        #[cfg(target_os = "linux")]
        reap_adopted(group)?;
        Ok(())
    }
}

impl Drop for Tree {
    /// Also clean up when creating an observer or reading the control pipe fails.
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::error!(%error, "cannot reap guarded test command");
        }
    }
}

/// Run the prepared native fixture's guardian operation.
///
/// This entry is used only by the harness-free native fixture. The guardian uses its
/// stdin as an EOF ownership signal, not as the watched command's input.
///
/// # Errors
/// Returns invalid operands, launch, observation or group-cleanup failures.
#[doc(hidden)]
pub fn run_group_guardian(operands: &[OsString]) -> io::Result<()> {
    let Some((program, args)) = operands.split_first() else {
        return Err(io::Error::other("the test guardian requires a program"));
    };
    #[cfg(target_os = "linux")]
    rustix::process::set_child_subreaper(Some(Pid::INIT))?;
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()?;
    let pid = Pid::from_child(&child);
    let mut tree = Tree { child: Some(child) };
    let (send, receive) = mpsc::channel();
    let control = send.clone();
    std::thread::Builder::new()
        .name("test ownership".into())
        .spawn(move || {
            let result = io::copy(&mut io::stdin().lock(), &mut io::sink()).map(|_| ());
            // Cleanup may already have won the race and released the receiver.
            let _ = control.send(result);
        })?;
    std::thread::Builder::new()
        .name("test command exit".into())
        .spawn(move || {
            // Leave the leader unreaped until the entire group has been signalled.
            let result = observe_exit(pid);
            // EOF may already have started cleanup and released the receiver.
            let _ = send.send(result);
        })?;
    let observed = receive.recv().map_err(io::Error::other)?;
    let stopped = tree.stop();
    observed?;
    stopped
}

/// Wait for leader exit without releasing its process-group identity.
fn observe_exit(pid: Pid) -> io::Result<()> {
    loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
            Err(rustix::io::Errno::INTR) => {}
            result => return result.map(|_| ()).map_err(Into::into),
        }
    }
}

/// Reap adopted members of the killed group without waiting for escaped daemons.
#[cfg(target_os = "linux")]
fn reap_adopted(group: Pid) -> io::Result<()> {
    loop {
        match waitid(WaitId::Pgid(Some(group)), WaitIdOptions::EXITED) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(rustix::io::Errno::CHILD) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
}
