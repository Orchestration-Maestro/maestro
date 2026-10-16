const selection = window.getSelection();
return value.root.parentElement === value.wrapper && value.wrapper.isConnected &&
  [...value.slots, value.wrapper].every((e, i) => e.getAttribute('style') === value.styles[i]) &&
  value.wrapper.scrollLeft === value.left && value.wrapper.scrollTop === value.top &&
  selection.anchorNode === value.anchor && selection.anchorOffset === value.anchorOffset &&
  selection.focusNode === value.focus && selection.focusOffset === value.focusOffset;
