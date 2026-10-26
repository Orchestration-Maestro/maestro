// Native fault observations are scoped to one overlay transaction.
let errors;
function capture(event) {
  errors.push(event.message);
  event.preventDefault();
}
export function beginErrors() {
  if (errors) throw new Error('unfinished error observation');
  errors = [];
  window.addEventListener('error', capture);
}
export function finishErrors(expected) {
  window.removeEventListener('error', capture);
  const actual = errors;
  errors = undefined;
  if (actual.length !== expected) throw new Error('wrong native error count: ' + actual.length);
}

export function pressKey(key) { window.dispatchEvent(new KeyboardEvent('keydown', {key})); }

let keyObservation;
export function beginKeys() {
  if (keyObservation) throw new Error('unfinished key observation');
  const add = window.addEventListener, remove = window.removeEventListener;
  const live = new Set();
  window.addEventListener = function(name, callback, ...rest) {
    if (name === 'keydown') live.add(callback);
    return add.call(this, name, callback, ...rest);
  };
  window.removeEventListener = function(name, callback, ...rest) {
    if (name === 'keydown' && !live.delete(callback)) throw new Error('wrong key callback removed');
    return remove.call(this, name, callback, ...rest);
  };
  keyObservation = () => {
    window.addEventListener = add;
    window.removeEventListener = remove;
    if (live.size) throw new Error('key callback retained after close');
  };
}
export function finishKeys() {
  const finish = keyObservation;
  keyObservation = undefined;
  finish();
}

export function refuseCreation() {
  const create = document.createElement;
  document.createElement = function(tag, ...rest) {
    if (tag === 'attachment-overlay') throw new DOMException('controlled overlay creation', 'InvalidStateError');
    return create.call(this, tag, ...rest);
  };
  return () => { document.createElement = create; };
}
