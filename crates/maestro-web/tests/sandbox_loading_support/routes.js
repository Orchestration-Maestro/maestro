import {assert,equal} from './native.js';
let state;
export function begin() {
    assert(!state,'one native transaction');
    const add = window.addEventListener, remove = window.removeEventListener, open = window.open;
    const active = new Map(), opened = [], tabs = [], messages = [];
    state = {add, remove, open, active, opened, tabs, messages, deliveries:0, mode:'', adds:0, frames:[], received:[], providerMessages:[], replies:[], restorers:[]};
    window.addEventListener = function(kind, callback, ...rest) {
        if (kind !== 'message') return add.call(this,kind,callback,...rest);
        if (state.mode === 'register' || (state.mode === 'register-adapter' && ++state.adds === 2)) throw new DOMException('controlled registration','InvalidStateError');
        const wrapped = event => { state.deliveries++; callback(event); };
        add.call(this,kind,wrapped,...rest); active.set(callback,wrapped);
    };
    window.removeEventListener = function(kind,callback,...rest) {
        if (kind !== 'message' || !active.has(callback)) return remove.call(this,kind,callback,...rest);
        if (state.mode === 'detach') throw new DOMException('controlled detach','InvalidStateError');
        remove.call(this,kind,active.get(callback),...rest); active.delete(callback);
    };
    const observer = event => { if(event.data?.type === 'code-received') state.received.push(event.data); if(event.data?.type === 'response-received') state.replies.push(event.data.reply); };
    add.call(window,'message',observer); state.restorers.push(()=>remove.call(window,'message',observer));
    window.open = (...args) => { opened.push(args); return null; };
}
export function end() {
    const s = state;
    for (const restore of s.restorers.reverse()) restore();
    equal(s.active.size,0,'all native registrations released');
    window.addEventListener = s.add; window.removeEventListener = s.remove; window.open = s.open;
    state = undefined;
}
function current() { return [...document.querySelectorAll('[data-sandbox-test] iframe')].at(-1); }
export function arm(kind) {
    state.wait = new Promise(resolve => {
        const listener = event => { if(event.data?.type === kind) { state.remove.call(window,'message',listener); resolve(event.data); } };
        state.add.call(window,'message',listener);
    });
}
export function waited() { return state.wait; }
export function content() { return `<html><head></head><body><a href="https://example.org/fixture"><span>link</span></a><script>
window.addEventListener('message', e=>{if(e.data.type==='click-now'){document.querySelector('span').click(); parent.postMessage({type:'clicked'},'*');}if(e.data.type==='send-probe'){sendRuntimeMessage({type:'console',method:'log',text:'normal',args:[]}).then(()=>parent.postMessage({type:'probe-done'},'*'));}});
parent.postMessage({type:'loaded',first:window.firstProvider,second:window.secondProvider},'*');</script></body></html>`; }
function send(frame, value) { frame.contentWindow.postMessage(value,'*'); }
export function dispatch(kind, source) {
    const frames = [...document.querySelectorAll('[data-sandbox-test] iframe')];
    const frame = current();
    if (frame && !state.frames.includes(frame.contentWindow)) state.frames.push(frame.contentWindow);
    const selected = source === 'current' ? frame?.contentWindow : source === 'previous' ? state.frames[0] : source === 'sibling' ? frames[0]?.contentWindow : source === 'foreign' ? window : null;
    window.dispatchEvent(new MessageEvent('message',{data:{type:kind,url:'https://example.org/controlled',error:'bootstrap problem',stack:'native stack'},source:selected}));
}
export function record_provider(message) { state.providerMessages.push(message); }
export function record(message) { state.messages.push(message); }
export function closed(expected) {
    equal(state.active.size,expected,'native registration set after close');
    const before = state.deliveries, opens = state.opened.length;
    window.dispatchEvent(new MessageEvent('message',{data:{type:'open-external-url',url:'https://example.org/closed'},source:window}));
    if(expected === 0) equal(state.deliveries,before,'no native callback survives close');
    equal(state.opened.length,opens,'closed source cannot open');
}
export function url(mode) {
    return 'data:text/html,'+encodeURIComponent(`<script>
window.addEventListener('message',e=>{
 if(e.data.type==='runtime-response'){parent.postMessage({type:'response-received',reply:e.data},'*');}
 if(e.data.type==='send-ready'){parent.postMessage({type:'sandbox-ready'},'*');parent.postMessage({type:'ready-sent'},'*');}
 if(e.data.type==='send-error'){parent.postMessage({type:'sandbox-error',error:'bootstrap problem',stack:'native stack'},'*');parent.postMessage({type:'error-sent'},'*');}
 if(e.data.type==='sandbox-load'){parent.postMessage({type:'code-received',id:e.data.sandboxId,code:e.data.code},'*');parent.postMessage({type:'open-external-url',url:'https://example.org/extension'},'*');parent.postMessage({type:'url-done'},'*');}
});parent.postMessage({type:'url-mounted',mode:${JSON.stringify(mode)}},'*');</script>`);
}
export async function route_check(mode) {
    const frame = current();
    if (mode === 'srcdoc') {
        equal(frame.getAttribute('sandbox'),'allow-scripts allow-modals','sandbox flags');
        equal([frame.style.width,frame.style.height,frame.style.borderStyle],['100%','100%','none'],'frame sizing');
        arm('probe-done'); send(frame,{type:'send-probe'}); await waited();
        equal(state.messages.filter(x=>x.type==='console').map(x=>[x.method,x.text]),[['log','normal']],'normal console consumer route');
    } else if (mode === 'click') {
        const before = state.opened.length;
        arm('clicked'); send(frame,{type:'click-now'}); await waited();
        equal(state.opened.slice(before),[['https://example.org/fixture','_blank']],'one native open for nested click');
        equal(state.active.size,2,'one adapter plus shared router registration');
    } else if (mode === 'sibling-probe') {
        const before = state.messages.length;
        arm('probe-done'); send(frame,{type:'send-probe'}); await waited();
        equal(state.messages.slice(before).filter(x=>x.type==='console').map(x=>x.text),['normal'],'surviving sibling consumer');
    } else if (mode === 'unrelated') {
        const before = state.opened.length;
        for(const source of ['previous','sibling','absent','foreign']) dispatch('open-external-url',source);
        dispatch('unrelated','current');
        window.dispatchEvent(new MessageEvent('message',{data:'ordinary traffic',source:window}));
        equal(state.opened.length,before,'unrelated or stale source ignored');
        dispatch('open-external-url','current'); equal(state.opened.length,before+1,'live source accepted');
    } else if (mode === 'ready') {
        assert(frame.src && !frame.hasAttribute('srcdoc'),'URL not srcdoc');
        equal(state.opened.length,0,'nothing sent before readiness');
        dispatch('sandbox-ready','foreign'); dispatch('sandbox-error','absent');
        arm('url-done'); send(frame,{type:'send-ready'}); await waited();
        equal(state.opened,[['https://example.org/extension','_blank']],'ready settles current URL load');
        equal(state.received.length,1,'one sandbox-load'); assert(state.received[0].code.includes('url branch') && state.received[0].id.startsWith('url-'),'actual id and document forwarded');
        const before = state.opened.length;
        arm('ready-sent'); send(frame,{type:'send-ready'}); await waited();
        arm('error-sent'); send(frame,{type:'send-error'}); await waited();
        equal(state.opened.length,before,'late ready and error cannot resettle');
    } else if (mode === 'error') {
        dispatch('sandbox-error','foreign'); dispatch('sandbox-error','absent');
        arm('response-received'); send(frame,{type:'send-error'}); await waited();
        equal(state.providerMessages.map(x=>x.error),[{message:'bootstrap problem',stack:'native stack'}],'provider error delivery');
        equal(state.replies.map(x=>[x.type,x.sandboxId,x.bootstrapAcknowledged]),[['runtime-response','url-error',true]],'normal admitted response construction');
        // A consumer acknowledgement is synchronous before the forwarding promise settles.
        const errors = state.messages.filter(x=>x.type==='execution-error');
        equal(errors.map(x=>({sandboxId:x.sandboxId,type:x.type,error:x.error})),[{sandboxId:'url-error',type:'execution-error',error:{message:'bootstrap problem',stack:'native stack'}}],'source-checked execution error');
        arm('ready-sent'); send(frame,{type:'send-ready'}); await waited();
        equal(state.opened.length,0,'error cannot later send sandbox-load');
    } else if (mode === 'tabs') {
        arm('url-done'); send(frame,{type:'send-ready'}); await waited();
        equal(state.tabs,[{url:'https://example.org/extension'}],'extension native tabs route');
        equal(state.opened,[],'tabs does not also open a window');
    } else if (mode === 'reset-opens') { state.opened.length = 0; state.received.length = 0;
    } else if (mode === 'empty') {
        equal(document.querySelectorAll('[data-sandbox-test] iframe').length,0,'failed attempt removed frame');
        closed(0);
    }
}
export function fault(mode) {
    state.mode = mode; state.adds = 0;
    if (mode === 'sibling') {
        const parent = document.createElement('div'); parent.setAttribute('data-sandbox-test',''); const frame = document.createElement('iframe'); parent.append(frame); document.body.append(parent);
    }
    if (mode === 'tabs') {
        const old = globalThis.chrome?.tabs;
        globalThis.chrome ??= {}; globalThis.chrome.tabs = {create:options=>state.tabs.push(options)};
        state.restorers.push(()=>{if(old === undefined) delete globalThis.chrome.tabs; else globalThis.chrome.tabs = old;});
    }
    if (mode === 'parse-throw' || mode === 'parse-no-message') {
        const original = DOMParser.prototype.parseFromString;
        DOMParser.prototype.parseFromString = function(...args) { if(state.mode === 'parse-throw') throw new Error('controlled parse failure'); if(state.mode === 'parse-no-message') throw {}; return original.apply(this,args); };
        state.restorers.push(()=>DOMParser.prototype.parseFromString=original);
    }
    if(mode === 'create' || mode === 'append') {
        const target = mode === 'create' ? document : document.querySelector('[data-sandbox-test]');
        const key = mode === 'create' ? 'createElement' : 'appendChild', original = target[key];
        target[key] = function(...args) { if(state.mode === mode && (mode === 'append' || args[0] === 'iframe')) throw new DOMException('controlled '+mode,'InvalidStateError'); return original.apply(this,args); };
        state.restorers.push(()=>target[key]=original);
    }
}

export async function validation_check(expected, cssText) {
    const frame = current();
    await new Promise(resolve=>frame.addEventListener('load',resolve,{once:true}));
    if(expected === '') {
        equal(frame.getAttribute('sandbox'),'allow-scripts allow-modals','native recovery is accepted');
        assert(!frame.srcdoc.includes('HTML Validation Error'),'no invented validation failure'); return;
    }
    assert(!frame.hasAttribute('sandbox'),'host error document');
    const doc = frame.contentDocument;
    equal(doc.querySelector('h3').textContent,'HTML Validation Error','heading');
    equal(doc.querySelector('pre').textContent,expected,'literal validation text');
    equal(doc.querySelectorAll('pre script,pre tag').length,0,'error text is not markup');
    equal(frame.contentWindow.executed,undefined,'validation text cannot execute');
    if(cssText) {
        const css = JSON.parse(cssText), style = frame.contentWindow.getComputedStyle(doc.documentElement);
        for(const [key,value] of Object.entries(css)) equal(style.getPropertyValue(key).trim(),value,'selected pack '+key);
        const body = frame.contentWindow.getComputedStyle(doc.body), pre = frame.contentWindow.getComputedStyle(doc.querySelector('pre'));
        const fontProbe = doc.createElement('span'); fontProbe.style.fontFamily = css['--maestro-font-mono'];
        equal(body.fontFamily,fontProbe.style.fontFamily,'selected mono family');
        equal(pre.whiteSpace,'pre-wrap','error wrapping'); equal(pre.overflowX,'auto','error scrolling');
    }
}
