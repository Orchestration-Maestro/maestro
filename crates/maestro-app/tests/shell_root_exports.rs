//! Native shell operations are available from the application root.
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{BashOperations, create_local_bash_operations, get_shell_config};
use std::sync::Arc;

#[test]
fn shell_operations_are_available_from_application_root() {
    let config = get_shell_config(Some(".")).unwrap();
    assert_eq!(config.shell, ".");
    assert_eq!(config.args, ["-c"]);
    let _: Arc<dyn BashOperations> =
        create_local_bash_operations(String::new(), Some(config.shell));
}
