() => {
  const complete = window.complete;
  const events = [];
  let errors = 0;
  window.events = events;
  window.complete = function (...args) {
    const script = document.currentScript;
    events.push({
      completion: true,
      ready: document.readyState,
      parent: script.parentElement.tagName,
      previous: script.previousElementSibling?.tagName || null,
    });
    return complete.apply(this, args);
  };
  // Counted syntax errors are handled here, so the page reports none of its own.
  window.addEventListener('error', event => {
    if (event.error?.name === 'SyntaxError') errors += 1;
    event.preventDefault();
  });
  document.addEventListener('DOMContentLoaded', () => events.push('dom-ready'));
  window.addEventListener('load', () => window.parent.postMessage({
    type: 'console',
    sandboxId: window.sandboxId,
    text: JSON.stringify({
      events,
      errors,
      completions: events.filter(event => event && event.completion).length,
      body: [...(document.body?.children || [])].map(node => node.tagName),
    }),
  }, '*'));
}
