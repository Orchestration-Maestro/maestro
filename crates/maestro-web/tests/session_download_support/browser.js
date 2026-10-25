
let active;
export function begin(mode) {
    if (active) throw new Error('unfinished browser transaction');
    const create = document.createElement;
    const append = document.body.appendChild;
    const body = document.body;
    const createUrl = URL.createObjectURL;
    const revoke = URL.revokeObjectURL;
    const state = { mode, blobs: [], anchors: [], live: new Set(), events: [] };
    URL.createObjectURL = function(blob) {
        const url = createUrl.call(this, blob);
        state.blobs.push(blob); state.live.add(url); state.events.push('create-url');
        return url;
    };
    URL.revokeObjectURL = function(url) {
        if (!state.live.delete(url)) throw new Error('wrong URL revoked');
        state.events.push('revoke-url'); return revoke.call(this, url);
    };
    document.createElement = function(tag, ...rest) {
        if (tag === 'a' && mode === 'create') throw new DOMException('controlled create', 'InvalidStateError');
        const element = create.call(this, tag, ...rest);
        if (tag === 'a') {
            state.anchors.push(element);
            element.addEventListener('click', event => {
                event.preventDefault();
                if (!state.live.has(element.href) || !element.isConnected) throw new Error('click without live resources');
                state.events.push('click');
                if (mode === 'removed') element.remove();
            });
        }
        return element;
    };
    body.appendChild = function(child) {
        if (mode === 'append') throw new DOMException('controlled append', 'HierarchyRequestError');
        state.events.push('append'); return append.call(this, child);
    };
    if (mode === 'body') Object.defineProperty(document, 'body', { configurable: true, get: () => null });
    state.restore = () => {
        URL.createObjectURL = createUrl; URL.revokeObjectURL = revoke;
        document.createElement = create; body.appendChild = append;
        if (mode === 'body') delete document.body;
        active = undefined;
    };
    active = state;
}
export async function finish(bytes, mime, name, success) {
    const state = active;
    try {
        if (state.blobs.length !== 1 || state.live.size !== 0) throw new Error('Blob/URL ownership');
        const blob = state.blobs[0];
        const actual = new Uint8Array(await blob.arrayBuffer());
        if (actual.length !== bytes.length || actual.some((b, i) => b !== bytes[i]) || blob.type !== mime) throw new Error('original bytes/MIME');
        for (const anchor of state.anchors) {
            if (anchor.download !== name || anchor.isConnected) throw new Error('literal name/anchor cleanup');
        }
        const expected = success ? ['create-url', 'append', 'click', 'revoke-url'] : ['create-url', 'revoke-url'];
        if (JSON.stringify(state.events) !== JSON.stringify(expected)) throw new Error('effect order: ' + state.events.join(','));
        console.log('PASS native download ' + state.mode + ': ' + state.events.join(' -> '));
    } finally { state.restore(); }
}
// Observe an activation that must leave the platform untouched.
export async function finishNone() {
    const state = active;
    try {
        if (state.blobs.length || state.anchors.length || state.events.length) throw new Error('unexpected download effects: ' + state.events.join(',') + ' (' + state.blobs.length + ' Blobs)');
        console.log('PASS native download ' + state.mode + ': none');
    } finally { state.restore(); }
}
