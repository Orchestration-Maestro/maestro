const root = value;
const wrapper = root.parentElement;
const slots = [...root.querySelectorAll('.test-slot')];
const elements = [...slots, wrapper];
const styles = elements.map(e => e.getAttribute('style'));
const set = Element.prototype.setAttribute;
const remove = Element.prototype.removeAttribute;
const rect = Element.prototype.getBoundingClientRect;
const scroll = ['scrollLeft', 'scrollTop'].map(k => Object.getOwnPropertyDescriptor(Element.prototype, k));
let fired = false, cleanupFired = false;
const restored = [], offsets = [];
function attribute(e, name, text, operation) {
  const i = elements.indexOf(e);
  const restoring = name === 'style' && i >= 0 && text === styles[i];
  if (restoring) {
    restored.push(i);
    if (cleanup && !cleanupFired && i === 0) {
      cleanupFired = true;
      throw new Error('cleanup marker');
    }
  }
  operation();
  if (!fired && name === 'style' &&
      ((kind.startsWith('slot') && i === Number(kind.slice(4))) ||
       (kind === 'publish' && e === wrapper && text.includes('visibility:visible')))) {
    fired = true;
    throw new Error('operation marker');
  }
}
Element.prototype.setAttribute = function(name, text) {
  return attribute(this, name, text, () => set.call(this, name, text));
};
Element.prototype.removeAttribute = function(name) {
  return attribute(this, name, null, () => remove.call(this, name));
};
Element.prototype.getBoundingClientRect = function() {
  const result = rect.call(this);
  if (!fired && kind === 'readback' && this === wrapper) {
    fired = true;
    throw new Error('operation marker');
  }
  return result;
};
['scrollLeft', 'scrollTop'].forEach((key, i) => {
  Object.defineProperty(wrapper, key, {
    configurable: true,
    get() { return scroll[i].get.call(this); },
    set(v) { offsets.push(key); scroll[i].set.call(this, v); }
  });
});
return { restored, offsets, stop() {
  Element.prototype.setAttribute = set;
  Element.prototype.removeAttribute = remove;
  Element.prototype.getBoundingClientRect = rect;
  delete wrapper.scrollLeft; delete wrapper.scrollTop;
  return { fired, cleanupFired, restored, offsets };
}};
