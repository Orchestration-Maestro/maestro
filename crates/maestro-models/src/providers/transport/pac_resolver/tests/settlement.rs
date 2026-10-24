//! Authored asynchronous return values use the same retained worker.

#[test]
fn pac_async_results_await_authored_helpers_and_thenables() {
    crate::tests::runtime::run(async {
        for (source, expected) in [
            (
                "async function helper(host){return await dnsResolve(host)}async function FindProxyForURL(){return await helper('localhost')}",
                "127.0.0.1",
            ),
            (
                "function FindProxyForURL(){return {then(resolve){resolve('THENABLE')}}}",
                "THENABLE",
            ),
            ("async function FindProxyForURL(){return false}", "DIRECT"),
        ] {
            let resolver = super::super::Resolver::new(source.into()).await.unwrap();
            assert_eq!(
                resolver.resolve("https://example.com").await.unwrap(),
                expected
            );
            resolver.close().await.unwrap();
        }
        let resolver = super::super::Resolver::new(
            "async function FindProxyForURL(){throw new Error('authored rejection')}".into(),
        )
        .await
        .unwrap();
        assert!(
            resolver
                .resolve("https://example.com")
                .await
                .unwrap_err()
                .to_string()
                .contains("authored rejection")
        );
        resolver.close().await.unwrap();
    });
}

#[test]
fn pac_pending_results_settle_when_another_invocation_arrives() {
    crate::tests::runtime::run(async {
        let resolver = super::super::Resolver::new("let finish;function FindProxyForURL(url){if(url.includes('/wait'))return new Promise(resolve=>finish=resolve);finish('FIRST');return 'SECOND'}".into()).await.unwrap();
        {
            let first = resolver.resolve("https://example.com/wait");
            tokio::pin!(first);
            let waker = std::task::Waker::noop();
            assert!(
                std::future::Future::poll(
                    first.as_mut(),
                    &mut std::task::Context::from_waker(waker)
                )
                .is_pending()
            );
            assert_eq!(
                resolver
                    .resolve("https://example.com/release")
                    .await
                    .unwrap(),
                "SECOND"
            );
            assert_eq!(first.await.unwrap(), "FIRST");
        }
        resolver.close().await.unwrap();
    });
}

#[test]
fn pac_abandoned_pending_results_release_without_settlement() {
    crate::tests::runtime::run(async {
        let resolver = super::super::Resolver::new("function FindProxyForURL(url){return url.includes('/wait')?new Promise(()=>{}):'DIRECT'}".into()).await.unwrap();
        let commands = resolver.commands.as_ref().unwrap();
        let invocation = super::super::Invocation {
            commands: commands.clone(),
            token: std::sync::Arc::new(()),
        };
        let weak = std::sync::Arc::downgrade(&invocation.token);
        let (reply, result) = tokio::sync::oneshot::channel();
        commands
            .send(super::super::Command::Resolve(super::super::Request {
                token: invocation.token.clone(),
                url: "https://example.com/wait".into(),
                reply,
            }))
            .unwrap();
        // A later completed invocation proves the earlier pending entry was admitted.
        assert_eq!(
            resolver
                .resolve("https://example.com/barrier")
                .await
                .unwrap(),
            "DIRECT"
        );
        assert!(weak.upgrade().is_some());
        drop(result);
        drop(invocation);
        assert_eq!(
            resolver
                .resolve("https://example.com/barrier")
                .await
                .unwrap(),
            "DIRECT"
        );
        assert!(weak.upgrade().is_none());
        resolver.close().await.unwrap();
    });
}

#[test]
fn resolver_keeps_bracketed_ipv6_and_hostless_errors_distinct() {
    crate::tests::runtime::run(async {
        let resolver = super::super::Resolver::new(
            "function FindProxyForURL(url,host){return JSON.stringify([url,host])}".into(),
        )
        .await
        .unwrap();
        assert_eq!(
            resolver
                .resolve("https://[::1]:8443/a%20b?q=1&q=2")
                .await
                .unwrap(),
            "[\"https://[::1]:8443/a%20b?q=1&q=2\",\"[::1]\"]"
        );
        assert!(
            resolver
                .resolve("data:,x")
                .await
                .unwrap_err()
                .to_string()
                .contains("Could not determine `host`")
        );
        assert!(
            resolver
                .resolve("")
                .await
                .unwrap_err()
                .to_string()
                .contains("relative URL")
        );
        resolver.close().await.unwrap();
    });
}
