//! Source compilation resolves the callable once, including lexical bindings.

use super::{collect_body, request, routed};
use crate::tests::loopback;

#[test]
fn lexical_callable_routes_direct_requests() {
    crate::tests::runtime::run(async {
        let server = loopback::serve_routed(|_| loopback::Reply {
            status: 200,
            content_type: "text/plain",
            headers: &[],
            body: b"lexical".to_vec(),
            close: false,
        })
        .unwrap();
        let fetch = routed("const FindProxyForURL = () => 'DIRECT'");
        let response = fetch(request(server.url.clone())).await.unwrap();
        assert_eq!(collect_body(response.body).await.unwrap(), b"lexical");
        server.finish().unwrap();
    });
}

#[test]
fn compiled_callable_survives_global_reassignment() {
    crate::tests::runtime::run(async {
        let fetch =
            routed("function FindProxyForURL(){FindProxyForURL=()=> 'WRONG';return 'RETAINED'}");
        for _ in 0..2 {
            let error = fetch(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().contains("RETAINED"), "{error}");
        }
    });
}
