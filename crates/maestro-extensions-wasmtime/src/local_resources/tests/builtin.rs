//! A native tool definition ends with its capability.
use super::Pump;
use crate::{
    builtin_reads::Request,
    guest_calls::tool_renders::{Info, Phase},
    local_resources::{Capability, builtin::Factory},
    native_renders::{Render, Slot},
    read_imports::ReadEndpoint,
    tests::ended_errors,
};
use maestro_extensions::LiveTheme;

#[test]
fn released_builtin_tool_rejects_every_kind_of_operation_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let tool = registry
        .create_builtin(Factory::Read("/".into(), None, None))
        .unwrap();
    let theme = registry
        .insert(Capability::Theme(LiveTheme::default()))
        .unwrap();
    let render = move || Render {
        row: None,
        theme: crate::theme_reads::Endpoint(theme.clone()),
        last: None,
        info: Info {
            tool_call_id: "call".into(),
            args: None,
            cwd: "/".into(),
            phase: Phase {
                execution_started: false,
                args_complete: false,
                is_partial: false,
            },
            expanded: false,
            show_images: false,
            is_error: false,
        },
        slot: Slot::Call(None),
    };
    let lease = tool.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            tool.read(Request::Metadata).unwrap_err(),
            tool.read(|reply| Request::Prepare("{}".into(), reply))
                .err()
                .unwrap(),
            tool.render(render()).result().unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension built-in tool resource ended"; 3]);
}
