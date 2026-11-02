//! The capabilities lent to component factories, declared once on the shared import interface.

/// Declares the terminal, row redraw and footer operations without a second adapter trait.
macro_rules! factory_imports {
    () => {
        /// Terminal handle lent to a component factory.
        type Tui: 'static;
        /// Reads the original frame writer's full redraw count.
        fn tui_full_redraws(&self, tui: &Self::Tui) -> ExtensionResult<u64>;
        /// Redraw capability of one tool row.
        type Invalidator: 'static;
        /// Redraws the tool row behind `invalidator`.
        fn invalidate(&self, invalidator: &Self::Invalidator) -> ExtensionResult<()>;
        /// Footer metadata lent to a footer factory.
        type FooterData: 'static;
        /// Reads the current branch.
        fn footer_get_git_branch(
            &self,
            footer: &Self::FooterData,
        ) -> ExtensionResult<Option<String>>;
        /// Reads the extension statuses in insertion order.
        fn footer_get_extension_statuses(
            &self,
            footer: &Self::FooterData,
        ) -> ExtensionResult<Vec<(String, String)>>;
        /// Reads the count of providers with available models.
        fn footer_get_available_provider_count(
            &self,
            footer: &Self::FooterData,
        ) -> ExtensionResult<f64>;
        /// Adds the branch change listener under `listener`, returning its host remover.
        fn footer_on_branch_change(
            &self,
            footer: &Self::FooterData,
            listener: &Self::Callback,
        ) -> ExtensionResult<Self::Subscription>;
    };
}
