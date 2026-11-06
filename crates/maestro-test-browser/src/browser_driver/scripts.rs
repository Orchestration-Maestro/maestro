//! Browser-native observations and host binding installation.

/// Promise-returning host functions installed in every default execution context.
pub(super) const BINDINGS: &str = r"(() => {
  if (globalThis.__maestroCallbacks) return;
  const callbacks = new Map(); let next = 0;
  globalThis.__maestroCallbacks = callbacks;
  globalThis.__maestroReply = (id, value, error) => {
    const pending = callbacks.get(id); if (!pending) return;
    callbacks.delete(id);
    if (error) pending.reject(new Error(error)); else pending.resolve(value);
  };
  for (const name of ['inspectSandbox','sandboxDriverPhase','driverPress','driverAria','driverPointer','readingPaint']) {
    globalThis[name] = (...args) => new Promise((resolve, reject) => {
      const id = ++next; callbacks.set(id, {resolve,reject});
      __maestroDriver(JSON.stringify({id,name,args}));
    });
  }
  addEventListener('message', event => {
    if (event.data?.type === 'pointer-done') void sandboxDriverPhase('pointer-done received');
  });
  addEventListener('click', () => { void sandboxDriverPhase('click dispatched'); }, true);
})()";

/// Find the mounted native frame without scanning any document twice.
pub(super) const LOOKUP: &str = r"id => {
  const documents = [document];
  for (const doc of documents) for (const frame of doc.querySelectorAll('iframe')) {
    if (frame.parentElement.id === id) return frame;
    if (frame.contentDocument) documents.push(frame.contentDocument);
  }
  return null;
}";

/// Observe the selected sandbox's retained document and program settlement.
pub(super) const STATE: &str = r"(async () => {
  if (document.readyState !== 'complete') await new Promise(resolve => addEventListener('load', resolve, {once:true}));
  if (window.S?.started) await window.done;
  return {...window.S, head:[...document.head.children].map(e=>e.localName).join(','),
    body:[...document.body.children].map(e=>e.localName).join(','), scaffold:document.querySelectorAll('svg,template').length,
    metaParent:document.querySelector('meta[http-equiv]')?.parentElement.localName,
    origin:self.origin, href:location.href};
})()";

/// Return the actual element, scrolled into its renderer's visible viewport.
pub(super) const TARGET: &str = r"(() => {
  const node = document.querySelector('#target');
  if (!node) throw Error('mounted click target missing');
  node.scrollIntoView({block:'center',inline:'center',behavior:'instant'});
  return node;
})()";
