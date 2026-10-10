//! Exact authentication guidance formatter outputs.
#[cfg(test)]
mod tests {
    use maestro_credentials::*;
    /// Independent expected help for supplied authored roots.
    fn cases() -> Vec<(&'static str, &'static str)> {
        #[cfg(not(windows))]
        return vec![
            (
                "",
                "Use /login to log into a provider via OAuth or API key. See:\n  providers.md\n  models.md",
            ),
            (
                ".",
                "Use /login to log into a provider via OAuth or API key. See:\n  providers.md\n  models.md",
            ),
            (
                "docs/../guide",
                "Use /login to log into a provider via OAuth or API key. See:\n  guide/providers.md\n  guide/models.md",
            ),
            (
                "/root//docs/",
                "Use /login to log into a provider via OAuth or API key. See:\n  /root/docs/providers.md\n  /root/docs/models.md",
            ),
            (
                "C:\\docs\\..\\guide",
                "Use /login to log into a provider via OAuth or API key. See:\n  C:\\docs\\..\\guide/providers.md\n  C:\\docs\\..\\guide/models.md",
            ),
        ];
        #[cfg(windows)]
        return vec![
            (
                "",
                "Use /login to log into a provider via OAuth or API key. See:\n  providers.md\n  models.md",
            ),
            (
                ".",
                "Use /login to log into a provider via OAuth or API key. See:\n  providers.md\n  models.md",
            ),
            (
                "docs/../guide",
                "Use /login to log into a provider via OAuth or API key. See:\n  guide\\providers.md\n  guide\\models.md",
            ),
            (
                "/root//docs/",
                "Use /login to log into a provider via OAuth or API key. See:\n  \\root\\docs\\providers.md\n  \\root\\docs\\models.md",
            ),
            (
                "C:\\docs\\..\\guide",
                "Use /login to log into a provider via OAuth or API key. See:\n  C:\\guide\\providers.md\n  C:\\guide\\models.md",
            ),
        ];
    }
    #[test]
    fn login_help_uses_normalized_documentation_paths() {
        for (root, expected) in cases() {
            assert_eq!(get_provider_login_help(root), expected);
        }
    }
    #[test]
    fn missing_models_message_keeps_help_spacing() {
        for (root, help) in cases() {
            assert_eq!(
                format_no_models_available_message(root),
                format!("No models available. {help}")
            );
        }
    }
    #[test]
    fn missing_selection_message_keeps_blank_lines() {
        for (root, help) in cases() {
            assert_eq!(
                format_no_model_selected_message(root),
                format!("No model selected.\n\n{help}\n\nThen use /model to select a model.")
            );
        }
    }
    #[test]
    fn missing_key_message_substitutes_only_unknown() {
        for (root, help) in cases() {
            for (provider, display) in [
                ("unknown", "the selected model"),
                ("Unknown", "Unknown"),
                ("", ""),
                ("provider\n二", "provider\n二"),
            ] {
                assert_eq!(
                    format_no_api_key_found_message(provider, root),
                    format!("No API key found for {display}.\n\n{help}")
                );
            }
        }
    }
}
