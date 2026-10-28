//! Native byte publication and permissions through a real authored extension.
use super::{command_answer, loaded};
use crate::support::{Root, rpc::Rpc};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

/// Invokes the author's filesystem facade through ordinary command dispatch.
fn call(rpc: &mut Rpc, data: &Value) -> Value {
    let reply = command_answer(rpc, "native-files", data);
    assert!(reply.get("error").is_none(), "{reply}");
    reply["value"].clone()
}

#[test]
fn native_files_save_private_bytes_and_preserve_existing_modes() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let path = root.path("private Ω");
    assert_eq!(
        call(
            &mut rpc,
            &json!({"op":"save", "path":path,
        "bytes":[0,255,10], "mode":0o600})
        ),
        Value::Null
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"\0\xff\n");
    assert_eq!(
        call(&mut rpc, &json!({"op":"mode", "path":path})),
        0o100_600
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    call(
        &mut rpc,
        &json!({"op":"save", "path":path, "bytes":[], "mode":0o600}),
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let dir = root.path("private-dir");
    call(&mut rpc, &json!({"op":"mkdir", "path":dir, "mode":0o700}));
    assert_eq!(call(&mut rpc, &json!({"op":"mode", "path":dir})), 0o40700);
    call(&mut rpc, &json!({"op":"chmod", "path":dir, "mode":0o750}));
    assert_eq!(
        std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
        0o750
    );
    rpc.finish();
}

#[test]
fn native_files_save_long_leaves_and_follow_final_links() {
    use std::os::unix::fs::symlink;
    let root = Root::new();
    let mut rpc = loaded(&root);
    let path = root.path(&"x".repeat(255));
    call(
        &mut rpc,
        &json!({"op":"save", "path":path, "bytes":[0,255], "mode":0o600}),
    );
    assert_eq!(std::fs::read(path).unwrap(), b"\0\xff");
    for existing in [false, true] {
        let target = root.path(if existing {
            "existing-target"
        } else {
            "dangling-target"
        });
        if existing {
            std::fs::write(&target, b"old").unwrap();
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let link = root.path(if existing {
            "existing-link"
        } else {
            "dangling-link"
        });
        symlink(target.file_name().unwrap(), &link).unwrap();
        call(
            &mut rpc,
            &json!({"op":"save", "path":link, "bytes":[255,0,13], "mode":0o600}),
        );
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"\xff\0\r");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            if existing { 0o640 } else { 0o600 }
        );
    }
    rpc.finish();
}

#[test]
fn native_files_precommit_failures_and_real_denial_preserve_content() {
    let root = Root::new();
    let mut rpc = loaded(&root);
    let parent = root.path("protected");
    std::fs::create_dir(&parent).unwrap();
    let file = parent.join("original");
    std::fs::write(&file, b"original\0\xff").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&file).unwrap().permissions().mode();
    let reply = command_answer(
        &mut rpc,
        "native-files",
        &json!({"op":"save", "path":file.join("child"), "bytes":[1], "mode":0o600}),
    );
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty()),
        "{reply}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"original\0\xff");
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode(),
        before
    );
    let Some(_restore) = maestro_test_directory::deny(&parent, 0o500).unwrap() else {
        eprintln!("privileged host: native permission denial cannot be exercised");
        rpc.finish();
        return;
    };
    let native = std::fs::write(parent.join("probe"), b"denied").unwrap_err();
    assert_eq!(native.kind(), std::io::ErrorKind::PermissionDenied);
    let reply = command_answer(
        &mut rpc,
        "native-files",
        &json!({"op":"save", "path":file, "bytes":[1], "mode":0o600}),
    );
    assert_eq!(reply, json!({"error":native.to_string()}));
    assert_eq!(std::fs::read(&file).unwrap(), b"original\0\xff");
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode(),
        before
    );
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 1);
    eprintln!("native permission denial observed; bytes and mode preserved");
    rpc.finish();
}
