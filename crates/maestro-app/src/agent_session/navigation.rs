//! Session names, tree navigation and fork points.
use super::{
    AgentSession, AgentSessionEvent, ForkMessage, NavigateTreeResult,
    state::{agent_messages, content_text, thrown},
};
use crate::configuration::config::get_docs_path;
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_catalog::ResolvedRequestAuth;
use maestro_credentials::format_no_api_key_found_message;
use maestro_extensions::{
    ExtensionExecution, SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionTreeEvent,
    TreePreparation,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo, Model};
use maestro_session::{
    BranchSummaryResult, GenerateBranchSummaryOptions, Presence,
    collect_entries_for_branch_summary, generate_branch_summary,
    session_manager::{
        BranchSummaryEntry, BranchSummaryOptions, NavigateTreeOptions, ReadonlySessionManager,
    },
};
use serde::Deserialize as _;
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

/// Summary text and details chosen for the branch left behind.
struct BranchSummary {
    /// The summary text.
    text: String,
    /// Its stored details.
    details: Presence<Value>,
    /// Whether an extension supplied it.
    from_extension: bool,
}

/// The summarizer instructions and the label, after extension overrides.
struct NavigationChoices {
    /// Instructions for the summarizer.
    custom_instructions: Option<String>,
    /// Whether they replace the default prompt.
    replace_instructions: bool,
    /// Label for the summary or the target.
    label: Option<String>,
}

impl NavigationChoices {
    /// Applies a handler's overrides: a missing field keeps the choice, null clears it.
    fn apply(&mut self, result: SessionBeforeTreeResult) {
        replace(&mut self.custom_instructions, result.custom_instructions);
        match result.replace_instructions {
            Presence::Missing => {}
            Presence::Null => self.replace_instructions = false,
            Presence::Present(replace) => self.replace_instructions = replace,
        }
        replace(&mut self.label, result.label);
    }
}

/// Replaces `slot` with a supplied override; null clears it.
fn replace<T>(slot: &mut Option<T>, value: Presence<T>) {
    match value {
        Presence::Missing => {}
        Presence::Null => *slot = None,
        Presence::Present(value) => *slot = Some(value),
    }
}

/// An optional choice as an event field: absent stays omitted.
pub(super) fn presence<T>(value: Option<T>) -> Presence<T> {
    value.map_or(Presence::Missing, Presence::Present)
}

/// What the before-tree handlers decided.
enum BeforeTree {
    /// A handler cancelled the move.
    Cancelled,
    /// The move goes on, with the summary a handler supplied when one was used.
    Continue(Option<BranchSummary>),
}

/// The entry moved to, the leaf it selects and the leaf moved from.
struct TreeMove<'a> {
    /// The target entry identity.
    target_id: &'a str,
    /// The leaf the target selects.
    new_leaf_id: Option<String>,
    /// Text to put back in the editor.
    editor_text: Option<String>,
    /// The leaf before the move.
    old_leaf_id: Option<String>,
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Records a display name and announces the name now in effect.
    ///
    /// # Errors
    /// Returns a recording failure.
    pub fn set_session_name(&self, name: &str) -> Result<(), DiagnosticErrorInfo> {
        self.session_manager
            .borrow_mut()
            .append_session_info(name)
            .map_err(thrown)?;
        let name = self.session_name().map_err(thrown)?;
        self.emit(&AgentSessionEvent::SessionInfoChanged { name });
        Ok(())
    }

    /// Starts moving to `target_id` when called, optionally summarizing the branch left.
    /// The move belongs to the session: dropping even the never-polled result future leaves
    /// it running, and `abort_branch_summary` stops its summary.
    ///
    /// # Errors
    /// Returns `Entry <id> not found`, an authentication, summary, recording or extension
    /// failure, or the failure when the navigation's task ended before it did.
    pub fn navigate_tree(
        self: &Rc<Self>,
        target_id: &str,
        options: NavigateTreeOptions,
    ) -> impl std::future::Future<Output = Result<NavigateTreeResult, DiagnosticErrorInfo>> + use<E>
    {
        let session = Rc::clone(self);
        let target_id = target_id.to_owned();
        let result = self.start_operation(async move {
            let navigated = session.navigate(&target_id, options).await;
            session.state.borrow_mut().branch_summary_signal = None;
            navigated
        });
        async move { result.await.map_err(thrown)? }
    }

    /// The navigation the session runs, before its summary signal is cleared.
    async fn navigate(
        &self,
        target_id: &str,
        options: NavigateTreeOptions,
    ) -> Result<NavigateTreeResult, DiagnosticErrorInfo> {
        let Some((tree_move, mut event)) = self.prepare_navigation(target_id, &options)? else {
            return Ok(NavigateTreeResult::default());
        };
        let summarize = event.preparation.user_wants_summary;
        let mut choices = NavigationChoices {
            custom_instructions: options.custom_instructions,
            replace_instructions: options.replace_instructions.unwrap_or(false),
            label: options.label,
        };
        let extension_summary = match self
            .before_tree(&mut event, summarize, &mut choices)
            .await?
        {
            BeforeTree::Cancelled => {
                return Ok(NavigateTreeResult {
                    cancelled: true,
                    ..NavigateTreeResult::default()
                });
            }
            BeforeTree::Continue(summary) => summary,
        };
        let entries = event.preparation.entries_to_summarize;
        let summary = match extension_summary {
            Some(summary) => Some(summary),
            None if summarize && !entries.is_empty() => {
                let summary = self.summarize_branch(&entries, &choices, event.signal);
                let Some(summary) = summary.await? else {
                    return Ok(NavigateTreeResult {
                        cancelled: true,
                        aborted: true,
                        ..NavigateTreeResult::default()
                    });
                };
                Some(summary)
            }
            None => None,
        };
        self.finish_navigation(tree_move, summary, choices.label)
            .await
    }

    /// Starts a branch summary and records its signal for the caller to abort.
    fn open_branch_summary(&self) -> Cancellation {
        let signal = Cancellation::new();
        self.state.borrow_mut().branch_summary_signal = Some(signal.clone());
        signal
    }

    /// The move to `target_id` and the before-tree event describing the branch left;
    /// `None` when the target is already the leaf.
    fn prepare_navigation<'a>(
        &self,
        target_id: &'a str,
        options: &NavigateTreeOptions,
    ) -> Result<Option<(TreeMove<'a>, SessionBeforeTreeEvent)>, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let old_leaf_id = manager.get_leaf_id().map(str::to_owned);
        if old_leaf_id.as_deref() == Some(target_id) {
            return Ok(None);
        }
        let target = manager
            .get_entry(target_id)
            .ok_or_else(|| DiagnosticErrorInfo::error(format!("Entry {target_id} not found")))?;
        let (new_leaf_id, editor_text) = leaf_for(target, target_id);
        let collected =
            collect_entries_for_branch_summary(&*manager, old_leaf_id.as_deref(), target_id)
                .map_err(thrown)?;
        let event = SessionBeforeTreeEvent {
            preparation: TreePreparation {
                target_id: target_id.to_owned(),
                old_leaf_id: old_leaf_id.clone(),
                common_ancestor_id: collected.common_ancestor_id,
                entries_to_summarize: collected.entries.into_iter().cloned().collect(),
                user_wants_summary: options.summarize.unwrap_or(false),
                custom_instructions: presence(options.custom_instructions.clone()),
                replace_instructions: presence(options.replace_instructions),
                label: presence(options.label.clone()),
            },
            signal: self.open_branch_summary(),
        };
        let tree_move = TreeMove {
            target_id,
            new_leaf_id,
            editor_text,
            old_leaf_id,
        };
        Ok(Some((tree_move, event)))
    }

    /// Lets the before-tree handlers cancel the move, supply the summary or override the
    /// summarizer instructions and the label.
    async fn before_tree(
        &self,
        event: &mut SessionBeforeTreeEvent,
        summarize: bool,
        choices: &mut NavigationChoices,
    ) -> Result<BeforeTree, DiagnosticErrorInfo> {
        if !self.extension_runner().has_handlers("session_before_tree") {
            return Ok(BeforeTree::Continue(None));
        }
        let Some(mut result) = self.extension_runner().emit(event).await.map_err(thrown)? else {
            return Ok(BeforeTree::Continue(None));
        };
        if matches!(result.cancel, Presence::Present(true)) {
            return Ok(BeforeTree::Cancelled);
        }
        let summary = match std::mem::take(&mut result.summary) {
            Presence::Present(summary) if summarize => Some(BranchSummary {
                text: summary.summary,
                details: summary.details,
                from_extension: true,
            }),
            _ => None,
        };
        choices.apply(result);
        Ok(BeforeTree::Continue(summary))
    }

    /// Runs the default summarizer; `None` when it was aborted.
    async fn summarize_branch(
        &self,
        entries: &[Value],
        choices: &NavigationChoices,
        signal: Cancellation,
    ) -> Result<Option<BranchSummary>, DiagnosticErrorInfo> {
        let model = self.model();
        let (api_key, headers) = self.get_required_request_auth(&model).await?;
        let reserve_tokens = self
            .settings_manager
            .borrow()
            .get_branch_summary_settings()
            .reserve_tokens;
        let options = GenerateBranchSummaryOptions {
            model: read(&model).clone(),
            api_key,
            headers,
            signal,
            custom_instructions: choices.custom_instructions.clone(),
            replace_instructions: choices.replace_instructions,
            reserve_tokens: Some(reserve_tokens),
        };
        let (text, read_files, modified_files) =
            match generate_branch_summary(entries, &options).await? {
                BranchSummaryResult::Aborted { .. } => return Ok(None),
                BranchSummaryResult::Error { error } => {
                    return Err(DiagnosticErrorInfo::error(error));
                }
                BranchSummaryResult::Summary {
                    summary,
                    read_files,
                    modified_files,
                } => (summary, read_files, modified_files),
                BranchSummaryResult::NoContent { summary } => (summary, Vec::new(), Vec::new()),
            };
        Ok(Some(BranchSummary {
            text,
            details: Presence::Present(
                json!({"readFiles": read_files, "modifiedFiles": modified_files}),
            ),
            from_extension: false,
        }))
    }

    /// Moves the leaf, stores the summary and the label, rebuilds the agent history and
    /// announces the move.
    async fn finish_navigation(
        &self,
        tree_move: TreeMove<'_>,
        summary: Option<BranchSummary>,
        label: Option<String>,
    ) -> Result<NavigateTreeResult, DiagnosticErrorInfo> {
        let summary = summary.filter(|summary| !summary.text.is_empty());
        let from_extension = summary.as_ref().map(|summary| summary.from_extension);
        let label = label.filter(|label| !label.is_empty());
        let summary_entry = self.store_move(&tree_move, summary, label.as_deref())?;
        let context = self
            .session_manager
            .borrow()
            .build_session_context()
            .map_err(thrown)?;
        write(self.agent.state()).set_messages(agent_messages(context.messages)?);
        let new_leaf_id = self
            .session_manager
            .borrow()
            .get_leaf_id()
            .map(str::to_owned);
        let mut event = SessionTreeEvent {
            new_leaf_id,
            old_leaf_id: tree_move.old_leaf_id,
            summary_entry: presence(summary_entry.clone()),
            from_extension: presence(from_extension),
        };
        self.extension_runner()
            .emit(&mut event)
            .await
            .map_err(thrown)?;
        Ok(NavigateTreeResult {
            editor_text: tree_move.editor_text,
            cancelled: false,
            aborted: false,
            summary_entry,
        })
    }

    /// Branches to the target's leaf, through a summary entry when there is a summary, and
    /// puts the label on the summary or else on the target; returns the stored summary entry.
    fn store_move(
        &self,
        tree_move: &TreeMove<'_>,
        summary: Option<BranchSummary>,
        label: Option<&str>,
    ) -> Result<Option<BranchSummaryEntry>, DiagnosticErrorInfo> {
        let mut manager = self.session_manager.borrow_mut();
        let new_leaf_id = tree_move.new_leaf_id.as_deref();
        let (summary_entry, labelled) = if let Some(summary) = summary {
            let id = manager
                .branch_with_summary(
                    new_leaf_id,
                    &summary.text,
                    BranchSummaryOptions {
                        details: summary.details,
                        from_hook: Presence::Present(summary.from_extension),
                        usage: Presence::Missing,
                    },
                )
                .map_err(thrown)?;
            let entry = manager.get_entry(&id).unwrap_or(&Value::Null);
            let entry = BranchSummaryEntry::deserialize(entry).map_err(thrown)?;
            (Some(entry), id)
        } else {
            match new_leaf_id {
                None => manager.reset_leaf(),
                Some(id) => manager.branch(id).map_err(thrown)?,
            }
            (None, tree_move.target_id.to_owned())
        };
        if let Some(label) = label {
            manager
                .append_label_change(&labelled, Presence::Present(label))
                .map_err(thrown)?;
        }
        Ok(summary_entry)
    }

    /// Every user message with text, in entry order.
    #[must_use]
    pub fn get_user_messages_for_forking(&self) -> Vec<ForkMessage> {
        self.session_manager
            .borrow()
            .get_entries()
            .into_iter()
            .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("message"))
            .filter(|entry| entry["message"].get("role").and_then(Value::as_str) == Some("user"))
            .filter_map(|entry| {
                let text = content_text(entry["message"].get("content"));
                let entry_id = entry.get("id").and_then(Value::as_str)?.to_owned();
                (!text.is_empty()).then_some(ForkMessage { entry_id, text })
            })
            .collect()
    }

    /// The key and headers a summary request for `model` uses.
    pub(super) async fn get_required_request_auth(
        &self,
        model: &Arc<RwLock<Model>>,
    ) -> Result<(String, Option<IndexMap<String, String>>), DiagnosticErrorInfo> {
        let provider = read(model).provider.clone();
        let no_key = || match get_docs_path() {
            Ok(docs) => {
                DiagnosticErrorInfo::error(format_no_api_key_found_message(&provider, &docs))
            }
            Err(error) => thrown(error),
        };
        let resolved = self.model_registry.get_api_key_and_headers(model).await;
        match resolved {
            ResolvedRequestAuth::Error { error } if error.starts_with("No API key found") => {
                Err(no_key())
            }
            ResolvedRequestAuth::Error { error } => Err(DiagnosticErrorInfo::error(error)),
            ResolvedRequestAuth::Ok {
                api_key: Some(api_key),
                headers,
            } if !api_key.is_empty() => Ok((api_key, headers)),
            ResolvedRequestAuth::Ok { .. } if self.model_registry.is_using_oauth(model) => {
                Err(DiagnosticErrorInfo::error(format!(
                    "Authentication failed for \"{provider}\". Credentials may have expired or network is unavailable. Run '/login {provider}' to re-authenticate."
                )))
            }
            ResolvedRequestAuth::Ok { .. } => Err(no_key()),
        }
    }
}

/// The new leaf and editor text for a target: a user or custom message moves to its parent
/// and returns its text; any other entry becomes the leaf.
fn leaf_for(target: &Value, target_id: &str) -> (Option<String>, Option<String>) {
    let parent = || {
        target
            .get("parentId")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    match target.get("type").and_then(Value::as_str) {
        Some("message")
            if target["message"].get("role").and_then(Value::as_str) == Some("user") =>
        {
            let text = content_text(target["message"].get("content"));
            (parent(), Some(text))
        }
        Some("custom_message") => (parent(), Some(content_text(target.get("content")))),
        _ => (Some(target_id.to_owned()), None),
    }
}
