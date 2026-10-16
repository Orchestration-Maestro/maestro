const wrapper = value.parentElement;
const slots = [...value.querySelectorAll('.test-slot')];
const selection = window.getSelection();
return {
  root: value, wrapper, slots,
  styles: [...slots, wrapper].map(e => e.getAttribute('style')),
  left: wrapper.scrollLeft, top: wrapper.scrollTop,
  anchor: selection.anchorNode, anchorOffset: selection.anchorOffset,
  focus: selection.focusNode, focusOffset: selection.focusOffset
};
