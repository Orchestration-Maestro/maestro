const wrapper = value.parentElement;
const frame = wrapper.parentElement;
const slots = [...value.querySelectorAll('.test-slot')];
const selection = window.getSelection();
return {
  root: value, wrapper, frame, slots,
  styles: [...slots, wrapper, frame].map(e => e.getAttribute('style')),
  left: wrapper.scrollLeft, top: wrapper.scrollTop,
  anchor: selection.anchorNode, anchorOffset: selection.anchorOffset,
  focus: selection.focusNode, focusOffset: selection.focusOffset
};
