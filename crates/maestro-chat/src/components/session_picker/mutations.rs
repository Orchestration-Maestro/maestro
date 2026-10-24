//! Mutation effects and retained discovery reconciliation.
use super::{
    Duration, Error, LoadReason, LocalFuture, Rc, SessionFileOperations, SessionList, State,
    StatusMessage,
};
use crate::components::session_picker::delete_session_file::{DeleteMethod, delete_session_file};
impl State {
    /// Shares confirmation, error and deletion policy through replaceable list slots.
    pub(super) fn wire_mutations(self: &Rc<Self>, list: &SessionList) {
        let state = self.clone();
        list.set_on_delete_confirmation_change(Some(Rc::new(move |path| {
            state
                .header
                .set_confirming_delete_path(path.map(str::to_owned));
            (state.render)();
        })));
        let state = self.clone();
        list.set_on_error(Some(Rc::new(move |message| {
            state.header.set_status_message(
                Some(StatusMessage::Error(message.to_owned())),
                Some(Duration::from_millis(3000)),
            );
            (state.render)();
        })));
        let Some(operations) = &self.file_operations else {
            return;
        };
        let operations = operations.clone();
        let state = self.clone();
        list.set_on_delete_session(Some(Rc::new(move |path| {
            state.delete(path.to_owned(), operations.clone())
        })));
    }
    /// Owns the state and publication destination until deletion settles.
    fn delete(
        self: &Rc<Self>,
        path: String,
        operations: Rc<dyn SessionFileOperations>,
    ) -> LocalFuture {
        let state = self.clone();
        let list = self.list.upgrade();
        Box::pin(async move {
            let result = delete_session_file(path.clone(), operations).await;
            if let Some(list) = list {
                state.finish_delete(&path, &list, result).await?;
            }
            Ok(())
        })
    }
    /// Filters retained record handles in both successful snapshots.
    fn remove_path(&self, path: &str) {
        for discovery in &self.discoveries {
            let snapshot = discovery.records.borrow().clone();
            if let Some(snapshot) = snapshot {
                let records = snapshot
                    .iter()
                    .filter(|record| record.path != path)
                    .cloned()
                    .collect();
                discovery.records.replace(Some(Rc::new(records)));
            }
        }
    }
    /// Filters immutable handles before awaiting the selected scope's shared loader.
    async fn finish_delete(
        self: &Rc<Self>,
        path: &str,
        list: &SessionList,
        result: Result<
            DeleteMethod,
            crate::components::session_picker::delete_session_file::DeleteFailure,
        >,
    ) -> Result<(), Box<dyn Error>> {
        match result {
            Ok(method) => {
                self.remove_path(path);
                self.publish(list);
                let message = match method {
                    DeleteMethod::Trash => "Session moved to trash",
                    DeleteMethod::Unlink => "Session deleted",
                };
                self.header.set_status_message(
                    Some(StatusMessage::Info(message.into())),
                    Some(Duration::from_millis(2000)),
                );
                self.load(self.scope.get(), LoadReason::Refresh).await?;
            }
            Err(error) => self.header.set_status_message(
                Some(StatusMessage::Error(format!("Failed to delete: {error}"))),
                Some(Duration::from_millis(3000)),
            ),
        }
        (self.render)();
        Ok(())
    }
}
