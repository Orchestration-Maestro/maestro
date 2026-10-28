//! Opt-in interactive user shell commands with full terminal access.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashResult, Container, CustomFactoryOutcome, ExtensionAPI, ExtensionComponent,
    ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture, ExtensionResult,
    Presence, TUI, UserBashEventResult,
};
use std::rc::Rc;

/// Editors, pagers, interactive Git operations and terminal applications.
const DEFAULT_INTERACTIVE_COMMANDS: &[&str] = &[
    "vim",
    "nvim",
    "vi",
    "nano",
    "emacs",
    "pico",
    "micro",
    "helix",
    "hx",
    "kak",
    "less",
    "more",
    "most",
    "git commit",
    "git rebase",
    "git merge",
    "git cherry-pick",
    "git revert",
    "git add -p",
    "git add --patch",
    "git add -i",
    "git add --interactive",
    "git stash -p",
    "git stash --patch",
    "git reset -p",
    "git reset --patch",
    "git checkout -p",
    "git checkout --patch",
    "git difftool",
    "git mergetool",
    "htop",
    "top",
    "btop",
    "glances",
    "ranger",
    "nnn",
    "lf",
    "mc",
    "vifm",
    "tig",
    "lazygit",
    "gitui",
    "fzf",
    "sk",
    "ssh",
    "telnet",
    "mosh",
    "psql",
    "mysql",
    "sqlite3",
    "mongosh",
    "redis-cli",
    "kubectl edit",
    "kubectl exec -it",
    "docker exec -it",
    "docker run -it",
    "tmux",
    "screen",
    "ncdu",
];

/// Matches a complete prefix, not another command beginning with the same letters.
fn matches_prefix(command: &str, prefix: &str) -> bool {
    command == prefix
        || command
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with([' ', '\t']))
}

/// Reads comma-separated prefixes at command time for case-insensitive detection.
fn configured(name: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Detects the start or last pipe operand without parsing shell syntax.
fn is_interactive(command: &str) -> bool {
    let command = command.trim().to_lowercase();
    let added = configured("INTERACTIVE_COMMANDS");
    let excluded = configured("INTERACTIVE_EXCLUDE");
    DEFAULT_INTERACTIVE_COMMANDS
        .iter()
        .copied()
        .chain(added.iter().map(String::as_str))
        .filter(|prefix| !excluded.iter().any(|excluded| excluded == prefix))
        .any(|prefix| {
            matches_prefix(&command, prefix)
                || command.rsplit_once('|').is_some_and(|(_, tail)| {
                    let tail = tail.trim();
                    tail == prefix
                        || tail
                            .strip_prefix(prefix)
                            .is_some_and(|rest| rest.starts_with(' '))
                })
        })
}

/// Prevents normal shell execution with an already-produced result.
fn result(output: String, code: i32) -> ExtensionEventResult {
    ExtensionEventResult::UserBash(UserBashEventResult {
        operations: None,
        result: Presence::Present(BashResult {
            output,
            exit_code: Presence::Present(f64::from(code)),
            cancelled: false,
            truncated: false,
            full_output_path: Presence::Missing,
        }),
    })
}

/// Handles the user command without intercepting agent tool execution.
fn on_event(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    api: ExtensionAPI,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::UserBash(event) = event else {
            return Ok(None);
        };
        let forced = event
            .command
            .strip_prefix("i ")
            .or_else(|| event.command.strip_prefix("i\t"));
        if forced.is_none() && !is_interactive(&event.command) {
            return Ok(None);
        }
        if !ctx.has_ui()? {
            return Ok(Some(result("(interactive commands require TUI)".into(), 1)));
        }
        let command = forced.map_or_else(
            || event.command.clone(),
            |command| command.trim().to_owned(),
        );
        let cwd = ctx.cwd()?;
        let completion = ctx
            .ui()?
            .custom(
                Rc::new(move |tui, _, _, done, _| {
                    let (api, cwd, command) = (api.clone(), cwd.clone(), command.clone());
                    Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                        let completion = run(&tui, &api, cwd, command).await;
                        done(completion)?;
                        Ok(ExtensionComponent {
                            component: (&Container::new()?).into(),
                            dispose: None,
                            set_expanded: None,
                            width: None,
                        })
                    })))
                }),
                None,
            )
            .await?
            .unwrap_or(Ok(None));
        Ok(Some(completed(completion)))
    })
}

/// Runs with inherited descriptors, always attempting terminal restoration before returning.
async fn run(
    tui: &TUI,
    api: &ExtensionAPI,
    cwd: String,
    command: String,
) -> ExtensionResult<Option<i32>> {
    tui.stop()?;
    let completion = async {
        tui.terminal()?.write("\x1b[2J\x1b[H")?;
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        let io = api.native_io();
        let child = io
            .spawn(
                shell,
                vec!["-c".into(), command],
                maestro_extensions_wasm::child_process::SpawnOptions {
                    cwd: Some(cwd),
                    stdio: [maestro_extensions_wasm::child_process::Stdio::Inherit; 3],
                    ..Default::default()
                },
            )
            .await?;
        Ok(child.wait().await?.code)
    }
    .await;
    let restarted = tui.start();
    let redrawn = tui.request_render(true);
    restarted?;
    redrawn?;
    completion
}

/// Keeps native errors visible while preventing fallback execution of the same command.
fn completed(completion: ExtensionResult<Option<i32>>) -> ExtensionEventResult {
    match completion {
        Ok(Some(0)) => result("(interactive command completed successfully)".into(), 0),
        Ok(code) => {
            let display = code.map_or_else(|| "null".into(), |code| code.to_string());
            result(
                format!("(interactive command exited with code {display})"),
                code.unwrap_or(1),
            )
        }
        Err(error) => result(format!("(interactive command failed: {error})"), 1),
    }
}

/// Registers interactive user shell handling.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.on(
            "user_bash",
            Rc::new(move |event, ctx| on_event(event, ctx, registered.clone())),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
