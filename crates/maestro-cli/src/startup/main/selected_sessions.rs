//! The session startup selects, settled before any directory-bound service exists.
use super::bootstrap::AppMode;
use super::inputs::prompt_confirm;
use super::presentation::{MissingSessionCwdPresentation, prompt_for_missing_session_cwd};
use super::sessions::{
    SessionOperation, SessionSelection, fork_session, open_selected_session,
    select_session_operation,
};
use super::writers::SharedWriter;
use crate::input::args::Args;
use crate::input::session_picker::{SessionPresentation, select_session};
use crate::packages::package_manager_cli::presentation::line;
use maestro_app::presentation_data::input_selectors::{SessionListProgress, SessionsLoader};
use maestro_app::{
    MissingSessionCwdError, SessionManager, SettingsManager, get_missing_session_cwd_issue,
};
use maestro_theme::{Theme, ThemeState, ThemeWatchOperations};
use std::{cell::RefCell, error::Error, io, io::Write, ops::ControlFlow, rc::Rc};
use tokio::io::AsyncBufRead;

/// A startup phase's result: continue with a value, or end startup with a status.
type Phase<T> = Result<ControlFlow<i32, T>, Box<dyn Error>>;

/// The startup directories a session is looked up and created in.
pub(super) struct SessionDirs {
    /// The original working directory.
    pub(super) cwd: String,
    /// The user configuration directory.
    pub(super) agent_dir: String,
    /// The selected session directory, when one is configured.
    pub(super) session_dir: Option<String>,
}

/// The owners and streams the session phase reads and writes.
pub(super) struct SessionEffects<'a> {
    /// Startup settings over the original working directory.
    pub(super) settings: Rc<RefCell<SettingsManager>>,
    /// The theme a session selector presents with.
    pub(super) theme: Rc<ThemeState>,
    /// Watch effects of the selector's theme.
    pub(super) watcher: Rc<dyn ThemeWatchOperations>,
    /// Answers to the foreign-session confirmation.
    pub(super) stdin: &'a mut (dyn AsyncBufRead + Unpin),
    /// Ordinary output.
    pub(super) stdout: Rc<RefCell<dyn Write>>,
    /// Diagnostic output.
    pub(super) stderr: Rc<RefCell<dyn Write>>,
    /// Whether terminal styles are enabled.
    pub(super) styles: bool,
}

/// The presentations the session phase may show, each at most once.
pub(super) struct SessionPresentations {
    /// The resume selector.
    pub(super) sessions: SessionPresentation,
    /// The missing-directory confirmation.
    pub(super) missing_cwd: MissingSessionCwdPresentation,
}

/// Open the selected session, settling a stored directory that no longer exists.
///
/// An interactive run asks whether to continue in the original directory; any other run
/// reports the missing directory and ends with status 1.
///
/// # Errors
/// Returns listing, session file, presentation and output failures.
pub(super) async fn create_session_manager(
    parsed: &Args,
    dirs: &SessionDirs,
    app_mode: AppMode,
    mut effects: SessionEffects<'_>,
    presentations: SessionPresentations,
) -> Phase<SessionManager> {
    let manager = match selected(parsed, dirs, &mut effects, presentations.sessions).await? {
        ControlFlow::Continue(manager) => manager,
        ControlFlow::Break(status) => return Ok(ControlFlow::Break(status)),
    };
    let Some(issue) = get_missing_session_cwd_issue(&manager, &dirs.cwd) else {
        return Ok(ControlFlow::Continue(manager));
    };
    if !app_mode.is_interactive() {
        let message = MissingSessionCwdError::new(issue).to_string();
        line(&effects.stderr, &Theme::red(&message, effects.styles))?;
        return Ok(ControlFlow::Break(1));
    }
    let presentation = presentations.missing_cwd;
    let chosen = prompt_for_missing_session_cwd(&issue, effects.settings, presentation).await?;
    let Some(cwd) = chosen else {
        return Ok(ControlFlow::Break(0));
    };
    let file = issue.session_file.unwrap_or_default();
    let reopened = SessionManager::open(&file, dirs.session_dir.as_deref(), Some(&cwd))?;
    Ok(ControlFlow::Continue(reopened))
}

/// Perform the selected operation, reporting explicit open, fork and lookup failures.
async fn selected(
    parsed: &Args,
    dirs: &SessionDirs,
    effects: &mut SessionEffects<'_>,
    sessions: SessionPresentation,
) -> Phase<SessionManager> {
    let session_dir = dirs.session_dir.as_deref();
    match select_session_operation(parsed, &dirs.cwd, session_dir, &dirs.agent_dir).await? {
        SessionSelection::Selected(operation) => {
            let result = open_selected_session(&operation, &dirs.cwd, session_dir, &dirs.agent_dir);
            if matches!(
                operation,
                SessionOperation::Open { .. } | SessionOperation::Fork { .. }
            ) {
                session_or_report(result, effects)
            } else {
                Ok(ControlFlow::Continue(result?))
            }
        }
        SessionSelection::ConfirmFork { path, cwd } => {
            confirm_fork(&path, &cwd, dirs, effects).await
        }
        SessionSelection::Resume => resume(dirs, effects, sessions).await,
        SessionSelection::NotFound(argument) => {
            let message = format!("No session found matching '{argument}'");
            line(&effects.stderr, &Theme::red(&message, effects.styles))?;
            Ok(ControlFlow::Break(1))
        }
    }
}

/// Return a selected session, or report its failure in the run's style with status 1.
fn session_or_report(
    result: io::Result<SessionManager>,
    effects: &SessionEffects<'_>,
) -> Phase<SessionManager> {
    match result {
        Ok(manager) => Ok(ControlFlow::Continue(manager)),
        Err(error) => {
            line(
                &effects.stderr,
                &Theme::red(&format!("Error: {error}"), effects.styles),
            )?;
            Ok(ControlFlow::Break(1))
        }
    }
}

/// Ask before copying another project's session in; closed input decides nothing.
async fn confirm_fork(
    path: &str,
    cwd: &str,
    dirs: &SessionDirs,
    effects: &mut SessionEffects<'_>,
) -> Phase<SessionManager> {
    let notice = format!("Session found in different project: {cwd}");
    line(&effects.stdout, &Theme::yellow(&notice, effects.styles))?;
    let mut stdout = SharedWriter(Rc::clone(&effects.stdout));
    let question = "Fork this session into current directory?";
    match prompt_confirm(question, effects.stdin, &mut stdout).await? {
        Some(true) => session_or_report(
            fork_session(
                path,
                &dirs.cwd,
                dirs.session_dir.as_deref(),
                &dirs.agent_dir,
            ),
            effects,
        ),
        Some(false) => {
            line(&effects.stdout, &Theme::dim("Aborted.", effects.styles))?;
            Ok(ControlFlow::Break(0))
        }
        None => Ok(ControlFlow::Break(0)),
    }
}

/// Choose a session in the selector, watching its theme only while it is shown.
async fn resume(
    dirs: &SessionDirs,
    effects: &SessionEffects<'_>,
    sessions: SessionPresentation,
) -> Phase<SessionManager> {
    let theme = effects.settings.borrow().get_theme();
    let watcher = Rc::clone(&effects.watcher);
    effects.theme.init_theme(theme.as_deref(), Some(watcher))?;
    let current = sessions_loader(dirs, true);
    let selection = select_session(current, sessions_loader(dirs, false), sessions).await;
    effects.theme.stop_theme_watcher();
    let Some(path) = selection? else {
        line(
            &effects.stdout,
            &Theme::dim("No session selected", effects.styles),
        )?;
        return Ok(ControlFlow::Break(0));
    };
    let manager = SessionManager::open(&path, dirs.session_dir.as_deref(), None)?;
    Ok(ControlFlow::Continue(manager))
}

/// List the current project's sessions, or every project's, reporting progress.
fn sessions_loader(dirs: &SessionDirs, current: bool) -> SessionsLoader {
    let cwd = dirs.cwd.clone();
    let agent_dir = dirs.agent_dir.clone();
    let session_dir = dirs.session_dir.clone();
    Rc::new(move |progress: Option<SessionListProgress>| {
        let (cwd, agent_dir, session_dir) = (cwd.clone(), agent_dir.clone(), session_dir.clone());
        let mut notify = progress.map(|progress| {
            move |loaded, total| {
                progress(loaded, total);
                Ok(())
            }
        });
        Box::pin(async move {
            let notify = notify
                .as_mut()
                .map(|notify| notify as &mut dyn FnMut(usize, usize) -> io::Result<()>);
            if current {
                Ok(SessionManager::list(&cwd, session_dir.as_deref(), &agent_dir, notify).await?)
            } else {
                Ok(SessionManager::list_all(&agent_dir, notify).await)
            }
        })
    })
}
