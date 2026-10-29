//! Messages shown to the user once a load has finished.

/// The notice for a checkout that was restored to its pin, with the files it reset or removed.
#[must_use]
pub fn restored(source: &str, files: &[String]) -> String {
    let mut text = format!("Manifest release restored: {source}");
    for file in files {
        text.push_str("\n  ");
        text.push_str(file);
    }
    text
}

/// The warning for a checkout that drifted and could not be restored.
#[must_use]
pub fn drifted(source: &str, files: &[String], error: &str) -> String {
    let drift = if files.is_empty() {
        "the checkout is not at its pinned commit".to_owned()
    } else {
        format!("changed: {}", files.join(", "))
    };
    format!(
        "Manifest release {source} has drifted ({drift}) and could not be restored: {error}. \
         Running on the last accepted composition; the next start repairs it."
    )
}

/// The warning for a project selection that the user's manifest overrules.
#[must_use]
pub fn ignored(project: &str, user: &str) -> String {
    format!(
        "The project's manifest {project} is ignored: the user's manifest {user} locks \"manifest\"."
    )
}
