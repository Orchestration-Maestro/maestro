//! Labels from supplied native paths.
#![cfg(test)]

#[test]
fn labels_use_supplied_home_checkout_and_path_fallback() {
    use maestro_workspace::workspace::git::{automatic_workspace_label, fallback_label_from_cwd};
    use std::path::Path;
    let home = Path::new("/home/member");
    assert_eq!(fallback_label_from_cwd(home, Some(home)), "~");
    assert_eq!(
        fallback_label_from_cwd(home, Some(Path::new("/elsewhere"))),
        "member"
    );
    assert_eq!(fallback_label_from_cwd(home, None), "member");
    assert_eq!(fallback_label_from_cwd(Path::new(""), None), "");
    assert_eq!(fallback_label_from_cwd(Path::new("/"), None), "/");
    assert_eq!(
        fallback_label_from_cwd(Path::new("/work/日本語"), None),
        "日本語"
    );
    assert_eq!(
        automatic_workspace_label(home, Path::new("/work/checkout"), Some(home)),
        "checkout"
    );
    assert_eq!(
        automatic_workspace_label(home, Path::new("/"), Some(home)),
        "~"
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let native = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 255]));
        assert_eq!(fallback_label_from_cwd(&native, None), "/�");
    }
}
