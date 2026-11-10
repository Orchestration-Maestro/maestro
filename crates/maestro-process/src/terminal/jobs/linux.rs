//! Linux procfs process observations.
use super::{
    ForegroundJob, ForegroundProcess, ProcGroupMember, ProcessDetectionMode,
    child_groups_foreground_process_group_with, foreground_process_group_members_with,
};
use std::{
    os::fd::AsFd,
    path::{Path, PathBuf},
};

/// A procfs observation root; each operation reads fresh records.
pub(super) struct Procfs<'a>(pub(super) &'a Path);

impl Procfs<'_> {
    /// Borrows the host process filesystem.
    fn native() -> Self {
        Self(Path::new("/proc"))
    }

    /// Reads one process record as text.
    fn text(&self, pid: u32, name: &str) -> Option<String> {
        std::fs::read_to_string(self.0.join(pid.to_string()).join(name)).ok()
    }

    /// Parses group and command from a process stat record.
    fn group_and_name(&self, pid: u32) -> Option<(i32, String)> {
        process_pgrp_and_comm_from_stat(&self.text(pid, "stat")?)
    }

    /// Reads one numeric field following the parenthesized process name.
    fn stat_field(&self, pid: u32, index: usize) -> Option<i32> {
        let stat = self.text(pid, "stat")?;
        stat.get(stat.rfind(')')? + 2..)?
            .split_whitespace()
            .nth(index)?
            .parse()
            .ok()
    }

    /// Enumerates numeric task identities for a process.
    fn task_ids(&self, pid: u32) -> Vec<u32> {
        numeric_entries(&self.0.join(pid.to_string()).join("task"))
    }

    /// Reads the direct children of one task.
    fn task_children(&self, pid: u32, tid: u32) -> Vec<u32> {
        self.text(pid, &format!("task/{tid}/children"))
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|value| value.parse().ok())
            .collect()
    }

    /// Reads argv without converting native cwd or environment values.
    fn argv(&self, pid: u32) -> Option<Vec<String>> {
        let bytes = std::fs::read(self.0.join(pid.to_string()).join("cmdline")).ok()?;
        process_argv_from_bytes(&bytes)
    }

    /// Attaches arguments to one selected group member.
    fn process(&self, member: ProcGroupMember) -> ForegroundProcess {
        let argv = self.argv(member.pid);
        ForegroundProcess {
            pid: member.pid,
            name: member.comm,
            argv0: None,
            cmdline: argv.as_ref().map(|argv| argv.join(" ")),
            argv,
        }
    }

    /// Filters the fresh descendants of both roots to the selected group.
    fn job_for_group(&self, child: u32, group: u32) -> Option<ForegroundJob> {
        let members = foreground_process_group_members_with(
            child,
            group,
            |pid| self.task_ids(pid),
            |pid, tid| self.task_children(pid, tid),
            |group, pid| {
                let (pgrp, comm) = self.group_and_name(pid)?;
                (u32::try_from(pgrp).ok() == Some(group)).then_some(ProcGroupMember { pid, comm })
            },
        )?;
        Some(ForegroundJob {
            process_group_id: group,
            processes: members
                .into_iter()
                .map(|member| self.process(member))
                .collect(),
        })
    }

    /// Selects the native group before considering the explicit fallback.
    pub(super) fn foreground_job(
        &self,
        child: u32,
        mode: ProcessDetectionMode,
    ) -> Option<ForegroundJob> {
        if let Some(group) = self
            .stat_field(child, 5)
            .and_then(|group| u32::try_from(group).ok())
            .filter(|group| *group > 0)
        {
            return self.job_for_group(child, group);
        }
        if mode != ProcessDetectionMode::ChildGroups {
            return None;
        }
        let (shell, _) = self.group_and_name(child)?;
        let shell = u32::try_from(shell).ok().filter(|group| *group > 0)?;
        let group = child_groups_foreground_process_group_with(
            child,
            shell,
            |pid| self.task_ids(pid),
            |pid, tid| self.task_children(pid, tid),
            |pid| self.group_and_name(pid).map(|(group, _)| group),
        )?;
        self.job_for_group(child, group)
    }

    /// Returns the directly readable leader only when its group matches.
    pub(super) fn leader(&self, group: u32) -> Option<ForegroundJob> {
        let (pgrp, comm) = self.group_and_name(group)?;
        if u32::try_from(pgrp).ok() != Some(group) {
            return None;
        }
        Some(ForegroundJob {
            process_group_id: group,
            processes: vec![self.process(ProcGroupMember { pid: group, comm })],
        })
    }

    /// Lists processes sharing a live member's session.
    pub(super) fn session_processes(&self, child: u32) -> Vec<u32> {
        self.stat_field(child, 3)
            .and_then(|session| u32::try_from(session).ok())
            .map_or_else(Vec::new, |session| self.session_processes_by_id(session))
    }

    /// Lists members by their original session identity, without reading its leader.
    pub(super) fn session_processes_by_id(&self, session: u32) -> Vec<u32> {
        let Some(session) = i32::try_from(session).ok().filter(|session| *session > 0) else {
            return Vec::new();
        };
        numeric_entries(self.0)
            .into_iter()
            .filter(|pid| self.stat_field(*pid, 3) == Some(session))
            .collect()
    }
}

/// Reads numeric directory names, ignoring unreadable or disappearing entries.
fn numeric_entries(path: &Path) -> Vec<u32> {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| name.parse().ok())
                .flatten()
        })
        .collect()
}

/// Parses a process name containing spaces or closing parentheses.
pub(super) fn process_pgrp_and_comm_from_stat(stat: &str) -> Option<(i32, String)> {
    let close = stat.rfind(')')?;
    let name = stat.get(stat.find('(')? + 1..close)?.to_owned();
    let group = stat
        .get(close + 2..)?
        .split_whitespace()
        .nth(2)?
        .parse()
        .ok()?;
    Some((group, name))
}

/// Keeps nonempty NUL-separated arguments with lossy decoding.
pub(super) fn process_argv_from_bytes(bytes: &[u8]) -> Option<Vec<String>> {
    let argv: Vec<_> = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    (!argv.is_empty()).then_some(argv)
}

/// Returns the first matching environment value, including empty values.
pub(super) fn environment_value(bytes: &[u8], name: &str) -> Option<String> {
    let value = bytes
        .split(|byte| *byte == 0)
        .find_map(|record| record.strip_prefix(name.as_bytes())?.strip_prefix(b"="))?;
    std::str::from_utf8(value).ok().map(str::to_owned)
}

/// Collects live members of the terminal's foreground group.
#[must_use]
pub fn foreground_job(child_pid: u32, mode: ProcessDetectionMode) -> Option<ForegroundJob> {
    Procfs::native().foreground_job(child_pid, mode)
}

/// Reads a group leader whose stat record confirms its group identity.
#[must_use]
pub fn foreground_group_leader_job(group: u32) -> Option<ForegroundJob> {
    Procfs::native().leader(group)
}

/// Reads a positive native terminal foreground group from a child stat record.
#[must_use]
pub fn foreground_process_group_id(child_pid: u32) -> Option<u32> {
    Procfs::native()
        .stat_field(child_pid, 5)
        .and_then(|group| u32::try_from(group).ok())
        .filter(|group| *group > 0)
}

/// Queries the terminal foreground group through a borrowed descriptor.
#[must_use]
pub fn foreground_process_group_id_for_tty_fd(fd: impl AsFd) -> Option<u32> {
    rustix::termios::tcgetpgrp(fd)
        .ok()
        .map(rustix::process::Pid::as_raw_nonzero)
        .map(std::num::NonZeroI32::get)
        .and_then(|pid| u32::try_from(pid).ok())
}

/// Reads the process's native working-directory path.
#[must_use]
pub fn process_cwd(pid: u32) -> Option<PathBuf> {
    if pid == 0 {
        return None;
    }
    std::fs::read_link(Path::new("/proc").join(pid.to_string()).join("cwd")).ok()
}

/// Reads the first matching raw environment value with strict UTF-8 decoding.
#[must_use]
pub fn process_environment_value(pid: u32, name: &str) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let bytes = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    environment_value(&bytes, name)
}

/// Lists readable processes in a live member's session.
#[must_use]
pub fn session_processes(child_pid: u32) -> Vec<u32> {
    Procfs::native().session_processes(child_pid)
}

/// Lists readable members of a retained terminal session, even after its leader is reaped.
///
/// Terminal setup creates a new session whose identity is the spawned child's PID.
/// Pass that original identity, not the PID of an arbitrary member.
#[must_use]
pub fn session_processes_by_id(session_id: u32) -> Vec<u32> {
    Procfs::native().session_processes_by_id(session_id)
}
