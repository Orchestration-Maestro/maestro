const expected = JSON.parse(globalThis.exportTypography);
const document = value.ownerDocument;
const target = value.querySelector('.markdown-content p');
const reference = document.createElement('div');
reference.textContent = target.firstChild.textContent;
reference.style.cssText = 'position:fixed;visibility:hidden;white-space:pre';
reference.style.fontFamily = [JSON.stringify(expected.family), ...expected.fallbacks].join(',');
reference.style.fontSize = `${expected.size}px`;
reference.style.lineHeight = expected.lineHeight;
reference.style.fontWeight = expected.weight;
document.body.append(reference);
try {
    const measure = element => {
        const range = document.createRange();
        range.selectNodeContents(element.firstChild);
        return range.getBoundingClientRect();
    };
    const actual = measure(target), wanted = measure(reference);
    const selected = getComputedStyle(target), independent = getComputedStyle(reference);
    return selected.fontFamily === independent.fontFamily &&
        selected.fontSize === independent.fontSize &&
        Math.abs(actual.width - wanted.width) < 0.01 &&
        Math.abs(actual.height - wanted.height) < 0.01 &&
        Math.abs(target.getBoundingClientRect().height - reference.getBoundingClientRect().height) < 0.01 &&
        wanted.width > 0 && wanted.height > 0;
} finally {
    reference.remove();
}
