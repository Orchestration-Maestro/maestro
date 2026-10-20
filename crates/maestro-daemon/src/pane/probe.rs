//! Agent classification from foreground process facts and environment hints.

use maestro_detection::agents::{
    Agent, agent_label, identify_agent_in_job, identify_agent_in_process, parse_agent_label,
};
use maestro_process::pty::foreground::{self, ForegroundJob, ProcessDetectionMode};

/// Agent identity confirmation.
pub mod presence;
/// Process-probe scheduling.
pub mod schedule;

/// Recognizes the raw agent environment value supplied by the process owner.
#[must_use]
pub fn parse_agent_env_hint(value: Option<&str>) -> Option<Agent> {
    parse_agent_label(value?)
}

/// Result of a foreground process scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessProbeResult {
    /// Observed or inferred process group.
    pub process_group_id: Option<u32>,
    /// Whether the observed job contains the pane shell.
    pub foreground_is_pane_shell: bool,
    /// Recognized agent identity.
    pub agent: Option<Agent>,
    /// Effective executable name or canonical environment label.
    pub process_name: Option<String>,
}

/// Reads the leader's hint before any non-leader hint.
#[must_use]
pub fn agent_hint_for_foreground_job_members(
    job: &ForegroundJob,
    read_hint: impl Fn(u32) -> Option<Agent>,
) -> Option<Agent> {
    read_hint(job.process_group_id)
        .or_else(|| agent_hint_for_non_leader_foreground_job_members(job, read_hint))
}

/// Reads hints from non-leader members in observation order.
fn agent_hint_for_non_leader_foreground_job_members(
    job: &ForegroundJob,
    read_hint: impl Fn(u32) -> Option<Agent>,
) -> Option<Agent> {
    job.processes
        .iter()
        .filter(|process| process.pid != job.process_group_id)
        .find_map(|process| read_hint(process.pid))
}

/// Classifies only the observed leader using the shared detection owner.
fn identify_process_group_leader_in_job(job: &ForegroundJob) -> Option<(Agent, String)> {
    let leader = job
        .processes
        .iter()
        .find(|process| process.pid == job.process_group_id)?;
    identify_agent_in_process(leader)
}

/// Attaches job facts to a classification result.
fn process_probe_result(
    job: &ForegroundJob,
    pid: u32,
    agent: Agent,
    name: String,
) -> ProcessProbeResult {
    ProcessProbeResult {
        process_group_id: Some(job.process_group_id),
        foreground_is_pane_shell: job.processes.iter().any(|process| process.pid == pid),
        agent: Some(agent),
        process_name: Some(name),
    }
}

/// Resolves leader hints, leader names, member hints and member names in order.
#[must_use]
pub fn probe_foreground_process_from_jobs(
    pid: u32,
    foreground_pgid: Option<u32>,
    leader_job: Option<&ForegroundJob>,
    foreground_job: impl FnOnce() -> Option<ForegroundJob>,
    read_hint: impl Fn(u32) -> Option<Agent> + Copy,
) -> ProcessProbeResult {
    if let Some(job) = leader_job {
        if let Some(agent) = agent_hint_for_foreground_job_members(job, read_hint) {
            return process_probe_result(job, pid, agent, agent_label(agent).to_owned());
        }
        if let Some((agent, name)) = identify_agent_in_job(job) {
            return process_probe_result(job, pid, agent, name);
        }
    }
    if let Some(job) = foreground_job() {
        if let Some(agent) = read_hint(job.process_group_id) {
            return process_probe_result(&job, pid, agent, agent_label(agent).to_owned());
        }
        if let Some((agent, name)) = identify_process_group_leader_in_job(&job) {
            return process_probe_result(&job, pid, agent, name);
        }
        if let Some(agent) = agent_hint_for_non_leader_foreground_job_members(&job, read_hint) {
            return process_probe_result(&job, pid, agent, agent_label(agent).to_owned());
        }
        let identified = identify_agent_in_job(&job);
        return ProcessProbeResult {
            process_group_id: Some(job.process_group_id),
            foreground_is_pane_shell: job.processes.iter().any(|process| process.pid == pid),
            agent: identified.as_ref().map(|(agent, _)| *agent),
            process_name: identified.map(|(_, name)| name),
        };
    }
    ProcessProbeResult {
        process_group_id: foreground_pgid,
        foreground_is_pane_shell: false,
        agent: None,
        process_name: None,
    }
}

/// Scans native foreground jobs, keeping process I/O in the process owner.
#[must_use]
pub fn probe_foreground_process(
    pid: u32,
    foreground_pgid: Option<u32>,
    mode: ProcessDetectionMode,
) -> ProcessProbeResult {
    probe_foreground_process_from_jobs(
        pid,
        foreground_pgid,
        foreground_pgid
            .and_then(foreground::foreground_group_leader_job)
            .as_ref(),
        || foreground::foreground_job(pid, mode),
        |pid| {
            parse_agent_env_hint(
                foreground::process_environment_value(pid, "MAESTRO_AGENT").as_deref(),
            )
        },
    )
}

#[cfg(test)]
mod tests;
