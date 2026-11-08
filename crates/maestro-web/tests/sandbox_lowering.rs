//! Admission of supplied tool programs and provider expressions.
use maestro_web::components::sandboxed_iframe::lowering;

/// Real loading inputs remain accepted by the compiler before browser qualification.
#[test]
fn supplied_tool_inputs_are_accepted() {
    for source in [
        include_str!("sandbox_loading_support/program_inputs/user.js"),
        include_str!("sandbox_loading_support/program_inputs/marker_user.js"),
        "for(let i=0;i<1;i++){try{return 1}finally{continue}}",
        "function f(){return 1}var f;return f();",
        "return f();function f(){return 1}function f(){return 2}",
    ] {
        lowering::program(source).unwrap();
    }
    let supplied = [
        include_str!("sandbox_loading_support/program_inputs/provider.js"),
        include_str!("sandbox_loading_support/program_inputs/marker_provider.js"),
        "function(id){window.id=id}",
        "id=>{window.id=id}",
    ];
    for source in supplied {
        lowering::provider(source).unwrap();
    }
}

/// Body and expression grammar stay distinct, without admitting static module declarations.
#[test]
fn invalid_bodies_and_provider_expressions_are_refused() {
    for source in [
        "import x from './helper.js'; return x;",
        "export default 1;",
        "return new.target;",
        "}; window.sideEffect=true; async()=>{",
        "await ;",
        "return '",
    ] {
        assert!(lowering::program(source).is_err(), "{source}");
    }
    for source in [
        "function(",
        "function(){});window.sideEffect=true;(",
        "function(){});window.sideEffect=true;(function(){}",
        "function(){}) || (function(){}",
        "return 1",
    ] {
        assert!(lowering::provider(source).is_err(), "{source}");
    }
}
