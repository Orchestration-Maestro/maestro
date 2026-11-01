//! Supplied-file notifications through the compiled guest and native watcher.
use super::{command_answer, loaded};
use crate::support::Root;
use serde_json::{Value, json};

/// Requires the retained guest watch operation to succeed.
fn call(rpc: &mut crate::support::rpc::Rpc, data: &Value) -> Value {
    let reply = command_answer(rpc, "native-watch", data);
    assert!(reply.get("error").is_none(), "{reply}");
    reply["value"].clone()
}

#[test]
fn native_watch_notifies_for_the_supplied_file() {
    let root = Root::new();
    let path = root.path("outside Ω.txt");
    std::fs::write(&path, "before").unwrap();
    let mut rpc = loaded(&root);
    call(&mut rpc, &json!({"op":"watch", "path":path}));
    std::fs::write(&path, "after Ω").unwrap();
    assert_eq!(call(&mut rpc, &json!({"op":"next"})), json!({"path":path}));
    call(&mut rpc, &json!({"op":"close"}));
    rpc.finish();
}

#[test]
fn native_watch_close_releases_its_registration() {
    let root = Root::new();
    let path = root.path("outside watch Ω.txt");
    std::fs::write(&path, "before").unwrap();
    let mut rpc = loaded(&root);
    for (index, action) in ["close", "drop", "self"].into_iter().enumerate() {
        call(
            &mut rpc,
            &json!({"op":"watch", "path":path, "self_close":action == "self"}),
        );
        if action == "self" {
            std::fs::write(&path, "changed").unwrap();
            assert_eq!(call(&mut rpc, &json!({"op":"next"})), json!({"path":path}));
        } else {
            call(&mut rpc, &json!({"op":action}));
            if action == "close" {
                call(&mut rpc, &json!({"op":"close"}));
            }
        }
        assert_eq!(
            call(&mut rpc, &json!({"op":"released", "count":index+1})),
            index + 1
        );
    }
    rpc.finish();
}

#[test]
fn native_watch_failures_do_not_leak_callbacks() {
    let root = Root::new();
    let path = root.path("outside live Ω.txt");
    let missing = root.path("missing Ω.txt");
    std::fs::write(&path, "before").unwrap();
    let mut rpc = loaded(&root);
    call(&mut rpc, &json!({"op":"watch", "path":path}));
    let failure = command_answer(
        &mut rpc,
        "native-watch",
        &json!({"op":"watch", "path":missing}),
    );
    assert!(!failure["error"].as_str().unwrap().is_empty(), "{failure}");
    assert_eq!(call(&mut rpc, &json!({"op":"released", "count":1})), 1);
    std::fs::write(&path, "after failed sibling").unwrap();
    assert_eq!(call(&mut rpc, &json!({"op":"next"})), json!({"path":path}));
    call(&mut rpc, &json!({"op":"close"}));
    assert_eq!(call(&mut rpc, &json!({"op":"released", "count":2})), 2);
    std::fs::write(&missing, "created").unwrap();
    call(&mut rpc, &json!({"op":"watch", "path":missing}));
    std::fs::write(&missing, "recovered").unwrap();
    assert_eq!(
        call(&mut rpc, &json!({"op":"next"})),
        json!({"path":missing})
    );
    call(&mut rpc, &json!({"op":"drop"}));
    assert_eq!(call(&mut rpc, &json!({"op":"released", "count":3})), 3);
    rpc.finish();
}

/// Checks retained identity, callback emission and release through generated imports.
pub(super) fn adapter_contract(root: &Root, rpc: &mut crate::support::rpc::Rpc) {
    let path = root.path("adapter Ω.txt");
    std::fs::write(&path, "before").unwrap();
    call(rpc, &json!({"op":"watch", "path":path}));
    std::fs::write(&path, "after").unwrap();
    assert_eq!(call(rpc, &json!({"op":"next"})), json!({"path":path}));
    call(rpc, &json!({"op":"drop"}));
    assert_eq!(call(rpc, &json!({"op":"released", "count":1})), 1);
}
