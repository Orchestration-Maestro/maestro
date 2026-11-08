//! Supplied-body completion witnesses using native browser capabilities.
use serde_json::{Value, json};

/// A successfully returned value and the ordinary completion trace.
fn returned(source: &'static str, value: Value) -> (&'static str, Value) {
    (
        source,
        json!({"log":["complete"],"error":null,"value":value,"helper":null}),
    )
}

/// The measured native-browser corpus, with the cancelled return first.
pub(crate) fn measured(id: &str) -> Vec<(&'static str, Value)> {
    vec![
        returned(
            "for(let i=0;i<1;i++){try{return 1}finally{continue}}",
            Value::Null,
        ),
        (
            "S.started=true;S.metaUrl=import.meta.url;S.base=document.baseURI;document.body.dataset.probe='native';S.helper=(await import('./helper.js')).answer;await fetch('./helper.js');await new Promise(r=>setTimeout(r,0));return [document.body.dataset.probe,typeof document.createElement,import.meta.url===document.baseURI,42];",
            json!({"log":["complete"],"error":null,"value":["native","function",true,42],"helper":"helper-ok"}),
        ),
        (
            "S.started=true;window.__completionCallbacks=[async ok=>{await null;S.log.push('callback:'+ok)}];S.metaUrl=import.meta.url;S.base=document.baseURI;S.helper=(await import('./helper.js')).answer;await null;return [sandboxId,42];",
            json!({"log":["callback:true","complete"],"error":null,"value":[id,42],"helper":"helper-ok"}),
        ),
        (
            "window.__completionCallbacks=[async ok=>{await null;S.log.push('callback:'+ok)}];throw new TypeError('user failure');",
            json!({"log":["callback:false","complete"],"error":{"message":"user failure"},"value":null,"helper":null}),
        ),
        (
            "window.__completionCallbacks=[async ok=>{await null;S.log.push('callback:'+ok)}];await null;return 42;",
            json!({"log":["callback:true","complete"],"error":null,"value":42,"helper":null}),
        ),
        (
            "window.__completionCallbacks=[()=>{throw Error('callback failure')},ok=>{S.log.push('second:'+ok)}];return 'kept';",
            json!({"log":["complete"],"error":null,"value":"kept","helper":null}),
        ),
        returned("try{return 1}finally{return 2}", json!(2)),
        returned("const inner=()=>{return 7};return inner()", json!(7)),
        returned("return {then(resolve){resolve(42)}}", json!(42)),
        (
            "return Promise.reject(new TypeError('user failure'))",
            json!({"log":["complete"],"error":{"message":"user failure"},"value":null,"helper":null}),
        ),
        returned("await null;", Value::Null),
    ]
}

/// A cancelled return inside a finalizer must not replace an outer pending return.
pub(crate) fn finalizer_restoration() -> Vec<(&'static str, Value)> {
    vec![
        returned(
            "return (async()=>{try{return 1}finally{for(let i=0;i<1;i++){try{return 2}finally{break}}}})()",
            json!(1),
        ),
        returned(
            "try{return 1}finally{for(let i=0;i<1;i++){try{return 2}finally{break}}}",
            json!(1),
        ),
        returned(
            "try{return 1}finally{for(let i=0;i<1;i++){try{return 2}finally{continue}}}",
            json!(1),
        ),
        returned(
            "try{return 1}finally{outer:{try{return 2}finally{break outer}}}",
            json!(1),
        ),
        returned(
            "try{return 1}finally{try{try{return 2}finally{throw 3}}catch{}}",
            json!(1),
        ),
        returned(
            "try{return 1}finally{try{return 2}finally{for(let i=0;i<1;i++){try{return 3}finally{break}}}}",
            json!(2),
        ),
        returned(
            "try{return 1}finally{for(let i=0;i<1;i++){try{return {then(){S.log.push('assimilated')}}}finally{break}}}",
            json!(1),
        ),
        returned("try{return 1}finally{}", json!(1)),
        returned(
            "try{return 1}finally{await null;for(let i=0;i<1;i++){try{return 2}finally{break}}}",
            json!(1),
        ),
        returned(
            "let __maestro5=7;try{return __maestro5}finally{for(let i=0;i<1;i++){try{return 2}finally{break}}}",
            json!(7),
        ),
    ]
}

/// Finally, catch, loop, scope and namespace combinations that distinguish abrupt completions.
pub(crate) fn control_flow() -> Vec<(&'static str, Value)> {
    vec![
        returned("outer:{try{return 1}finally{break outer}}", Value::Null),
        returned(
            "try{try{return 1}finally{throw 2}}catch(error){return error+1}",
            json!(3),
        ),
        returned(
            "let n=0;outer:for(let i=0;i<2;i++){try{try{return ++n}finally{if(i===0)continue outer}}finally{n+=10}}",
            json!(12),
        ),
        returned(
            "if(false){var value=1}var result=typeof value;for(var i=0;i<2;i++){}return [result,i]",
            json!(["undefined", 2]),
        ),
        returned(
            "return f();function f(){return 1}function f(){return 2}var f;",
            json!(2),
        ),
        returned(
            "let x=1;function f(){return x}var f;f=()=>2;return f()",
            json!(2),
        ),
        returned("let x=1;function f(){return x}var f;return f()", json!(1)),
        returned(
            "function* inner(){return 9}return inner().next().value",
            json!(9),
        ),
        returned(
            "try{throw 1}catch(error){var error=2}return typeof error",
            json!("undefined"),
        ),
        returned("return\n99", Value::Null),
        returned(
            "const text='\\uD800';return text.charCodeAt(0)",
            json!(55296),
        ),
        returned("return /return\\s+1/.test('return 1')", json!(true)),
        returned(
            "function f(){return f}const saved=f;f=1;return saved()",
            json!(1),
        ),
        returned(
            "class C{static value(){return 8}}return C.value()",
            json!(8),
        ),
        returned(
            "var __maestro0=1;let __maestro1=2;const nested=()=>__maestro0+__maestro1;__maestro2:{return nested()}",
            json!(3),
        ),
        returned("let \\u005f_maestro0=4;return __maestro0", json!(4)),
        returned(
            "window['__maestro'+0]++;return window['__maestro'+0]",
            json!(3),
        ),
    ]
}

/// Iterator close errors must override return/break, unlike throw completions.
pub(crate) fn cleanup() -> Vec<(&'static str, Value)> {
    vec![
        (
            "window.__completionCallbacks=[,ok=>S.log.push('callback:'+ok)];return 9;",
            json!({"log":["callback:true","complete"],"error":null,"value":9,"helper":null}),
        ),
        returned(
            "const it={[Symbol.iterator](){return this},next(){return {value:1,done:false}},return(){throw 9}};try{for(const n of it){return n}}catch(error){return error}",
            json!(9),
        ),
        returned(
            "const it={[Symbol.asyncIterator](){return this},async next(){return {value:1,done:false}},async return(){throw 9}};try{for await(const n of it){return n}}catch(error){return error}",
            json!(9),
        ),
        returned(
            "for(let i=0;i<1;i++){try{return {then(){S.log.push('assimilated')}}}finally{continue}}",
            Value::Null,
        ),
        returned(
            "window.__completionCallbacks=[42,()=>S.log.push('second')];return 42",
            json!(42),
        ),
        (
            "window.__completionCallbacks=[Object.assign(function(ok){S.log.push('callback:'+ok)},{call:null})];return 42;",
            json!({"log":["callback:true","complete"],"error":null,"value":42,"helper":null}),
        ),
        (
            "const finish=complete;let first=true;window.__completionCallbacks=[ok=>S.log.push('callback:'+ok)];window.complete=async(error,value)=>{if(first){first=false;S.log.push('refused');throw new TypeError('user failure')}return finish(error,value)};return 42;",
            json!({"log":["callback:true","refused","callback:false","complete"],"error":{"message":"user failure"},"value":null,"helper":null}),
        ),
        (
            "window.__completionCallbacks=[()=>Promise.reject(1),()=>S.log.push('second')];return 42;",
            json!({"log":["second","complete"],"error":null,"value":42,"helper":null}),
        ),
        (
            "try{return 1}finally{try{throw 9}catch({x=3}){S.log.push(x)}}",
            json!({"log":[3,"complete"],"error":null,"value":1,"helper":null}),
        ),
        (
            "try{return (S.log.push('return'),1)}finally{S.log.push('finally');return 2}",
            json!({"log":["return","finally","complete"],"error":null,"value":2,"helper":null}),
        ),
    ]
}

/// Real tool inputs, plus early errors that the browser must still refuse.
pub(crate) fn admission() -> Vec<(&'static str, Value)> {
    let mut cases = vec![(
        include_str!("../sandbox_loading_support/program_inputs/user.js"),
        json!({"log":["complete"],"error":null,"value":"</ScRiPt> <!--<script> </script> <![CDATA[ ]]> &amp; &lt;x> 雪","helper":"helper-ok"}),
    )];
    for source in [
        "break missing;",
        "continue missing;",
        "break __maestro2;",
        "let duplicate;let duplicate;",
        "with({}){}",
    ] {
        cases.push((
            source,
            json!({"log":["complete"],"error":{"name":"SyntaxError"},"value":null,"helper":null}),
        ));
    }
    cases
}
