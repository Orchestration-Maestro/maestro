const prototype = Element.prototype;
const attr = prototype.setAttribute, bounds = prototype.getBoundingClientRect;
let fired = false;
const candidate = target => target !== value && (target.matches('.maestro-session-viewer-header') || target.querySelector('.maestro-session-viewer-header'));
prototype.setAttribute = function (name, text) {
  if (!fired && ((kind === 'mount' && name === 'class' && text === 'system-prompt expandable') || (kind === 'publish' && name === 'style' && candidate(this) && !text.includes('visibility:hidden') && text.includes('height:')))) {
    fired = true;
    throw Error('candidate marker');
  }
  return attr.call(this, name, text);
};
prototype.getBoundingClientRect = function () {
  if (!fired && kind === 'read' && candidate(this)) {
    fired = true;
    throw Error('read marker');
  }
  return bounds.call(this);
};
return () => {
  prototype.setAttribute = attr;
  prototype.getBoundingClientRect = bounds;
  if (!fired) throw Error('candidate fault not reached');
};
