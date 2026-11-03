//! Real-browser regressions selected only by the pinned Chromium profile.

use serde_json::json;
use std::io;

mod support;
use support::{MOUNT, POLICY, fixture, run, state};

/// The designated export permits exactly one matching srcdoc policy exception.
const POLICY_EXPORT: &str = "inline_programs_preserve_text_policies_and_order";
/// Mount identifiers remain literal, including punctuation and Unicode.
const ID: &str = "probe:\"雪]";

/// Module imports supplied by the page retain actual linked-module identity.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn loading_exports_receive_the_page_module_import() {
    run(fixture(&[], async |browser, origin| {
        let value = browser.check(&format!("{origin}/"), "loading", "(async () => {const importModule=url=>import(url);const a=await importModule('/helper.js');const b=await importModule('/helper.js');return {answer:a.answer,same:a===b};})()").await?;
        assert_eq!(value, json!({"answer":"helper-ok","same":true}));
        Ok(())
    })).unwrap();
}

/// Successive exports cannot share page globals or module state.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn exports_run_serially_in_fresh_pages() {
    run(fixture(&[], async |browser, origin| {
        let url = format!("{origin}/");
        assert_eq!(browser.check(&url,"first","localStorage.setItem('driver-sentinel','first');document.cookie='driver-sentinel=first';window.initialized=true;Promise.resolve('first')").await?, json!("first"));
        assert_eq!(browser.check(&url,"second","({global:typeof window.initialized,storage:localStorage.getItem('driver-sentinel'),cookie:document.cookie})").await?, json!({"global":"undefined","storage":null,"cookie":""}));
        let phases = browser.lines().iter().filter(|line| line.ends_with("export resolved")).collect::<Vec<_>>();
        assert_eq!(phases, ["PHASE first export resolved", "PHASE second export resolved"]);
        Ok(())
    })).unwrap();
}

/// Both direct throws and rejected promises keep the original exception text.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn assertion_failures_keep_sync_and_async_errors() {
    for expression in [
        "(()=>{throw Error('sync sentinel')})()",
        "Promise.reject(Error('async sentinel'))",
    ] {
        let error = run(fixture(&[], async |browser, origin| {
            browser
                .check(&format!("{origin}/"), "assertion", expression)
                .await
                .map(|_| ())
        }))
        .unwrap_err();
        assert!(error.to_string().contains("sentinel"), "{error}");
    }
}

/// Errors from an independent target never become this page's assertion failure.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn page_errors_stay_with_their_own_session() {
    run(fixture(&[],async |browser,origin| {
        let mut page=browser.page("owned-session").await?;
        page.initialize().await?;page.navigate(&format!("{origin}/")).await?;
        let (context,session)=support::foreign_page(&mut page).await?;
        let route=Some(session.as_ref());
        page.send(route,"Runtime.enable",json!({})).await?;
        page.send(route,"Runtime.evaluate",json!({"expression":"new Promise(resolve=>document.readyState==='complete'?resolve():addEventListener('load',resolve,{once:true}))","awaitPromise":true})).await?;
        page.send(route,"Runtime.evaluate",json!({"expression":r"window.addEventListener('error',event=>window.otherError=event.message);const script=document.createElement('script');script.textContent=`throw Error('foreign sentinel')`;document.body.append(script)"})).await?;
        let observed=page.send(route,"Runtime.evaluate",json!({"expression":"window.otherError","returnByValue":true})).await?;
        assert!(observed["result"]["value"].as_str().unwrap().contains("foreign sentinel"));
        assert_eq!(page.evaluate("location.href").await?,json!(format!("{origin}/")));
        page.send(None,"Target.disposeBrowserContext",json!({"browserContextId":context})).await?;
        page.close().await
    })).unwrap();
}

/// Unexpected errors from embedded documents fail the containing assertion.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn unexpected_page_errors_fail() {
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(&format!("{origin}/"), "inventory", &format!("({POLICY})()"))
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(error.to_string().contains("window.complete"), "{error}");
}

/// The exact designated source and exception count pass.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn expected_policy_error_matches_once() {
    run(fixture(&[], async |browser, origin| {
        browser
            .check(
                &format!("{origin}/"),
                POLICY_EXPORT,
                &format!("({POLICY})()"),
            )
            .await?;
        assert!(browser.lines().iter().any(|line| line.ends_with("1/1")));
        Ok(())
    }))
    .unwrap();
}

/// A missing policy exception is a failed assertion, not a silent success.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn expected_policy_error_is_required() {
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(&format!("{origin}/"), POLICY_EXPORT, "Promise.resolve()")
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error.to_string().contains("expected policy error count"),
        "{error}"
    );
}

/// A second permitted-looking policy exception still fails the count.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn extra_policy_errors_fail() {
    let expression = format!("(async () => {{ await ({POLICY})(); await ({POLICY})(); }})()");
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(&format!("{origin}/"), POLICY_EXPORT, &expression)
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error.to_string().contains("expected policy error count"),
        "{error}"
    );
}

/// A policy frame may not disguise a different JavaScript failure.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn different_policy_errors_fail() {
    let expression = format!(
        "({POLICY})({})",
        json!("throw TypeError('different sentinel')")
    );
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(&format!("{origin}/"), POLICY_EXPORT, &expression)
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(error.to_string().contains("different sentinel"), "{error}");
}

/// Identical exception text is not admitted from the top page.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn policy_error_from_another_source_fails() {
    let error = run(fixture(&[], async |browser, origin| {
        browser.check(&format!("{origin}/"),POLICY_EXPORT,"(()=>{const script=document.createElement('script');script.textContent='window.complete()';document.body.append(script);})()").await.map(|_| ())
    })).unwrap_err();
    assert!(error.to_string().contains("window.complete"), "{error}");
}

/// Native renderer crashes settle the outstanding operation with an error.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn page_crashes_fail() {
    let error = run(fixture(&[], async |browser, _origin| {
        let mut page = browser.page("crash").await?;
        page.initialize().await?;
        page.execute("Page.crash", json!({})).await.map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error.to_string().to_lowercase().contains("crash"),
        "{error}"
    );
}

/// Browser disconnection fails the next operation instead of waiting for a deadline.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn browser_disconnects_fail() {
    let error = run(fixture(&[], async |browser, origin| {
        browser.execute("Browser.close", json!({})).await?;
        browser
            .check(&format!("{origin}/"), "disconnected", "new Promise(()=>{})")
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error.to_string().to_lowercase().contains("connection")
            || error.to_string().to_lowercase().contains("closed"),
        "{error}"
    );
}

/// Console runtime panics are assertion failures even if the export resolves.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn console_panics_fail() {
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(
                &format!("{origin}/"),
                "panic",
                "console.error('panicked at sentinel')",
            )
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error.to_string().contains("panicked at sentinel"),
        "{error}"
    );
}

/// Missing and empty mounts produce useful literal-identifier errors.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_inspection_reports_missing_mount() {
    for id in ["missing", "empty", ID] {
        let expression = format!(
            "document.body.innerHTML='<div id=empty></div><iframe sandbox=allow-scripts srcdoc=opaque></iframe>';inspectSandbox({},'state')",
            json!(id)
        );
        let error = run(fixture(&[], async |browser, origin| {
            browser
                .check(&format!("{origin}/"), "missing", &expression)
                .await
                .map(|_| ())
        }))
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("mounted frame not found {id}")),
            "{error}"
        );
    }
}

/// Native lookup ignores detached unrelated frames and decoys across nested documents.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_inspection_ignores_detached_unrelated_frames() {
    for nested in [false, true] {
        let expression = format!(
            "(async()=>{{await ({MOUNT})({nested});document.getElementById('obsolete').remove();const value=await inspectSandbox({},'state');if(decoyWitness.contentWindow.S.clicked) throw Error('decoy clicked');return value;}})()",
            json!(ID)
        );
        run(fixture(&[], async |browser, origin| {
            assert_eq!(
                browser
                    .check(&format!("{origin}/"), "state", &expression)
                    .await?,
                state()
            );
            Ok(())
        }))
        .unwrap();
    }
}

/// Live target removal is reported rather than falling back to a different frame.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_inspection_propagates_target_errors() {
    for (pending, shared) in [(false, false), (true, false), (false, true), (true, true)] {
        let args = if shared {
            vec!["--disable-features=IsolateSandboxedIframes".to_owned()]
        } else {
            Vec::new()
        };
        let setup = if pending {
            "window.done=new Promise(()=>{});Object.defineProperty(window,'S',{get(){parent.postMessage({removeOnState:true},'*');return {started:true};}});"
        } else {
            "window.done=Promise.resolve();"
        };
        let expression = format!(
            "(async()=>{{await ({MOUNT})(false,0,false,{});addEventListener('message',event=>{{if(event.source===selected.contentWindow&&event.data?.removeOnState){{window.removed=true;selected.remove();}}}});if(!{pending}) selected.remove();return inspectSandbox({},'state');}})()",
            json!(setup),
            json!(ID)
        );
        run(fixture(&args, async |browser, origin| {
            let mut page = browser.page("removed").await?;
            page.initialize().await?;
            page.navigate(&format!("{origin}/")).await?;
            let error = page.evaluate(&expression).await.unwrap_err();
            if pending {
                assert_eq!(page.evaluate("window.removed").await?, true);
            } else {
                assert!(error.to_string().contains("mounted frame"), "{error}");
            }
            page.close().await
        }))
        .unwrap();
    }
}

/// Every reachable document is traversed once even when many unrelated frames precede the mount.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_inspection_walks_each_document_once() {
    for count in [8, 16, 32] {
        let preceding = count - 2;
        let expression = format!(
            r"(async()=>{{await ({MOUNT})(true,{preceding});window.work={{queries:0,frames:0}};
          const documents=[document];for(const doc of documents) for(const frame of doc.querySelectorAll('iframe')) if(frame.contentDocument) documents.push(frame.contentDocument);
          for(const doc of documents){{const query=doc.querySelectorAll.bind(doc);doc.querySelectorAll=selector=>{{const values=query(selector);if(selector!=='iframe') return values;work.queries++;return {{*[Symbol.iterator](){{for(const value of values){{work.frames++;yield value;}}}}}};}};}}
          const value=await inspectSandbox({},'state');return {{value,work}};}})()",
            json!(ID)
        );
        run(fixture(&[], async |browser, origin| {
            let value = browser
                .check(&format!("{origin}/"), "linear", &expression)
                .await?;
            assert_eq!(value["value"], state());
            assert!(value["work"]["queries"].as_u64().unwrap() <= count + 2);
            assert!(value["work"]["frames"].as_u64().unwrap() <= count + 2);
            Ok(())
        }))
        .unwrap();
    }
}

/// Selected-renderer native clicks deliver one fresh trusted receipt for HTML and SVG targets.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_click_confirms_native_delivery_before_receipt_wait() {
    for shared in [false, true] {
        for (nested, svg) in [(false, false), (false, true), (true, false), (true, true)] {
            let args = if shared {
                vec!["--disable-features=IsolateSandboxedIframes".into()]
            } else {
                Vec::new()
            };
            let expression = format!(
                r"(async()=>{{await ({MOUNT})({nested},0,{svg});
          const frame=selected;const receipt=new Promise(resolve=>frame.ownerDocument.defaultView.addEventListener('message',event=>{{if(event.source===frame.contentWindow&&event.data?.type==='pointer-done') resolve(event.data);}}));
          await inspectSandbox({},'click');return receipt;}})()",
                json!(ID)
            );
            run(fixture(&args, async |browser, origin| {
                let value = browser
                    .check(&format!("{origin}/"), "click", &expression)
                    .await?;
                assert_eq!(
                    value,
                    json!({"type":"pointer-done","prevented":false,"trusted":true,"atTarget":true})
                );
                assert_eq!(
                    browser
                        .lines()
                        .iter()
                        .filter(|line| line.ends_with(&format!("click returned {ID}")))
                        .count(),
                    1
                );
                Ok(())
            }))
            .unwrap();
        }
    }
}

/// Native keyboard and pointer bindings produce trusted input in the retained document.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn native_keyboard_pointer_and_accessibility_bindings_work() {
    run(fixture(&[], async |browser, origin| {
        let value=browser.check(&format!("{origin}/"),"bindings",r#"(async()=>{
          document.body.innerHTML='<button aria-label="Named">click</button><input aria-label="Message">';
          const button=document.querySelector('button');let trusted=false;button.onclick=e=>trusted=e.isTrusted;
          const box=button.getBoundingClientRect();await driverPointer(JSON.stringify([box.x+box.width/2,box.y+box.height/2,true]));
          await driverPress('Tab');const input=document.querySelector('input');const focused=document.activeElement===input;
          await driverPress('Shift+Tab');const returned=document.activeElement===button;
          const tree=await driverAria('body');return {trusted,focused,returned,tree};})()"#).await?;
        assert_eq!(value["trusted"],true);assert_eq!(value["focused"],true);assert_eq!(value["returned"],true);
        let tree=value["tree"].as_str().unwrap();assert!(tree.contains("button \"Named\""),"{tree}");assert!(tree.contains("textbox \"Message\""),"{tree}");
        Ok(())
    })).unwrap();
}

/// Preflight reports all missing and non-callable exports before any ready export runs.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn inventory_requires_every_callable_export() {
    for name in ["absent", "not_callable"] {
        let error = run(fixture(&[], async |browser, origin| {
            browser
                .preflight(origin, &["ready".into(), name.into()])
                .await
        }))
        .unwrap_err();
        assert!(error.to_string().contains(name), "{error}");
        assert!(!error.to_string().contains("must not run"), "{error}");
    }
}

/// Suppressed native input and a synthetic click cannot satisfy the selected-child receipt.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_click_rejects_absent_or_untrusted_delivery() {
    for shared in [false, true] {
        for untrusted in [false, true] {
            run(delivery(
                shared,
                if untrusted {
                    Delivery::Untrusted
                } else {
                    Delivery::Absent
                },
            ))
            .unwrap();
        }
    }
}

/// Native input delivered to an overlapping child element is not a selected-target click.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_click_rejects_delivery_to_another_target() {
    let mount=MOUNT.replace("const loaded=new Promise(resolve=>target.onload=resolve);", "target.srcdoc+='<style>#other{position:fixed;inset:0;width:100vw;height:100vh;z-index:9999}</style><button id=other>other</button>';const loaded=new Promise(resolve=>target.onload=resolve);");
    let expression = format!(
        "(async()=>{{await ({mount})();await inspectSandbox({},'click');}})()",
        json!(ID)
    );
    let error = run(fixture(&[], async |browser, origin| {
        browser
            .check(&format!("{origin}/"), "overlap", &expression)
            .await
            .map(|_| ())
    }))
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&format!("native click did not reach mounted target {ID}")),
        "{error}"
    );
}

/// Suppression on the second click cannot reuse the first click's trusted receipt.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_click_does_not_reuse_an_earlier_receipt() {
    for shared in [false, true] {
        run(delivery(shared, Delivery::Stale)).unwrap();
    }
}

/// Native-delivery failure variations.
#[derive(Clone, Copy, Debug)]
enum Delivery {
    /// Native input is suppressed without a replacement event.
    Absent,
    /// A synthetic event replaces the suppressed native event.
    Untrusted,
    /// A prior native receipt exists before input suppression.
    Stale,
}

/// Inspect real selected-session suppression without replacing the native input transport.
async fn delivery(shared: bool, mode: Delivery) -> io::Result<()> {
    let untrusted = matches!(mode, Delivery::Untrusted);
    let stale = matches!(mode, Delivery::Stale);
    let args = if shared {
        vec!["--disable-features=IsolateSandboxedIframes".into()]
    } else {
        Vec::new()
    };
    fixture(&args,async |browser,origin| {
        let mut page=browser.page("delivery").await?;page.initialize().await?;page.navigate(&format!("{origin}/")).await?;
        let setup=if untrusted {"window.done=Promise.resolve();const scroll=Element.prototype.scrollIntoView;Element.prototype.scrollIntoView=function(...args){scroll.apply(this,args);this.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,composed:true}));};"} else {"window.done=Promise.resolve();"};
        page.evaluate(&format!("({MOUNT})(false,0,false,{})",json!(setup))).await?;
        let targets=page.send(None,"Target.getTargets",json!({})).await?;
        let selected=targets["targetInfos"].as_array().unwrap().iter().find(|target| target["type"]=="iframe");
        assert_eq!(selected.is_none(),shared,"actual selected-renderer route: {targets}");
        let session=if let Some(selected)=selected {
            let attached=page.send(None,"Target.attachToTarget",json!({"targetId":selected["targetId"],"flatten":true})).await?;
            Some(attached["sessionId"].as_str().unwrap().to_owned())
        } else {None};
        let inspect=format!("inspectSandbox({},'click')",json!(ID));
        if stale {page.evaluate(&inspect).await?;}
        if let Some(session)=session.as_deref() {page.send(Some(session),"Input.setIgnoreInputEvents",json!({"ignore":true})).await?;}
        else {page.execute("Input.setIgnoreInputEvents",json!({"ignore":true})).await?;}
        let error=page.evaluate(&inspect).await.unwrap_err();
        assert!(error.to_string().contains(&format!("native click did not reach mounted target {ID}")),"{error}");
        if let Some(session)=session.as_deref() {
            page.send(Some(session),"Input.setIgnoreInputEvents",json!({"ignore":false})).await?;
            page.send(None,"Target.detachFromTarget",json!({"sessionId":session})).await?;
        } else {page.execute("Input.setIgnoreInputEvents",json!({"ignore":false})).await?;}
        page.close().await?;
        assert_eq!(browser.lines().iter().filter(|line|line.ends_with(&format!("click returned {ID}"))).count(),usize::from(stale));
        Ok(())
    }).await
}

/// A containing-document overlay cannot redirect isolated-renderer native input.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_click_bypasses_containing_page_coordinate_dispatch() {
    let expression = format!(
        r"(async()=>{{await ({MOUNT})();const overlay=document.createElement('div');overlay.style='position:fixed;inset:0;z-index:99999';document.body.append(overlay);let containing=0;overlay.onclick=()=>containing++;
      const frame=selected;const receipt=new Promise(resolve=>addEventListener('message',event=>{{if(event.source===frame.contentWindow&&event.data?.type==='pointer-done') resolve(event.data);}}));
      await inspectSandbox({},'click');return {{receipt:await receipt,containing}};}})()",
        json!(ID)
    );
    run(fixture(&[], async |browser, origin| {
        let value = browser
            .check(&format!("{origin}/"), "isolated", &expression)
            .await?;
        assert_eq!(value["containing"], 0);
        assert_eq!(value["receipt"]["trusted"], true);
        assert_eq!(value["receipt"]["atTarget"], true);
        Ok(())
    }))
    .unwrap();
}

/// Successful and rejected sandbox operations leave no retained page or iframe target.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn sandbox_targets_close_after_native_operations() {
    run(fixture(&[], async |browser, origin| {
        for missing in [false,true] {
            let expression=format!("(async()=>{{await ({MOUNT})();if({missing}) selected.remove();await inspectSandbox({},'click');}})()",json!(ID));
            let result=browser.check(&format!("{origin}/"),"target-cleanup",&expression).await;
            assert_eq!(result.is_ok(),!missing);
            let after=browser.execute("Target.getTargets",json!({})).await?;
            assert!(!after["targetInfos"].as_array().unwrap().iter().any(|target|target["type"]=="iframe"||target["url"].as_str().is_some_and(|url|url.starts_with(origin))),"retained targets: {after}");
        }
        Ok(())
    }))
    .unwrap();
}

/// The driver's switches and the caller's reach the browser as one switch each.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn launch_switches_reach_chromium() {
    let args = vec!["--disable-features=IsolateSandboxedIframes".to_owned()];
    run(fixture(&args, async |browser, _| {
        let line = browser
            .execute("Browser.getBrowserCommandLine", json!({}))
            .await?;
        let arguments: Vec<&str> = line["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|argument| argument.as_str())
            .collect();
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.starts_with("----")),
            "{arguments:?}"
        );
        for switch in [
            "--force-device-scale-factor=1.25",
            "--js-flags=--expose-gc",
            "--no-startup-window",
        ] {
            assert!(arguments.contains(&switch), "{switch} in {arguments:?}");
        }
        let features = arguments
            .iter()
            .find_map(|argument| argument.strip_prefix("--disable-features="))
            .unwrap_or_default();
        assert!(
            features
                .split(',')
                .any(|name| name == "IsolateSandboxedIframes"),
            "{arguments:?}"
        );
        Ok(())
    }))
    .unwrap();
}

/// A fresh browser holds no page or frame target before the driver opens its own.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn browser_holds_only_driver_pages() {
    run(fixture(&[], async |browser, _| {
        let targets = browser.execute("Target.getTargets", json!({})).await?;
        assert!(
            !targets["targetInfos"]
                .as_array()
                .unwrap()
                .iter()
                .any(|target| matches!(target["type"].as_str(), Some("page" | "iframe"))),
            "startup targets: {targets}"
        );
        Ok(())
    }))
    .unwrap();
}

/// Both successful assertions and exceptions close the actual listening endpoint.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn resources_close_after_success_and_failure() {
    for expression in [
        "Promise.resolve()",
        "Promise.reject(Error('cleanup sentinel'))",
    ] {
        let mut endpoint = String::new();
        let result = run(fixture(&[], async |browser, origin| {
            endpoint = origin.to_owned();
            browser
                .check(&format!("{origin}/"), "cleanup", expression)
                .await
                .map(|_| ())
        }));
        assert_eq!(result.is_ok(), expression == "Promise.resolve()");
        assert!(
            run(async {
                reqwest::Client::builder()
                    .no_proxy()
                    .build()
                    .unwrap()
                    .get(endpoint)
                    .send()
                    .await
            })
            .is_err()
        );
    }
}
