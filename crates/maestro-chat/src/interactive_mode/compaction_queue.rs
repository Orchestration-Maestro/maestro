//! Delivery of the input captured while the session compacts: its capture, the prompt
//! that starts the batch, the tail behind it and the batch's recovery when a phase fails.

use super::InteractiveMode;
use super::queues::CompactionQueuedMessage;
use maestro_app::DiagnosticErrorInfo;
use maestro_app::agent_session::PromptOptions;
use maestro_app::messages::UserMessageDelivery;
use std::{
    cell::Cell,
    error::Error,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// The batch a compaction captured, shared by the dispatch that sends it and the first
/// prompt that may fail after that dispatch returned.
struct Delivery {
    /// The captured messages, restored as they were taken.
    batch: Vec<CompactionQueuedMessage>,
    /// Whether the batch was already recovered, so its failures restore it once.
    recovered: Cell<bool>,
    /// Whether the first prompt failed.
    first_failed: Cell<bool>,
}

impl Delivery {
    /// A delivery of `batch` that has neither recovered nor failed.
    fn new(batch: Vec<CompactionQueuedMessage>) -> Self {
        Self {
            batch,
            recovered: Cell::new(false),
            first_failed: Cell::new(false),
        }
    }

    /// Whether the first prompt already failed, which stops the tail behind it.
    fn first_failed(&self) -> bool {
        self.first_failed.get()
    }
}

impl InteractiveMode {
    /// Captures the input a compaction captured, shows the pending rows without it, and
    /// delivers it on the terminal host: with `will_retry` each message joins the queue its
    /// own mode names, and otherwise the first message that is not a registered command
    /// starts the next turn and the rest follows it. A phase that fails restores the batch
    /// once. Capturing before the host runs makes a second flush inert.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    pub(super) fn flush_compaction_queue(
        self: &Rc<Self>,
        will_retry: bool,
    ) -> Result<(), maestro_theme::ThemeError> {
        if self.compaction_queued_messages.borrow().is_empty() {
            return Ok(());
        }
        let delivery = Rc::new(Delivery::new(self.compaction_queued_messages.take()));
        self.update_pending_messages_display()?;
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                mode.deliver_captured_batch(delivery, will_retry).await
            }));
        Ok(())
    }

    /// Delivers the captured batch and refreshes the pending rows once it is sent, or
    /// restores it when a phase failed.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    async fn deliver_captured_batch(
        self: Rc<Self>,
        delivery: Rc<Delivery>,
        will_retry: bool,
    ) -> Result<(), Box<dyn Error>> {
        let delivered = if will_retry {
            self.deliver_each(&delivery.batch).await
        } else {
            self.deliver_with_first_prompt(&delivery).await
        };
        match delivered {
            Ok(()) => self.update_pending_messages_display()?,
            Err(error) => self.recover(&delivery, &error)?,
        }
        Ok(())
    }

    /// Sends the captured messages in order, stopping at the first failure.
    ///
    /// # Errors
    /// Returns the failure of the message that stopped the batch.
    async fn deliver_each(
        self: &Rc<Self>,
        messages: &[CompactionQueuedMessage],
    ) -> Result<(), DiagnosticErrorInfo> {
        for message in messages {
            self.deliver_one(message).await?;
        }
        Ok(())
    }

    /// Sends every registered command before the first prompt, starts that prompt, then
    /// sends the rest behind it and stops doing so once the prompt failed.
    ///
    /// # Errors
    /// Returns the failure of a command before the first prompt or of the tail.
    async fn deliver_with_first_prompt(
        self: &Rc<Self>,
        delivery: &Rc<Delivery>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let first_prompt = delivery
            .batch
            .iter()
            .position(|message| !self.is_extension_command(&message.text));
        let index = first_prompt.unwrap_or(delivery.batch.len());
        let (commands, rest) = delivery.batch.split_at(index);
        for message in commands {
            self.runtime_host
                .session()
                .prompt(&message.text, PromptOptions::default())
                .await?;
        }
        let Some((first, tail)) = rest.split_first() else {
            return Ok(());
        };
        if let Some(error) = self.start_first_prompt(delivery, first) {
            return Err(error);
        }
        for message in tail {
            if delivery.first_failed() {
                return Ok(());
            }
            self.deliver_one(message).await?;
        }
        Ok(())
    }

    /// Starts the batch's first prompt now. A prompt that already failed is returned, and
    /// one that is still running is observed on the terminal host, which restores the batch
    /// when it fails after this call returned.
    fn start_first_prompt(
        self: &Rc<Self>,
        delivery: &Rc<Delivery>,
        first: &CompactionQueuedMessage,
    ) -> Option<DiagnosticErrorInfo> {
        let mut prompt = Box::pin(
            self.runtime_host
                .session()
                .prompt(&first.text, PromptOptions::default()),
        );
        let polled = prompt
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        if let Poll::Ready(failed) = polled {
            return failed.err();
        }
        let delivery = Rc::clone(delivery);
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                let Err(error) = prompt.await else {
                    return Ok(());
                };
                delivery.first_failed.set(true);
                mode.recover(&delivery, &error)
            }));
        None
    }

    /// Sends one captured message the way its own kind asks: a registered command runs as a
    /// prompt, and other text joins the queue of its mode on the session in effect now.
    ///
    /// # Errors
    /// Returns the failure of the prompt, or the refusal of the queued text.
    async fn deliver_one(
        self: &Rc<Self>,
        message: &CompactionQueuedMessage,
    ) -> Result<(), DiagnosticErrorInfo> {
        let text = message.text.as_str();
        if self.is_extension_command(text) {
            return self
                .runtime_host
                .session()
                .prompt(text, PromptOptions::default())
                .await;
        }
        let session = self.runtime_host.session();
        match message.mode {
            UserMessageDelivery::FollowUp => session.follow_up(text, &[]),
            UserMessageDelivery::Steer => session.steer(text, &[]),
        }
    }

    /// Clears the queues of the session in effect now, restores the captured batch ahead of
    /// the input admitted since, refreshes the pending rows and shows one notice. A second
    /// recovery leaves everything as the first one left it.
    ///
    /// # Errors
    /// Returns the failure of the notice or of the published theme's dim colour.
    fn recover(
        &self,
        delivery: &Delivery,
        error: &DiagnosticErrorInfo,
    ) -> Result<(), Box<dyn Error>> {
        if delivery.recovered.replace(true) {
            return Ok(());
        }
        self.runtime_host.session().clear_queue();
        let mut restored = delivery.batch.clone();
        restored.append(&mut self.compaction_queued_messages.take());
        *self.compaction_queued_messages.borrow_mut() = restored;
        self.update_pending_messages_display()?;
        let plural = if delivery.batch.len() > 1 { "s" } else { "" };
        self.show_error(&format!(
            "Failed to send queued message{plural}: {}",
            error.message
        ))?;
        Ok(())
    }
}
