const prototype = EventTarget.prototype;
const add = prototype.addEventListener, remove = prototype.removeEventListener;
const added = [], removed = [];
let registrations = 0;
const disclosure = target => target instanceof Element && target.matches('.expandable,.tool-item:has(.tool-params-hint)');
prototype.addEventListener = function (name, callback, ...rest) {
  if (name === 'click' && disclosure(this)) {
    if (++registrations === 2) throw Error('registration marker');
    added.push([this, callback]);
  }
  return add.call(this, name, callback, ...rest);
};
prototype.removeEventListener = function (name, callback, ...rest) {
  if (name === 'click' && disclosure(this)) {
    removed.push([this, callback]);
    if (failedRemoval) throw Error('removal marker');
  }
  return remove.call(this, name, callback, ...rest);
};
return {stop() {
  prototype.addEventListener = add;
  prototype.removeEventListener = remove;
  return {added, removed};
}};
