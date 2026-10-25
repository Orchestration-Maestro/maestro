
let active;
export function begin(mode, text) {
    if (active) throw new Error('unfinished clipboard transaction');
    const descriptor = Object.getOwnPropertyDescriptor(navigator, 'clipboard');
    const create = document.createElement, body = document.body, append = body.appendChild;
    const execDescriptor = Object.getOwnPropertyDescriptor(document, 'execCommand');
    const timeout = window.setTimeout;
    const state = {mode, text, events: [], nodes: [], timers:0};
    window.setTimeout=function(...args){state.timers++;return timeout.apply(this,args)};
    const writeText = value => {
        state.events.push('api');
        if (value !== text) throw new Error('wrong API text');
        if (mode === 'call') throw new DOMException('controlled call');
        if (mode === 'reject') return Promise.reject(new DOMException('controlled rejection'));
        if (mode === 'pending') return new Promise(resolve=>{state.resolve=resolve});
        return Promise.resolve();
    };
    Object.defineProperty(navigator, 'clipboard', {configurable:true, get() {
        if (mode === 'getter') throw new DOMException('controlled getter');
        if (mode === 'missing') return undefined;
        if (mode === 'method') return {};
        return ['api','call','reject','pending'].includes(mode) ? {writeText} : undefined;
    }});
    document.createElement = function(tag, ...rest) {
        if (tag === 'textarea' && mode === 'create') throw new DOMException('controlled create');
        const node = create.call(this, tag, ...rest);
        if (tag === 'textarea') { state.nodes.push(node); state.events.push('create'); }
        return node;
    };
    body.appendChild = function(node) {
        if (node.localName === 'textarea') {
            if (mode === 'append') throw new DOMException('controlled append');
            state.events.push('append');
        }
        return append.call(this, node);
    };
    if (mode === 'body') Object.defineProperty(document, 'body', {configurable:true, get:()=>null});
    Object.defineProperty(document, 'execCommand', {configurable:true, value(command) {
        const node = state.nodes.at(-1);
        if (command !== 'copy' || document.activeElement !== node || node.value !== text ||
            node.selectionStart !== 0 || node.selectionEnd !== text.length ||
            node.style.position !== 'fixed' || node.style.opacity !== '0') throw new Error('native selection operands');
        state.events.push('exec');
        if (mode === 'exec') throw new DOMException('controlled exec');
        if (mode === 'removed') node.remove();
        return mode !== 'false';
    }});
    state.restore = () => {
        document.createElement=create; body.appendChild=append; window.setTimeout=timeout;
        if (mode === 'body') delete document.body;
        if (descriptor) Object.defineProperty(navigator,'clipboard',descriptor); else delete navigator.clipboard;
        if (execDescriptor) Object.defineProperty(document,'execCommand',execDescriptor); else delete document.execCommand;
        active=undefined;
    };
    active=state;
}
export function immediate(mode) {
    const api = ['api','call','reject','pending'].includes(mode);
    if (api && active.events[0] !== 'api') throw new Error('API deferred behind local dispatch');
    if (!api && !['create','getter'].includes(mode) && active.nodes.length !== 1) throw new Error('synchronous fallback deferred');
}
export function resolvePending() {active.resolve();}
export function finish(success) {
    const state=active;
    try {
        if (state.nodes.some(node=>node.isConnected)) throw new Error('temporary selection leaked');
        if (['api','pending'].includes(state.mode) && state.nodes.length) throw new Error('fallback after API success');
        const expected = !['false','create','body','append','exec'].includes(state.mode);
        if (success !== expected) throw new Error('wrong native success result');
        if (state.timers !== 0) throw new Error('feedback admitted without live control');
        const expectedEvents = ['api','pending'].includes(state.mode) ? ['api'] :
            state.mode==='create' ? [] : ['body','append'].includes(state.mode) ? ['create'] :
            ['call','reject'].includes(state.mode) ? ['api','create','append','exec'] : ['create','append','exec'];
        if (JSON.stringify(state.events)!==JSON.stringify(expectedEvents)) throw new Error('wrong effect sequence');
        console.log('PASS clipboard '+state.mode+': '+state.events.join(' -> '));
    } finally {state.restore();}
}
