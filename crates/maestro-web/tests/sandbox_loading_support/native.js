export function assert(value, message) { if (!value) throw new Error(message); }
export function equal(actual, expected, label) { assert(JSON.stringify(actual) === JSON.stringify(expected), label + ': ' + JSON.stringify(actual)); }
export function decode(value) {
    if (!value || typeof value !== 'object') return value;
    if ('$undefined' in value) return undefined;
    if ('$bigint' in value) return BigInt(value.$bigint);
    if ('$number' in value) return Number(value.$number);
    if ('$date' in value) return new Date(value.$date);
    if ('$bytes' in value) return new Uint8Array(value.$bytes);
    if ('$cycle' in value) { const cyclic = {}; cyclic.self = cyclic; return cyclic; }
    return Array.isArray(value) ? value.map(decode) : Object.fromEntries(Object.entries(value).map(([k,v]) => [k,decode(v)]));
}
export function finish() { document.querySelectorAll('[data-sandbox-test]').forEach(node => node.remove()); }
const stripComments = text => text.split('\n').filter(line => !line.trimStart().startsWith('//')).join('\n');
export function programSources(html) {
    const parsed = new DOMParser().parseFromString(html,'text/html');
    return [...parsed.querySelector('template').content.querySelectorAll('script'), ...parsed.querySelectorAll('svg script')].map(script=>script.textContent);
}
export async function observe_document(name, html, output) {
    const scripts = programSources(html);
    html=html.replace(/<template><svg><script>[\s\S]*?<\/script><\/svg><\/template><script>[\s\S]*?<\/script>/,'<script></script>');
    if(scripts[1])html=html.replace(/<svg display="none"><script type="module">[\s\S]*?<\/script><\/svg><script>[\s\S]*?<\/script>/,()=>'<script type="module">\n'+scripts[1]+'\t</script>');
    const suffix = html.slice(html.indexOf('</script>') + 9);
    equal(html.slice(0,html.indexOf('<style>')), output.prefix, name + ' prefix');
    equal(stripComments(suffix), stripComments(output.suffix), name + ' suffix');
    equal(scripts.length, output.script_count, name + ' scripts');
    equal(scripts[0].includes('window.sendRuntimeMessage'), output.bridge, name + ' bridge');
    equal(scripts[0].includes("document.addEventListener('click'"), output.navigation, name + ' navigation');
    let parseError = false; try { new Function(scripts[0]); } catch { parseError = true; }
    equal(parseError, output.script_parse_error, name + ' program parse');
    const frame = await realm();
    try {
        const win = frame.contentWindow;
        win.eval(scripts[0]);
        if (name === 'id_literal</script>tail') equal(win.sandboxId, 'literal</script>tail', name);
        if (name.startsWith('id_quote')) equal(win.sandboxId, 'quote"\\雪\n', name);
        if (name === 'runtime_closing_script') equal(win.literal, '</script>', name);
        if (name === 'invalid_key') equal(win['hyphen-name'], 1, name);
        if (name === 'numeric_keys') {
            const supplied = {}; new Function('window','document',scripts[0])(supplied, {addEventListener(){}});
            equal([supplied['1'], supplied['2'], supplied.a], ['one','two',3], name + ' generated values');
            equal(Object.keys(supplied).filter(k => ['1','2','a'].includes(k)), ['1','2','a'], name + ' canonical order');
            equal([win['1'],win['2'],win.a], [undefined,undefined,3], name + ' native Window indexed setters');
            equal([...win.calls], ['fixture-id'], name + ' native provider');
        }
        if (name === 'merge_order') { equal([win.a,win.b,win.c], [1,'new',3], name); assert(scripts[0].indexOf('window["a"]') < scripts[0].indexOf('window["b"]') && scripts[0].indexOf('window["b"]') < scripts[0].indexOf('window["c"]'), name + ' insertion order'); }
        if (name === 'provider_id_override') { equal(win.sandboxId, 'provider override', name); equal([...win.calls], ['fixture-id'], name + ' invocation id'); }
        const values = {value_null:null, value_string:'</ScRiPt>雪', value_number:0, value_nonfinite:null, value_boolean:true, value_object:{'1':'one','2':'two',b:1,a:2}, value_array:[null,'x',2], value_date:'2020-01-02T03:04:05.000Z', value_typed_array:{'0':1,'1':255}};
        if (name in values) equal(win.value, values[name], name + ' native value');
        if (output.completions) {
            const completions = [], logs = [];
            win.__completionCallbacks = [async ok => completions.push(['callback',ok])];
            win.complete = async (...args) => completions.push(args);
            win.console.error = (...args) => logs.push(args.map(String));
            await win.eval(scripts[1]);
            const projected = JSON.parse(JSON.stringify(completions));
            if (name.startsWith('repl_error')) { assert(projected.at(-1)[0].stack.length > 0, name + ' native stack'); projected.at(-1)[0].stack = '<native stack>'; }
            equal(projected, output.completions, name + ' completions'); equal(logs, output.logs, name + ' diagnostics');
        }
    } finally { frame.remove(); }
}

async function realm() {
    const frame = document.createElement('iframe');
    const loaded = new Promise(resolve => frame.addEventListener('load', resolve, {once:true}));
    document.body.append(frame); await loaded; return frame;
}
export async function standalone(html) {
    const frame = await realm();
    try {
        const win = frame.contentWindow, registered = [];
        const docAdd = win.document.addEventListener, winAdd = win.addEventListener;
        win.document.addEventListener = function(type,...rest) { registered.push(['document',type]); return docAdd.call(this,type,...rest); };
        win.addEventListener = function(type,...rest) { registered.push(['window',type]); return winAdd.call(this,type,...rest); };
        win.eval(programSources(html)[0]);
        equal(win.localHelper(), 'offline', 'standalone helper'); equal(registered, [], 'finite synchronous registration set');
        equal(typeof win.sendRuntimeMessage, 'undefined', 'no online bridge');
        equal(typeof win.complete, 'undefined', 'no implicit console');
    } finally { frame.remove(); }
}
export async function callbacks(html, inputText, expectedText) {
    const input = JSON.parse(inputText), expected = JSON.parse(expectedText), events = [];
    const frame = await realm();
    try {
        const win = frame.contentWindow;
        let release, started;
        const gate = new Promise(resolve => release = resolve), ready = new Promise(resolve => started = resolve);
        win.__completionCallbacks = input.callbacks === 'none' ? [] : [async ok => {
            events.push(['first',ok]);
            if (input.callbacks === 'reject') throw new Error('callback failure');
            if (input.callbacks === 'gated') { started(); await gate; events.push(['settled',ok]); }
        }, async ok => events.push(['second',ok])];
        win.console.error = message => events.push(['diagnostic',message]);
        win.complete = async (error,value) => { if(error) assert(error.stack.length > 0,'native stack'); events.push(['complete',error ? error.message : null,value ?? null]); };
        const scripts = programSources(html);
        const operation = win.eval(scripts[1]);
        if (input.callbacks === 'gated') { await ready; equal(events,[['first',true],['second',true]],'completion still pending'); release(); }
        await operation; equal(events,expected.events,'ordered completion observations');
    } finally { frame.remove(); }
}

export async function navigation(html, inputText, expectedText) {
    const input = JSON.parse(inputText), expected = JSON.parse(expectedText), events = [];
    const frame = await realm();
    const original = window.postMessage;
    try {
        const win = frame.contentWindow, doc = win.document;
        const base = doc.createElement('base'); base.href = 'https://example.org/a'; doc.head.append(base);
        win.eval(programSources(html)[0]);
        let target;
        if(input.kind === 'form') { target = doc.createElement('form'); target.action = input.href; }
        else if(input.kind === 'svg') { const svg = doc.createElementNS('http://www.w3.org/2000/svg','svg'); const anchor = doc.createElementNS(svg.namespaceURI,'a'); anchor.setAttribute('href',input.href); target = doc.createElementNS(svg.namespaceURI,'text'); anchor.append(target); svg.append(anchor); doc.body.append(svg); }
        else if(input.kind === 'nonlink') target = doc.createElement('span');
        else { const anchor = doc.createElement('a'); if(input.href) anchor.href = input.kind === 'resolved_hash' ? '#part' : input.href; target = doc.createElement('span'); anchor.append(target); doc.body.append(anchor); }
        if(!target.isConnected) doc.body.append(target);
        const event = new win.Event(input.kind === 'form' ? 'submit' : 'click',{bubbles:true,cancelable:true});
        const prevent = event.preventDefault, stop = event.stopPropagation;
        event.preventDefault = function() { events.push(['prevent']); return prevent.call(this); };
        event.stopPropagation = function() { events.push(['stop']); return stop.call(this); };
        window.postMessage = function(data,destination,...rest) { events.push(['post',data,destination]); return original.call(this,data,destination,...rest); };
        target.dispatchEvent(event);
        equal(events,expected.events,'native resolved navigation '+input.kind);
        equal(event.defaultPrevented,expected.events.length > 0,'default action decision '+input.kind);
    } finally { window.postMessage = original; frame.remove(); }
}
