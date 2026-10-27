// Controlled response completion and title-write settlement for the chat example.
export function holdResponse(body) {
  const original = window.fetch;
  let controller, started, aborted = false;
  const ready = new Promise(resolve => { started = resolve; });
  const split = body.indexOf('event: content_block_stop');
  const encoder = new TextEncoder();
  window.fetch = async (input, init) => {
    const request = new Request(input, init);
    request.signal.addEventListener('abort', () => {
      aborted = true;
      controller.error(new DOMException('Aborted', 'AbortError'));
    }, {once: true});
    const stream = new ReadableStream({start(value) {
      controller = value;
      controller.enqueue(encoder.encode(body.slice(0, split)));
      started();
    }});
    const response = new Response(stream, {headers: {'content-type': 'text/event-stream'}});
    Object.defineProperty(response, 'url', {value: request.url});
    return response;
  };
  return {ready, get aborted() { return aborted; }, finish() {
    controller.enqueue(encoder.encode(body.slice(split)));
    controller.close();
  }, restore() { window.fetch = original; }};
}

// Keep the last title write pending after the browser stores it.
export function holdTitleWrite(title) {
  const original = IDBObjectStore.prototype.put;
  let entered, release;
  const ready = new Promise(resolve => { entered = resolve; });
  IDBObjectStore.prototype.put = function(value, ...args) {
    const request = original.call(this, value, ...args);
    if (this.name === 'sessions' && value.title === title) {
      IDBObjectStore.prototype.put = original;
      request.addEventListener('success', event => {
        event.stopImmediatePropagation();
        release = () => request.dispatchEvent(new Event('success'));
        entered();
      }, {once: true});
    }
    return request;
  };
  return {ready, finish() { release(); }, restore() { IDBObjectStore.prototype.put = original; }};
}
