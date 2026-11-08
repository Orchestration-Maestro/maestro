//! Real-browser regressions selected only by the pinned Chromium profile.

use maestro_test_browser::browser_driver;
use serde_json::json;
use std::{fmt::Write as _, io};

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

/// The console scenes answer from real screenshots: resting lines, the hovered summary and the
/// focused follow control pass only while they paint their roles, and a wrong role, a missing
/// outline, a pointer elsewhere, focus elsewhere and a summary off the page each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn console_scenes_require_their_painted_roles() {
    let hover = "const b=document.querySelector('.maestro-artifact-summary').getBoundingClientRect();await driverPointer(JSON.stringify([b.x+b.width/2,b.y+b.height/2,false]));";
    let focus = "document.querySelector('.maestro-artifact-follow').focus();";
    let ring = "2px solid #ff0000";
    for (scene, (log, text, outline), prepare, failure) in [
        ("console-rest", ("#ffff00", "#fff", ring), "", None),
        (
            "console-rest",
            ("#808080", "#fff", ring),
            "",
            Some(".maestro-artifact-log paints muted"),
        ),
        (
            "console-rest",
            ("#ffff00", "#fff", ring),
            "document.querySelector('.maestro-artifact-summary').style.transform='translateX(-2000px)';",
            Some(".maestro-artifact-summary must be visible"),
        ),
        ("console-hover", ("#ffff00", "#fff", ring), hover, None),
        (
            "console-hover",
            ("#ffff00", "#ffff00", ring),
            hover,
            Some("the hovered summary paints text"),
        ),
        (
            "console-hover",
            ("#ffff00", "#fff", ring),
            "",
            Some("the pointer must rest on the console summary"),
        ),
        ("console-focus", ("#ffff00", "#fff", ring), focus, None),
        (
            "console-focus",
            ("#ffff00", "#fff", "none"),
            focus,
            Some("accent along the top edge"),
        ),
        (
            "console-focus",
            ("#ffff00", "#fff", ring),
            "",
            Some("keyboard focus must reach the follow control"),
        ),
    ] {
        let result = run(fixture(&[], async |browser, origin| {
            browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{globalThis.colors=JSON.stringify({{muted:'#ffff00',error:'#ff00ff',text:'#ffffff',accent:'#ff0000','on-accent':'#00ff00'}});document.body.innerHTML='<style>.maestro-artifact-summary{{color:#ffff00}}.maestro-artifact-summary:hover{{color:{text}}}</style><div style=\"margin:20px;padding:8px;width:400px;background:#000;font:16px sans-serif\"><div style=\"display:flex;gap:8px\"><button class=\"maestro-artifact-summary\" style=\"flex:1;border:0;background:transparent;font:inherit\">console (1 error)</button><button class=\"maestro-artifact-follow\" style=\"border:0;padding:4px;width:28px;height:28px;background:#ff0000;outline:{outline};outline-offset:2px\"><svg width=\"20\" height=\"20\" style=\"display:block\"><rect width=\"20\" height=\"20\" fill=\"#00ff00\"/></svg></button></div><div class=\"maestro-artifact-log\" style=\"color:{log}\">[log] ordinary</div><div class=\"maestro-artifact-error\" style=\"color:#ff00ff\">[error] failed</div></div>';{prepare}return await readingPaint('{scene}');}})()")).await
        }));
        assert_scene(result, failure, &format!("{scene} {log} {text} {outline}"));
    }
}

/// Require `result` to be an accepted scene when `failure` is absent, and otherwise an error
/// naming `failure`.
fn assert_scene(result: io::Result<serde_json::Value>, failure: Option<&str>, case: &str) {
    match result {
        Ok(value) => assert!(
            failure.is_none() && value == json!(true),
            "{case}: expected {}",
            failure.unwrap_or("an accepted scene")
        ),
        Err(error) => assert!(
            failure.is_some_and(|failure| error.to_string().contains(failure)),
            "{case}: {error}"
        ),
    }
}

/// Run `prepare` then the paint scene `scene` on a fresh page whose body is `body`, with a
/// background role published.
fn paint_scene(body: &str, prepare: &str, scene: &str) -> io::Result<serde_json::Value> {
    run(fixture(&[], async |browser, origin| {
        browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{globalThis.colors=JSON.stringify({{background:'#102030'}});document.body.innerHTML='{body}';{prepare}return await readingPaint('{scene}');}})()")).await
    }))
}

/// The glyph scenes answer from the ink a real screenshot shows: each shape passes only for its
/// own glyph, and a glyph that paints nothing fails.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn console_glyph_scenes_require_their_painted_shapes() {
    let right = r#"<path d="m9 18 6-6-6-6"/>"#;
    let down = r#"<path d="m6 9 6 6 6-6"/>"#;
    let copy = r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#;
    let check = r#"<path d="M20 6 9 17l-5-5"/>"#;
    for (scene, (paths, stroke), failure) in [
        ("console-collapsed", (right, "#000"), None),
        (
            "console-collapsed",
            (down, "#000"),
            Some("paints a right chevron"),
        ),
        ("console-expanded", (down, "#000"), None),
        (
            "console-expanded",
            (right, "#000"),
            Some("paints a down chevron"),
        ),
        ("console-copy", (copy, "#000"), None),
        (
            "console-copy",
            (check, "#000"),
            Some("paints the copy icon"),
        ),
        ("console-copied", (check, "#000"), None),
        (
            "console-copied",
            (copy, "#000"),
            Some("paints a check mark"),
        ),
        ("console-copied", (check, "none"), Some("paints no ink")),
    ] {
        let svg = format!(
            r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="{stroke}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="display:block">{paths}</svg>"#
        );
        let body = format!(
            r#"<div style="margin:20px"><button class="maestro-artifact-summary">{svg}</button><copy-button>{svg}</copy-button></div>"#
        );
        let result = paint_scene(&body, "", scene);
        assert_scene(result, failure, &format!("{scene} {paths} {stroke}"));
    }
}

/// The reasoning scenes answer from real screenshots: a resting header passes only while its
/// label and right chevron paint the muted role, a hovered one only while both paint the text
/// role under the pointer, and an expanded one only while its chevron points down.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn thinking_scenes_require_their_painted_roles_and_chevron() {
    let hover = "const b=document.querySelector('.maestro-thinking-header').getBoundingClientRect();await driverPointer(JSON.stringify([b.x+b.width/2,b.y+b.height/2,false]));";
    let turn =
        "document.querySelector('.maestro-thinking-chevron').style.transform='rotate(90deg)';";
    for (scene, (label, chevron), prepare, failure) in [
        ("thinking-rest", ("inherit", "inherit"), "", None),
        (
            "thinking-rest",
            ("#808080", "inherit"),
            "",
            Some("span:last-child paints muted"),
        ),
        (
            "thinking-rest",
            ("inherit", "#808080"),
            "",
            Some("svg paints muted"),
        ),
        (
            "thinking-rest",
            ("inherit", "inherit"),
            turn,
            Some("paints a right chevron"),
        ),
        ("thinking-hover", ("inherit", "inherit"), hover, None),
        (
            "thinking-hover",
            ("inherit", "#ffff00"),
            hover,
            Some("svg paints text"),
        ),
        (
            "thinking-hover",
            ("inherit", "inherit"),
            "",
            Some("the pointer must rest on the reasoning header"),
        ),
        ("thinking-expanded", ("inherit", "inherit"), turn, None),
        (
            "thinking-expanded",
            ("inherit", "inherit"),
            "",
            Some("paints a down chevron"),
        ),
    ] {
        let body = format!(
            r#"<style>.maestro-thinking-header{{color:#ffff00}}.maestro-thinking-header:hover{{color:#ffffff}}</style><div style="margin:20px;padding:8px;width:400px;background:#000"><button class="maestro-thinking-header" style="display:flex;gap:8px;border:0;background:transparent;font:16px sans-serif"><span class="maestro-thinking-chevron" style="display:inline-flex;color:{chevron}"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="display:block"><path d="m9 18 6-6-6-6"/></svg></span><span style="color:{label}">Thinking...</span></button></div>"#
        );
        let result = run(fixture(&[], async |browser, origin| {
            browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{globalThis.colors=JSON.stringify({{muted:'#ffff00',text:'#ffffff'}});document.body.innerHTML='{body}';{prepare}return await readingPaint('{scene}');}})()")).await
        }));
        assert_scene(
            result,
            failure,
            &format!("{scene} {label} {chevron} {prepare}"),
        );
    }
}

/// The Word page scene passes only for a visible page that paints white over its box with
/// black text: a grey page, grey text and a missing page each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn word_page_scene_requires_a_white_page_and_black_text() {
    for (page, text, failure) in [
        ("white", "black", None),
        ("#eeeeee", "black", Some("paints white over its box")),
        ("white", "#808080", Some("text paints black ink")),
        ("", "", Some("a Word page must be visible")),
    ] {
        let body = if page.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="maestro-docx-artifact" style="width:300px;height:180px;overflow:auto;background:#102030"><section class="docx" style="padding:32px;background:{page};color:{text};font:16px sans-serif"><p>Body text of the page</p></section></div>"#
            )
        };
        let result = paint_scene(&body, "", "word-page");
        assert_scene(result, failure, &format!("{page} {text}"));
    }
}

/// Run the Markdown paint scene `scene` on a fresh page whose body is `body`, with one distinct
/// colour published per brand role.
fn markdown_scene(body: &str, scene: &str) -> io::Result<serde_json::Value> {
    run(fixture(&[], async |browser, origin| {
        browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{globalThis.colors=JSON.stringify({{text:'#ffffff',muted:'#ffff00',accent:'#ff0000',panel:'#00ff00',border:'#0000ff',error:'#ff00ff',success:'#00ffff',warning:'#ff8000',live:'#8000ff'}});document.body.innerHTML='{body}';return await readingPaint('{scene}');}})()")).await
    }))
}

/// The hover scene passes only while hovering the link repaints its text and leaves its image
/// and inline-code background alone: a repainted image, a repainted code background and an
/// unchanged link each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn markdown_hover_scene_requires_text_repaint_without_image_or_code_change() {
    let svg = r"data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%2220%22 height=%2220%22%3E%3Cpath fill=%22red%22 d=%22M0 0h20v20H0z%22/%3E%3C/svg%3E";
    for (style, failure) in [
        ("a:hover span{color:#ff0000}", None),
        ("", Some("the link text must repaint on hover")),
        (
            "a:hover span{color:#ff0000}a:hover img{opacity:.5}",
            Some("the linked image's paint must not change"),
        ),
        (
            "a:hover span{color:#ff0000}a:hover code{background:#00ff00 !important}",
            Some("the inline-code background must not change"),
        ),
    ] {
        let body = format!(
            r##"<style>{style}</style><div class="maestro-reading-viewport" style="background:#000;padding:20px"><div class="maestro-markdown" style="font:24px sans-serif"><a href="#hover" style="color:#fff"><span>hover text</span><img width="20" height="20" src="{svg}"><code style="background:#808080;color:#000">code</code></a></div></div>"##
        );
        assert_scene(markdown_scene(&body, "markdown-hover"), failure, style);
    }
}

/// The syntax scene passes only while every token class paints its theme role: a keyword in the
/// wrong role and a missing class each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn markdown_syntax_scene_requires_each_token_class_in_its_role() {
    let tokens = [
        ("comment", "#ffff00"),
        ("keyword", "#ff0000"),
        ("function", "#ff0000"),
        ("variable", "#ffffff"),
        ("string", "#00ffff"),
        ("number", "#ff8000"),
        ("type", "#8000ff"),
        ("operator", "#ffffff"),
        ("punctuation", "#ffffff"),
    ];
    for (wrong, missing, failure) in [
        ("", "", None),
        (
            "keyword",
            "",
            Some(".hljs-keyword must paint accent pixels"),
        ),
        ("", "type", Some("paint element missing")),
    ] {
        let mut spans = String::new();
        for (name, colour) in tokens.iter().filter(|(name, _)| *name != missing) {
            let colour = if *name == wrong { "#ffffff" } else { colour };
            write!(
                spans,
                r#"<div class="hljs-{name}" style="color:{colour}">{name}</div>"#
            )
            .unwrap();
        }
        let body = format!(
            r#"<div class="maestro-reading-viewport" style="background:#000;padding:20px"><div class="maestro-markdown" style="font:bold 24px sans-serif">{spans}</div></div>"#
        );
        assert_scene(
            markdown_scene(&body, "markdown-syntax"),
            failure,
            &format!("{wrong}{missing}"),
        );
    }
}

/// The brand scene passes only while the paragraph, link, inline code, quote, table header,
/// formula error and listing paint their roles: a link in the wrong role, a paragraph in the
/// body role while thinking and a listing without its border each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn markdown_brand_scene_requires_each_selected_role_to_paint() {
    for (root, paragraph, link, border, failure) in [
        ("normal", "#ffffff", "#ff0000", "8px solid #0000ff", None),
        ("italic", "#ffff00", "#ff0000", "8px solid #0000ff", None),
        (
            "normal",
            "#ffffff",
            "#ffffff",
            "8px solid #0000ff",
            Some(".maestro-markdown a must paint accent pixels"),
        ),
        (
            "italic",
            "#ffffff",
            "#ff0000",
            "8px solid #0000ff",
            Some(".maestro-markdown p must paint muted pixels"),
        ),
        (
            "normal",
            "#ffffff",
            "#ff0000",
            "0 none",
            Some(".maestro-markdown .maestro-code must paint border pixels"),
        ),
    ] {
        let body = format!(
            r##"<div class="maestro-reading-viewport" style="background:#000;padding:20px"><div class="maestro-markdown" style="font:bold {root} 24px sans-serif;color:#fff"><p style="color:{paragraph}">paragraph</p><a href="#a" style="color:{link}">link</a> <code style="background:#00ff00;color:#000">code</code><blockquote style="margin:0;border-left:8px solid #0000ff">quote</blockquote><table><tr><th style="background:#00ff00;color:#000">head</th></tr></table><span class="katex-error" style="color:#ff00ff">bad</span><div class="maestro-code" style="border:{border};height:20px"></div></div></div>"##
        );
        assert_scene(
            markdown_scene(&body, "markdown-brand"),
            failure,
            &format!("{root} {paragraph} {link} {border}"),
        );
    }
}

/// The listing scenes pass only for a listing that covers its box with the background role,
/// keeps its text inside the bounded body and paints again exactly as captured: a transparent
/// listing, an unclipped one, a changed scroll position and a missing capture each fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn html_source_scenes_require_covered_clipped_and_unchanged_paint() {
    let shown = "await readingPaint('html-source');";
    let scrolled = "await readingPaint('html-source');document.querySelector('.maestro-html-source').scrollTop=40;";
    let lines = "line\\n".repeat(40);
    for (scene, (clip, background), prepare, failure) in [
        ("html-source", ("auto", "#102030"), "", None),
        (
            "html-source",
            ("auto", "transparent"),
            "",
            Some("paints the background role over its box"),
        ),
        (
            "html-source",
            ("visible", "#102030"),
            "",
            Some("nowhere outside the body"),
        ),
        ("html-source-again", ("auto", "#102030"), shown, None),
        (
            "html-source-again",
            ("auto", "#102030"),
            scrolled,
            Some("paints as it did before it was hidden"),
        ),
        (
            "html-source-again",
            ("auto", "#102030"),
            "",
            Some("missing screenshot html-source"),
        ),
    ] {
        let body = format!(
            r#"<div style="margin:20px;width:400px;height:160px"><div class="maestro-html-artifact" style="position:relative;height:100%;overflow:{clip}"><div class="maestro-html-source" style="position:absolute;inset:0;overflow:{clip};background:{background}"><pre style="margin:0;font:12px/16px monospace;color:#ff8000"><code>{lines}</code></pre></div></div></div>"#
        );
        let result = paint_scene(&body, prepare, scene);
        assert_scene(
            result,
            failure,
            &format!("{scene} {clip} {background} {prepare}"),
        );
    }
}

/// A paint scene answers from real screenshots: text that paints nothing, an outer frame shadow
/// and an unknown scene each fail, and only the painted, unshadowed reading root passes.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn reading_paint_fails_when_the_painted_output_differs() {
    for (scene, colour, shadow, failure) in [
        ("neighbors-generic", "black", "none", None),
        (
            "neighbors-generic",
            "transparent",
            "none",
            Some("literal native text must actually paint"),
        ),
        (
            "neighbors-generic",
            "black",
            "0 0 0 9px black",
            Some("neighbors must gain no outer frame shadow"),
        ),
        (
            "unknown",
            "black",
            "none",
            Some("unknown paint scene unknown"),
        ),
        (
            "viewport-99",
            "black",
            "none",
            Some("unknown paint scene viewport-99"),
        ),
    ] {
        let result = run(fixture(&[], async |browser, origin| {
            browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{document.body.innerHTML='<div class=\"maestro-reading-frame\" style=\"margin:20px;box-shadow:{shadow}\"><div class=\"maestro-generic-artifact\"><p style=\"color:{colour}\">painted text</p></div></div>';return await readingPaint('{scene}');}})()")).await
        }));
        match result {
            Ok(value) => assert!(
                failure.is_none() && value == json!(true),
                "{scene} {colour} {shadow}: expected {}",
                failure.unwrap_or("an accepted scene")
            ),
            Err(error) => assert!(
                failure.is_some_and(|failure| error.to_string().contains(failure)),
                "{scene} {colour} {shadow}: {error}"
            ),
        }
    }
}

/// The focus scene passes only for an inset accent ring painted along every edge of the focused
/// control: a missing ring, an outer ring, a group clipping one edge and an unfocused page each
/// fail.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn control_focus_requires_an_unclipped_accent_ring() {
    let inset = "inset 0 0 0 2px #ff0000";
    for (group, shadow, focus, failure) in [
        ("80px", inset, true, None),
        ("80px", "none", true, Some("accent along the top edge")),
        (
            "80px",
            "0 0 0 2px #ff0000",
            true,
            Some("accent along the top edge"),
        ),
        ("76px", inset, true, Some("accent along the right edge")),
        ("80px", inset, false, Some("a control must hold focus")),
    ] {
        let result = run(fixture(&[], async |browser, origin| {
            browser.check(&format!("{origin}/"), "paint", &format!("(async()=>{{globalThis.colors=JSON.stringify({{accent:'#ff0000'}});document.body.innerHTML='<div style=\"display:inline-flex;overflow:hidden;margin:20px;width:{group}\"><button style=\"flex:none;border:0;outline:none;width:80px;height:28px;background:white;box-shadow:{shadow}\">x</button></div>';if({focus})document.querySelector('button').focus();return await readingPaint('control-focus');}})()")).await
        }));
        match result {
            Ok(value) => assert!(
                failure.is_none() && value == json!(true),
                "{group} {shadow} {focus}: expected {}",
                failure.unwrap_or("an accepted scene")
            ),
            Err(error) => assert!(
                failure.is_some_and(|failure| error.to_string().contains(failure)),
                "{group} {shadow} {focus}: {error}"
            ),
        }
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

/// The committed tool report paints its brand roles from its verified library copies, and no
/// request leaves the loopback origin.
#[test]
#[ignore = "runs in the pinned Chromium job"]
fn tool_report_paints_from_its_verified_assets() {
    let work = maestro_test_directory::Scratch::new().unwrap();
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    run(browser_driver::tool_report(
        &checkout,
        &support::chromium(),
        &support::prepared("MAESTRO_BROWSER_ASSETS"),
        work.path(),
    ))
    .unwrap();
}

/// Each inventory export as its own test on the package the browser export setup prepared,
/// so the test runner's hang limit stops and names an export that never completes.
mod exports {
    use super::browser_driver;
    use super::support::{chromium, prepared, run};
    use maestro_test_directory::Scratch;

    /// Run one export in its own browser.
    fn export(family: &str, name: &str) {
        let profile = Scratch::new().unwrap();
        run(browser_driver::run_export(
            &chromium(),
            profile.path(),
            &prepared("MAESTRO_BROWSER_EXPORTS"),
            family,
            name,
        ))
        .unwrap();
    }

    include!(concat!(env!("OUT_DIR"), "/exports.rs"));
}
