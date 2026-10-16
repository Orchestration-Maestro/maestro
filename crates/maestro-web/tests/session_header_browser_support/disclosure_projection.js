const prompt = value.querySelector('.system-prompt');
const text = (root, selector) => root.querySelector(selector)?.textContent ?? null;
return JSON.stringify({
  prompt: prompt ? {
    expandable: prompt.classList.contains('expandable'),
    heading: text(prompt, '.system-prompt-header'),
    preview: text(prompt, '.system-prompt-preview'),
    hint: text(prompt, '.system-prompt-expand-hint'),
    full: text(prompt, '.system-prompt-full'),
  } : null,
  tools: [...value.querySelectorAll('.tool-item')].map(tool => ({
    name: text(tool, '.tool-item-name'),
    description: text(tool, '.tool-item-desc'),
    expandable: !!tool.querySelector('.tool-params-hint'),
    hint: tool.querySelector('.tool-params-hint') ? JSON.parse(getComputedStyle(tool.querySelector('.tool-params-hint'), '::after').content) : null,
    parameters: [...tool.querySelectorAll('.tool-param')].map(parameter => ({
      name: text(parameter, '.tool-param-name'),
      type: text(parameter, '.tool-param-type'),
      required: !!parameter.querySelector('.tool-param-required'),
      description: text(parameter, '.tool-param-desc'),
    })),
  })),
});
