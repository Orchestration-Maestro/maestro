//! Recognition of supplied foreground-job identities.
use super::{Agent, agent_label, identify_agent, normalized_agent_lookup_name, parse_agent_label};

/// Identifies the recognized group leader, otherwise the highest-priority member.
/// Equal-priority members retain their supplied order.
#[must_use]
pub fn identify_agent_in_job(
    job: &maestro_process::pty::foreground::ForegroundJob,
) -> Option<(Agent, String)> {
    if let Some(process) = job
        .processes
        .iter()
        .find(|process| process.pid == job.process_group_id)
    {
        let candidate = normalized_process_name(process);
        if let Some(agent) = identify_agent(&candidate) {
            return Some((agent, candidate));
        }
    }

    let mut best: Option<(u8, Agent, String)> = None;

    for process in &job.processes {
        let candidate = normalized_process_name(process);
        let Some(agent) = identify_agent(&candidate) else {
            continue;
        };
        let score = process_priority(process, &candidate);

        match &best {
            Some((best_score, _, _)) if *best_score >= score => {}
            _ => best = Some((score, agent, candidate)),
        }
    }

    best.map(|(_, agent, name)| (agent, name))
}

/// Selects the observed identity, including runtime scripts and executable paths.
fn normalized_process_name(
    process: &maestro_process::pty::foreground::ForegroundProcess,
) -> String {
    let effective = process.argv0.as_deref().unwrap_or(&process.name);
    let lower_effective = effective.to_lowercase();

    if is_generic_runtime_or_shell(&lower_effective)
        && let Some(wrapped_agent) =
            wrapped_agent_name_from_runtime_argv(&lower_effective, process.argv.as_deref())
    {
        return wrapped_agent;
    }

    if identify_agent(effective).is_some() {
        return effective.to_string();
    }

    if let Some(runtime) = process.argv.as_deref().and_then(|argv| argv.first())
        && matches!(
            normalized_agent_lookup_name(path_basename(runtime)).as_str(),
            "node" | "bun"
        )
        && let Some(wrapped_agent) =
            wrapped_agent_name_from_runtime_argv(runtime, process.argv.as_deref())
        && identify_agent(&wrapped_agent) == Some(Agent::Qwen)
    {
        return wrapped_agent;
    }

    if let Some(wrapped_agent) = argv0_agent_name(process.argv.as_deref())
        .or_else(|| cmdline_argv0_agent_name(process.cmdline.as_deref().unwrap_or_default()))
    {
        return wrapped_agent;
    }

    effective.to_string()
}

/// Recognizes an agent script under a supported runtime or shell.
fn wrapped_agent_name_from_runtime_argv(runtime: &str, argv: Option<&[String]>) -> Option<String> {
    let argv = argv?;
    let runtime_name = normalized_agent_lookup_name(path_basename(runtime));

    match runtime_name.as_str() {
        "node" | "bun" => script_arg_agent_name(argv, &["-e", "--eval", "-p", "--print"], &[]),
        name if is_python_runtime(name) => script_arg_agent_name(argv, &["-c"], &["-m"]),
        "sh" | "bash" | "zsh" | "fish" => script_arg_agent_name(argv, &["-c"], &[]),
        "cmd" => windows_cmd_arg_agent_name(argv),
        "powershell" | "pwsh" => powershell_arg_agent_name(argv),
        _ => None,
    }
}

/// Reads the command operand after a command-interpreter introducer.
fn windows_cmd_arg_agent_name(argv: &[String]) -> Option<String> {
    let mut operands = argv.iter().skip(1);
    while let Some(arg) = operands.next() {
        let flag = arg.trim_matches('"').to_lowercase();
        match flag.as_str() {
            "/c" | "/k" => {
                return operands
                    .next()
                    .and_then(|command| command_text_agent_name(command));
            }
            _ => {}
        }
    }
    None
}

/// Reads a file or command operand without decoding encoded commands.
fn powershell_arg_agent_name(argv: &[String]) -> Option<String> {
    let mut operands = argv.iter().skip(1);
    while let Some(arg) = operands.next() {
        let flag = arg.trim_matches('"').to_lowercase();
        match flag.as_str() {
            "-file" | "-f" | "/file" => {
                return operands
                    .next()
                    .and_then(|path| agent_name_from_path_token(path));
            }
            "-command" | "-c" | "/command" | "/c" => {
                return operands
                    .next()
                    .and_then(|command| command_text_agent_name(command));
            }
            "-encodedcommand" | "-enc" | "/encodedcommand" | "/enc" => return None,
            "-configurationname" | "-executionpolicy" | "-outputformat" | "-psconsolefile"
            | "-version" | "-windowstyle" | "-workingdirectory" => {
                let _ = operands.next();
            }
            _ if flag.starts_with('-') || flag.starts_with('/') => {}
            _ => return agent_name_from_path_token(arg),
        }
    }
    None
}

/// Skips invocation introducers and recognizes the first command token.
fn command_text_agent_name(command: &str) -> Option<String> {
    let mut rest = command;
    while let Some((token, next)) = command_text_token(rest) {
        let token = token.trim();
        if token.eq_ignore_ascii_case("&")
            || token.eq_ignore_ascii_case(".")
            || token.eq_ignore_ascii_case("call")
        {
            rest = next;
            continue;
        }
        return agent_name_from_path_token(token);
    }
    None
}

/// Splits the next quoted or whitespace-delimited command word.
fn command_text_token(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    let first = input.chars().next()?;
    if first == '"' || first == '\'' {
        let start = first.len_utf8();
        if let Some(end) = input[start..].find(first) {
            let end = start + end;
            return Some((&input[start..end], &input[end + first.len_utf8()..]));
        }
        return Some((&input[start..], ""));
    }

    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    Some((&input[..end], &input[end..]))
}

/// Finds the script operand while rejecting inline code and module flags.
fn script_arg_agent_name(
    argv: &[String],
    eval_flags: &[&str],
    module_flags: &[&str],
) -> Option<String> {
    let mut operands = argv.iter().skip(1);
    while let Some(arg) = operands.next() {
        if arg == "--" {
            return operands
                .next()
                .and_then(|token| agent_name_from_path_token(token));
        }

        if flag_matches(arg, eval_flags) || flag_matches(arg, module_flags) {
            return None;
        }

        if arg.starts_with('-') {
            if option_takes_value(arg) {
                let _ = operands.next();
            }
            continue;
        }

        return agent_name_from_path_token(arg);
    }

    None
}

/// Recognizes standalone flags and their attached payload forms.
fn flag_matches(arg: &str, flags: &[&str]) -> bool {
    flags
        .iter()
        .any(|flag| arg == *flag || short_flag_payload(arg, flag) || long_flag_value(arg, flag))
}

/// Tests whether a short flag has an attached payload.
fn short_flag_payload(arg: &str, flag: &str) -> bool {
    flag.starts_with('-')
        && !flag.starts_with("--")
        && arg.starts_with(flag)
        && arg.len() > flag.len()
}

/// Tests whether a long flag has an equals-delimited payload.
fn long_flag_value(arg: &str, flag: &str) -> bool {
    flag.starts_with("--")
        && arg
            .strip_prefix(flag)
            .is_some_and(|rest| rest.starts_with('='))
}

/// Identifies runtime options whose next argument is not the script.
fn option_takes_value(arg: &str) -> bool {
    matches!(
        arg,
        "-r" | "--require"
            | "--loader"
            | "--import"
            | "--experimental-loader"
            | "--inspect-port"
            | "-W"
            | "-X"
            | "-S"
            | "-L"
            | "-o"
    )
}

/// Recognizes the first supplied argument as an executable path.
fn argv0_agent_name(argv: Option<&[String]>) -> Option<String> {
    agent_name_from_path_token(argv?.first()?)
}

/// Recognizes the first whitespace-delimited command-line word.
fn cmdline_argv0_agent_name(cmdline: &str) -> Option<String> {
    agent_name_from_path_token(cmdline.split_whitespace().next()?)
}

/// Recognizes executable basenames, known package paths and symbolic-link targets.
fn agent_name_from_path_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c| matches!(c, '"' | '\''));
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }

    agent_name_from_basename(path_basename(trimmed))
        .or_else(|| agent_name_from_known_package_path(trimmed))
        .or_else(|| resolved_agent_name_from_path_token(trimmed))
}

/// Recognizes the shipped command entry points inside supported packages.
fn agent_name_from_known_package_path(path: &str) -> Option<String> {
    let components: Vec<String> = path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .map(normalized_agent_lookup_name)
        .collect();

    for window in components.windows(5) {
        if window
            == [
                "node_modules",
                "@earendil-works",
                "pi-coding-agent",
                "dist",
                "cli",
            ]
        {
            return Some(agent_label(Agent::Pi).to_string());
        }
        if window == ["node_modules", "@qwen-code", "qwen-code", "dist", "index"] {
            return Some(agent_label(Agent::Qwen).to_string());
        }
    }
    for window in components.windows(4) {
        if window == ["node_modules", "mastracode", "dist", "cli"] {
            return Some(agent_label(Agent::Mastracode).to_string());
        }
    }
    None
}

/// Resolves a multi-component path to a recognized executable basename.
fn resolved_agent_name_from_path_token(token: &str) -> Option<String> {
    let path = std::path::Path::new(token);
    if path.components().count() < 2 {
        return None;
    }

    let resolved = std::fs::canonicalize(path).ok()?;
    let basename = resolved.file_name()?.to_str()?;
    agent_name_from_basename(basename)
}

/// Converts a recognized basename to its canonical label.
fn agent_name_from_basename(basename: &str) -> Option<String> {
    let agent = parse_agent_label(basename)?;
    Some(agent_label(agent).to_string())
}

/// Returns the last nonempty component under either path separator.
fn path_basename(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

/// Ranks wrapped identities ahead of native agents and generic runtimes.
fn process_priority(
    process: &maestro_process::pty::foreground::ForegroundProcess,
    normalized_name: &str,
) -> u8 {
    let lower_name = normalized_name.to_lowercase();
    if lower_name != process.name.to_lowercase() {
        return 3;
    }
    if !is_generic_runtime_or_shell(&lower_name) {
        return 2;
    }
    1
}

/// Recognizes runtime and shell names that may wrap an agent.
fn is_generic_runtime_or_shell(name: &str) -> bool {
    let name = normalized_agent_lookup_name(path_basename(name));
    is_python_runtime(&name)
        || matches!(
            name.as_str(),
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "tmux"
                | "node"
                | "bun"
                | "cmd"
                | "powershell"
                | "pwsh"
        )
}

/// Recognizes Python with an optional dotted numeric version.
fn is_python_runtime(name: &str) -> bool {
    name == "python"
        || name.strip_prefix("python").is_some_and(|version| {
            !version.is_empty()
                && version
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        })
}

#[cfg(test)]
mod tests;
