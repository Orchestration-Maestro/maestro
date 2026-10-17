const root = value;
const wrapper = root.parentElement;
const slots = [...root.querySelectorAll('.test-slot')];
const frame = wrapper.parentElement;
const elements = [...slots, wrapper, frame];
const styles = elements.map(e => e.getAttribute('style'));
const set = Element.prototype.setAttribute;
const remove = Element.prototype.removeAttribute;
const rectDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, 'getBoundingClientRect');
const rect = rectDescriptor.value;
const scroll = ['scrollLeft', 'scrollTop'].map(k => Object.getOwnPropertyDescriptor(Element.prototype, k));
let fired = false, cleanupFired = false;
const restored = [], offsets = [];
function attribute(e, name, text, operation) {
  const i = elements.indexOf(e);
  const restoring = name === 'style' && i >= 0 && text === styles[i];
  if (restoring) {
    restored.push(i);
    if (cleanup && !cleanupFired && i === 0 && !kind.includes('axis')) {
      cleanupFired = true;
      throw new Error('cleanup marker');
    }
  }
  const target = kind.replace(/^publish-/, '').replace(/-(before|after)$/, '');
  const matches = name === 'style' &&
    ((target.startsWith('slot') && i === Number(target.slice(4))) ||
     (target === 'frame' && e === frame) ||
     (target === 'viewport' && e === wrapper) ||
     (kind === 'publish' && e === frame && text.includes('visibility:visible')));
  if (!fired && matches && kind.endsWith('-before')) {
    fired = true;
    throw new Error('operation marker');
  }
  operation();
  if (!fired && matches) {
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
  if (!fired && kind.startsWith('readback') && this === frame) {
    fired = true;
    throw new Error('operation marker');
  }
  return result;
};
if (kind === 'readback-getter') {
  Object.defineProperty(Element.prototype, 'getBoundingClientRect', {
    configurable: true,
    get() {
      if (!fired && this === frame) {
        fired = true;
        throw new Error('operation marker');
      }
      return rect;
    }
  });
}
['scrollLeft', 'scrollTop'].forEach((key, i) => {
  Object.defineProperty(wrapper, key, {
    configurable: true,
    get() { return scroll[i].get.call(this); },
    set(v) {
      offsets.push(key);
      if (fired && cleanup && !cleanupFired && kind.includes('axis') && i === 0) {
        cleanupFired = true;
        throw new Error('cleanup marker');
      }
      scroll[i].set.call(this, v);
    }
  });
});
return { restored, offsets, stop() {
  Element.prototype.setAttribute = set;
  Element.prototype.removeAttribute = remove;
  Object.defineProperty(Element.prototype, 'getBoundingClientRect', rectDescriptor);
  delete wrapper.scrollLeft; delete wrapper.scrollTop;
  return { fired, cleanupFired, restored, offsets };
}};
