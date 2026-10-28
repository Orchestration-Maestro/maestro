//! Opt-in issue completion backed by the GitHub CLI and the current editor provider.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{
    cell::RefCell,
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
};

use maestro_extensions_wasm::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, CompletionOptions,
    CompletionResult, CursorPosition, ExecOptions, ExtensionAPI, ExtensionContext, ExtensionFuture,
    ExtensionResult, NotificationType, fuzzy_filter, utils::is_whitespace_char,
};
use serde::Deserialize;

/// Maximum number of open issues requested from the CLI.
const MAX_ISSUES: &str = "100";
/// Maximum number of locally filtered candidates.
const MAX_SUGGESTIONS: usize = 20;
/// Channel used only to hand preload futures to the host's listener-tail owner.
const PRELOAD: &str = "github-issue-autocomplete:preload";

/// The fields requested from the GitHub CLI.
#[derive(Deserialize)]
struct GitHubIssue {
    /// Repository issue number.
    number: u64,
    /// Authored issue title.
    title: String,
    /// CLI state, displayed in lowercase.
    state: String,
}

/// A shared preload result, including an unavailable issue list or a host failure.
type Loaded = ExtensionResult<Option<Rc<Vec<GitHubIssue>>>>;

/// One session's preload, shared by every completion waiting for it.
#[derive(Default)]
struct Issues {
    /// Published once, even when loading fails.
    ready: Option<Loaded>,
    /// Completion requests waiting for publication.
    waiters: Vec<Waker>,
}

impl Issues {
    /// Waits for the single preload without holding a borrow while suspended.
    async fn get(shared: &RefCell<Self>) -> Loaded {
        poll_fn(|cx| {
            let mut state = shared.borrow_mut();
            if let Some(result) = &state.ready {
                return Poll::Ready(result.clone());
            }
            if !state
                .waiters
                .iter()
                .any(|waker| waker.will_wake(cx.waker()))
            {
                state.waiters.push(cx.waker().clone());
            }
            Poll::Pending
        })
        .await
    }

    /// Publishes before waking callers, outside the mutable borrow.
    fn publish(shared: &RefCell<Self>, result: Loaded) {
        let waiters = {
            let mut state = shared.borrow_mut();
            state.ready = Some(result);
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters {
            waiter.wake();
        }
    }
}

/// Trims using the host's shared whitespace set, including the byte order mark.
fn trim<'a>(api: &ExtensionAPI, text: &'a str) -> ExtensionResult<&'a str> {
    let mut start = 0;
    let mut end = text.len();
    for (index, ch) in text.char_indices() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        start = index + ch.len_utf8();
    }
    for (index, ch) in text[start..].char_indices().rev() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        end = start + index;
    }
    Ok(&text[start..end.max(start)])
}

/// Accepts only the source's SSH and HTTP GitHub remote spellings.
fn parse_repo(url: &str) -> Option<&str> {
    let repo = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let (owner, name) = repo.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(
        repo.strip_suffix(".git")
            .filter(|_| name != ".git")
            .unwrap_or(repo),
    )
}

/// Runs a literal command with the authored five-second deadline and session cwd.
async fn exec(
    api: &ExtensionAPI,
    cwd: &str,
    command: &str,
    args: &[&str],
) -> ExtensionResult<maestro_extensions_wasm::ExecResult> {
    api.exec(
        command,
        &args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>(),
        Some(ExecOptions {
            cwd: Some(cwd.into()),
            timeout: Some(5_000.0),
            signal: None,
        }),
    )
    .await
}

/// Splits remote columns using the host's shared whitespace set.
fn columns<'a>(api: &ExtensionAPI, line: &'a str) -> ExtensionResult<Vec<&'a str>> {
    let mut columns = Vec::new();
    let mut start = 0;
    for (index, ch) in line.char_indices() {
        if is_whitespace_char(api, &ch.to_string())? {
            if start < index {
                columns.push(&line[start..index]);
            }
            start = index + ch.len_utf8();
        }
    }
    if start < line.len() {
        columns.push(&line[start..]);
    }
    Ok(columns)
}

/// Resolves the first matching remote in the order Git prints them.
async fn resolve_repo(
    api: &ExtensionAPI,
    cwd: &str,
) -> ExtensionResult<Result<String, &'static str>> {
    let result = exec(api, cwd, "git", &["remote", "-v"]).await?;
    if result.code != 0 {
        return Ok(Err(
            "github-issue-autocomplete: cwd is not a git repository",
        ));
    }
    for line in result.stdout.split('\n') {
        let columns = columns(api, line)?;
        if let Some(repo) = columns.get(1).and_then(|url| parse_repo(url)) {
            return Ok(Ok(repo.into()));
        }
    }
    Ok(Err(
        "github-issue-autocomplete: cwd is not a GitHub repository",
    ))
}

/// Preloads once and reports a command or JSON failure once for this session.
async fn load(api: &ExtensionAPI, ctx: &ExtensionContext, repo: &str) -> Loaded {
    let cwd = ctx.cwd()?;
    let result = exec(
        api,
        &cwd,
        "gh",
        &[
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--limit",
            MAX_ISSUES,
            "--json",
            "number,title,state",
        ],
    )
    .await?;
    let error = if result.code != 0 {
        let details = trim(api, &result.stderr)?;
        let details = if details.is_empty() {
            format!("exit code {}", result.code)
        } else {
            details.into()
        };
        format!("github-issue-autocomplete: failed to load issues: {details}")
    } else {
        match serde_json::from_str::<Vec<GitHubIssue>>(&result.stdout) {
            Ok(issues) => return Ok(Some(Rc::new(issues))),
            Err(_) => "github-issue-autocomplete: failed to parse gh issue list output".into(),
        }
    };
    ctx.ui()?.notify(&error, Some(NotificationType::Error))?;
    Ok(None)
}

/// Offers a repository's cached issues while retaining all previous provider operations.
struct IssueProvider {
    /// Host facade used only for shared text and fuzzy helpers.
    api: ExtensionAPI,
    /// Provider that was current at installation.
    current: Rc<dyn AutocompleteProvider>,
    /// This session's independently owned preload.
    issues: Rc<RefCell<Issues>>,
}

/// Formats exactly the number label and lowercase state supplied by the CLI.
fn item(issue: &GitHubIssue) -> AutocompleteItem {
    AutocompleteItem {
        value: format!("#{}", issue.number),
        label: format!("#{}", issue.number),
        description: Some(format!("[{}] {}", issue.state.to_lowercase(), issue.title)),
    }
}

impl IssueProvider {
    /// Extracts only a hash token at the start or after a space or tab.
    fn token<'a>(&self, text: &'a str) -> ExtensionResult<Option<&'a str>> {
        let Some((before, query)) = text.rsplit_once('#') else {
            return Ok(None);
        };
        let boundary = before.is_empty() || before.ends_with([' ', '\t']);
        Ok((boundary && !is_whitespace_char(&self.api, query)?).then_some(query))
    }

    /// Prefers numeric prefixes, otherwise delegates ranking to the native fuzzy owner.
    fn filter(
        &self,
        issues: &[GitHubIssue],
        query: &str,
    ) -> ExtensionResult<Vec<AutocompleteItem>> {
        if trim(&self.api, query)?.is_empty() {
            return Ok(issues.iter().take(MAX_SUGGESTIONS).map(item).collect());
        }
        if query.bytes().all(|byte| byte.is_ascii_digit()) {
            let matches: Vec<_> = issues
                .iter()
                .filter(|issue| issue.number.to_string().starts_with(query))
                .take(MAX_SUGGESTIONS)
                .map(item)
                .collect();
            if !matches.is_empty() {
                return Ok(matches);
            }
        }
        Ok(fuzzy_filter(&self.api, issues, query, |issue| {
            format!("{} {}", issue.number, issue.title)
        })?
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(item)
        .collect())
    }
}

impl IssueProvider {
    /// Awaits this session's cache, then applies cancellation and local filtering.
    async fn suggestions(
        &self,
        token: &str,
        options: &CompletionOptions<'_>,
    ) -> ExtensionResult<Option<AutocompleteSuggestions>> {
        let issues = Issues::get(&self.issues).await?;
        if options.signal.aborted() {
            return Ok(None);
        }
        let Some(issues) = issues.filter(|issues| !issues.is_empty()) else {
            return Ok(None);
        };
        let items = self.filter(&issues, token)?;
        Ok((!items.is_empty()).then(|| AutocompleteSuggestions {
            items,
            prefix: format!("#{token}"),
        }))
    }
}

impl AutocompleteProvider for IssueProvider {
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a>,
    ) -> ExtensionFuture<'a, Option<AutocompleteSuggestions>> {
        Box::pin(async move {
            let text = lines.get(cursor.line as usize).map_or("", String::as_str);
            let text = text.get(..cursor.col as usize).unwrap_or(text);
            if let Some(token) = self.token(text)?
                && let Some(found) = self.suggestions(token, &options).await?
            {
                return Ok(Some(found));
            }
            self.current.get_suggestions(lines, cursor, options).await
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<CompletionResult> {
        self.current.apply_completion(lines, cursor, item, prefix)
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> ExtensionResult<Option<bool>> {
        Ok(Some(
            self.current
                .should_trigger_file_completion(lines, cursor)?
                .unwrap_or(true),
        ))
    }
}

/// Preload work armed only for the factory instance emitting its startup event.
type Pending = Rc<RefCell<Option<ExtensionFuture<'static, ()>>>>;

/// Resolves the repository, starts one independent preload and installs its wrapper.
async fn session_start(
    api: ExtensionAPI,
    ctx: ExtensionContext,
    pending: Pending,
) -> ExtensionResult<()> {
    let repo = match resolve_repo(&api, &ctx.cwd()?).await? {
        Ok(repo) => repo,
        Err(error) => return ctx.ui()?.notify(error, Some(NotificationType::Error)),
    };
    let issues = Rc::new(RefCell::new(Issues::default()));
    let (cache, loader, context) = (issues.clone(), api.clone(), ctx.clone());
    *pending.borrow_mut() = Some(Box::pin(async move {
        let result = load(&loader, &context, &repo).await;
        Issues::publish(&cache, result);
        Ok(())
    }));
    api.events().emit(PRELOAD, serde_json::Value::Null).await?;
    ctx.ui()?
        .add_autocomplete_provider(Rc::new(move |current, _| {
            Ok(Rc::new(IssueProvider {
                api: api.clone(),
                current,
                issues: issues.clone(),
            }))
        }))
}

/// Registers startup and hands each preload to the existing host-owned listener tails.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let pending = Pending::default();
        let armed = pending.clone();
        api.events()
            .on(PRELOAD, Rc::new(move |_, _| Ok(armed.borrow_mut().take())))?;
        let sender = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                let (api, pending) = (sender.clone(), pending.clone());
                Box::pin(async move {
                    session_start(api, ctx, pending).await?;
                    Ok(None)
                })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
