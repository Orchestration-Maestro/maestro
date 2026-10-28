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
