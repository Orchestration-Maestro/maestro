//! Native link-chain traversal over supplied observations, without filesystem effects.
use maestro_path::resolve_final_links;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

/// Relative targets retain kernel-significant components; absolute targets replace their parent.
#[test]
fn final_link_targets_keep_their_native_spelling() {
    let links = BTreeMap::from([
        (PathBuf::from("alias"), PathBuf::from("links/next")),
        (PathBuf::from("links/next"), PathBuf::from("../target/.")),
    ]);
    let resolved =
        resolve_final_links(Path::new("alias"), |path| Ok(links.get(path).cloned())).unwrap();
    assert_eq!(
        resolved.as_os_str(),
        Path::new("links").join("../target/.").as_os_str()
    );
    let absolute = PathBuf::from(if cfg!(windows) {
        r"C:\target\."
    } else {
        "/target/."
    });
    let resolved = resolve_final_links(Path::new("absolute"), |path| {
        Ok((path == Path::new("absolute")).then(|| absolute.clone()))
    })
    .unwrap();
    assert_eq!(resolved.as_os_str(), absolute.as_os_str());
}

/// Forty links are permitted, a further link fails, and observation errors remain unchanged.
#[test]
fn final_link_traversal_enforces_its_bound_and_preserves_errors() {
    let looping = resolve_final_links(Path::new("loop"), |_| Ok(Some(PathBuf::from("loop"))));
    assert!(looping.unwrap_err().to_string().contains("40 links"));
    let mut observations = 0;
    let resolved = resolve_final_links(Path::new("start"), |_| {
        observations += 1;
        Ok((observations <= 40).then(|| PathBuf::from("last")))
    })
    .unwrap();
    assert_eq!(resolved, Path::new("last"));
    assert_eq!(observations, 41);
    let denied = resolve_final_links(Path::new("denied"), |_| {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
    });
    assert_eq!(denied.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
}
