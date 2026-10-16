import {assert,equal,programSources} from './native.js';
const RAW = '</ScRiPt> <!--<script> </script> 雪';
const USER_RAW = '</ScRiPt> <!--<script> </script> <![CDATA[ ]]> &amp; &lt;x> 雪';
const HEAD_BOOT = '{const t=document.currentScript.previousElementSibling,s=t.content.firstChild.firstChild;t.replaceWith(s);s.remove()}';
const BODY_BOOT = 'document.currentScript.previousElementSibling.remove()';
export function program_url() { return location.origin+'/sandbox.html'; }
export function program_input(kind) {
    if(kind === 'raw') return RAW;
    if(kind === 'provider') return `function(){window.S={dcl:false,started:false};S.runtime=document.currentScript.textContent;S.raw=String.raw\`</ScRiPt> <!--<script> </script> 雪\`;S.data=window.obsData;document.addEventListener('DOMContentLoaded',()=>S.dcl=true);window.done=new Promise(r=>window.complete=async(error,value)=>{S.error=error;S.value=value;r()});if(location.hash==='#tt'){const m=document.createElement('meta');m.httpEquiv='Content-Security-Policy';m.content="script-src 'self' 'unsafe-inline'; require-trusted-types-for 'script'; trusted-types 'none'";document.head.append(m);}}`;
    if(kind === 'user') return `S.started=true;S.ready=document.readyState;S.beforeDcl=!S.dcl;S.metaUrl=import.meta.url;S.base=document.baseURI;S.cr=(()=>{/*\r*/}).toString().includes('\\r');S.helper=(await import('./helper.js')).answer;return String.raw\`</ScRiPt> <!--<script> </script> <![CDATA[ ]]> &amp; &lt;x> 雪\`;`;
    return `<head><script>document.addEventListener('click',e=>parent.postMessage({type:'pointer-done',prevented:e.defaultPrevented},'*'),true);</script></head>`;
}
export function artifact() {
    return `<html><head><meta http-equiv="Content-Security-Policy" content="img-src 'none'"><title>artifact</title><script>S.before=!!S.runtime;</script></head><body><img src="/pixel.png" onerror="S.blocked=true" onload="S.blocked=false"></body></html>`;
}
const attributes = f => [...f.attributes].map(a=>[a.name,a.value]);
let serial=0;
async function mount(html,url,policy='',trusted=false,hash='') {
    const prototype=document.querySelector('[data-sandbox-test] iframe');
    assert(prototype,'production-created frame');
    equal(prototype.hasAttribute('src'),url,'production loading path');
    const host=document.createElement('iframe');
    const hostLoaded=new Promise(resolve=>host.addEventListener('load',resolve,{once:true}));
    document.body.append(host);await hostLoaded;
    const doc=host.contentDocument;
    if(trusted) host.contentWindow.trustedTypes.createPolicy('default',{createHTML:t=>t});
    if(policy){const meta=doc.createElement('meta');meta.httpEquiv='Content-Security-Policy';meta.content=policy;doc.head.append(meta);}
    const box=doc.createElement('div');box.id='program-probe-'+ ++serial;doc.body.append(box);
    const frame=doc.createElement('iframe');
    for(const [key,value] of attributes(prototype)) frame.setAttribute(key,value);
    equal(attributes(frame),attributes(prototype),'complete production attribute set copied');
    equal(frame.getAttribute('sandbox'),prototype.getAttribute('sandbox'),'production sandbox flags');
    const loaded=new Promise(resolve=>{
        if(url) {
            const installed=e=>{if(e.source===frame.contentWindow&&e.data?.type==='sandbox-installed'){host.contentWindow.removeEventListener('message',installed);resolve();}};
            host.contentWindow.addEventListener('message',installed);
        } else frame.addEventListener('load',resolve,{once:true});
    });
    if(url) {
        const ready=e=>{if(e.source===frame.contentWindow&&e.data?.type==='sandbox-ready'){host.contentWindow.removeEventListener('message',ready);frame.contentWindow.postMessage({type:'sandbox-load',code:html},'*');}};
        host.contentWindow.addEventListener('message',ready);
    } else frame.srcdoc=html;
    if(hash && url) frame.src=program_url()+hash;
    box.append(frame);
    await loaded;
    // The driver reads the opaque frame after load; a started wrapper supplies its completion promise.
    const inspect=async()=>window.inspectSandbox(box.id,'state');
    return {frame,host,box,inspect,remove:()=>host.remove()};
}
async function digest(text) {
    const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));
    return "'sha256-"+btoa(String.fromCharCode(...new Uint8Array(bytes)))+"'";
}
export async function program_checks(html,ttHtml,url) {
    assert(html.includes('<template><svg><script>'),'runtime uses inert template carrier, not a resource');
    const [runtime,module]=programSources(html);
    assert(runtime.includes(program_input('provider')),'provider input is unchanged');
    assert(module.includes(program_input('user')),'user input is unchanged including CR');
    let mounted=await mount(html,url);
    try {
        const s=await mounted.inspect();
        equal(s.runtime,runtime,'entire assembled runtime text');
        equal([s.raw,s.data,s.value],[RAW,RAW,USER_RAW],'raw runtime, provider data and user values');
        equal([s.error,s.cr,s.ready,s.beforeDcl],[null,true,url?'complete':'interactive',true],'CR and module ordering');
        equal([s.metaUrl,s.helper],[s.base,'helper-ok'],'relative module resolution without base');
        equal([s.head,s.body,s.scaffold],['style,script','script',0],'no carrier scaffolds remain');
        equal(s.origin,'null','opaque sandbox origin');
    } finally {mounted.remove();}
    if(url) return;
    const hashes=await Promise.all([HEAD_BOOT,BODY_BOOT,runtime,module].map(digest));
    const encoded=t=>t.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/\r/g,'&#13;');
    const scenarios=[
        ['all',hashes,true,true],
        ['missing module',hashes.slice(0,3),true,false],
        ['missing runtime',[hashes[0],hashes[1],hashes[3]],false,false],
        ['encoded markup',[hashes[0],hashes[1],await digest(encoded(runtime)),await digest(encoded(module))],false,false],
        ['unmatched',[await digest('unrelated')],false,false],
    ];
    for(const [label,allowed,runs,completes] of scenarios) {
        mounted=await mount(html,false,"script-src 'self' "+allowed.join(' '));
        try {
            const s=await mounted.inspect();
            equal(!!s.runtime,runs,label+' runtime admission');
            equal(!!s.started,completes,label+' module admission');
            if(completes)equal(s.value,USER_RAW,label+' completion');
            console.log('POLICY '+label+': runtime='+!!s.runtime+' module='+!!s.started);
        } finally {mounted.remove();}
    }
    mounted=await mount(html,false,"script-src 'self' 'unsafe-inline'; require-trusted-types-for 'script'",true);
    try {equal((await mounted.inspect()).value,USER_RAW,'inherited Trusted Types with TrustedHTML-only default');}finally{mounted.remove();}
    // A provider installs this policy synchronously, before the parser reaches the module.
    mounted=await mount(ttHtml,false);
    try {equal((await mounted.inspect()).value,USER_RAW,'in-frame Trusted Types none');}finally{mounted.remove();}
}
export async function link_checks(html) {
    const svg=attrs=>`<svg width="200" height="40"><a ${attrs}><rect id="target" width="150" height="30"/></a></svg>`;
    const cases=[
        ['html','<a href="./html"><span id="target">link</span></a>','html'],
        ['href',svg('href="./vector"'),'vector'],
        ['xlink',svg('xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="./xlink"'),'xlink'],
        ['both',svg('href="./preferred" xlink:href="./ignored"'),'preferred'],
        ['empty',svg('href="" xlink:href="./ignored"'),''],
        ['missing',svg(''),null],
        ['animated',svg('href="./initial"').replace('<rect','<set attributeName="href" to="./animated" begin="0s" fill="freeze"/><rect'),'animated'],
    ];
    for(const [name,body,target] of cases) {
        const mounted=await mount(html.replace('</head>','<base href="https://example.org/links/"></head>')+'<body>'+body+'</body>',false);
        const routes=[];
        let done;
        const result=new Promise(resolve=>done=resolve);
        const listener=e=>{if(e.source!==mounted.frame.contentWindow)return;if(e.data?.type==='open-external-url')routes.push(e.data.url);if(e.data?.type==='pointer-done')done(e.data);};
        mounted.host.contentWindow.addEventListener('message',listener);
        try {
            await window.inspectSandbox(mounted.box.id,'click');
            const clicked=await result;
            equal(routes,target===null?[]:['https://example.org/links/'+target],name+' routes once');
            equal(clicked.prevented,target!==null,name+' native prevention');
            equal((await mounted.inspect()).href,'about:srcdoc',name+' frame does not navigate');
        } finally {mounted.host.contentWindow.removeEventListener('message',listener);mounted.remove();}
    }
}

export async function loaded_artifact(url) {
    const frame=document.querySelector('[data-sandbox-test] iframe');
    const ready=new Promise(resolve=>{
        if(url){const installed=e=>{if(e.source===frame.contentWindow&&e.data?.type==='sandbox-installed'){window.removeEventListener('message',installed);resolve();}};window.addEventListener('message',installed);}
        else frame.addEventListener('load',resolve,{once:true});
    });
    await ready;
    const parent=frame.parentElement;parent.id='artifact-probe';
    const s=await window.inspectSandbox(parent.id,'state');
    equal([s.before,s.metaParent,s.blocked,s.scaffold],[true,'head',true,0],'authored head policy and order');
    equal(s.head,'style,script,meta,title,script','head element order at public loading interface');
}
