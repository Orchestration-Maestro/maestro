const create = Document.prototype.createElement;
const replace = Node.prototype.replaceChild;
const append = Node.prototype.appendChild;
const set = Element.prototype.setAttribute;
const bounds = Element.prototype.getBoundingClientRect;
let fired = false;
function fail() { fired = true; throw new Error('native phase marker'); }
Document.prototype.createElement = function(tag, ...args) {
  if (!fired && phase === 'create' && tag === 'pre') fail();
  return create.call(this, tag, ...args);
};
Node.prototype.replaceChild = function(child, old) {
  if (!fired && phase === 'replacement' && child.tagName === 'PRE') fail();
  return replace.call(this, child, old);
};
Node.prototype.appendChild = function(child) {
  if (!fired && phase === 'append' && this.nodeName === 'BODY' && child.querySelector?.('pre')) fail();
  return append.call(this, child);
};
Element.prototype.getBoundingClientRect = function() {
  if (!fired && phase === 'placement' && this.id === 'canvas') fail();
  return bounds.call(this);
};
Element.prototype.setAttribute = function(name, value) {
  if (!fired && name === 'style' &&
      ((phase === 'content' && this.tagName === 'PRE') ||
       (phase === 'visibility' && this.tagName === 'DIV' && value.includes('position:fixed')))) fail();
  return set.call(this, name, value);
};
return { stop() {
  Document.prototype.createElement = create;
  Node.prototype.replaceChild = replace;
  Node.prototype.appendChild = append;
  Element.prototype.setAttribute = set;
  Element.prototype.getBoundingClientRect = bounds;
  return fired;
} };
