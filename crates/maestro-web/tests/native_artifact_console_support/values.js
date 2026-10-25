// Construct native operands that JSON alone cannot carry.
export function operand(kind, value) {
  if (kind === 'negative-zero') return -0;
  if (kind === 'template') return structuredClone({
    _$litType$: value.litType,
    strings: Object.assign(value.strings, {raw: value.raw}),
    values: value.values,
  });
  if (!value || typeof value !== 'object' || !value.nativeType) return value;
  switch (value.nativeType) {
    case 'Undefined': return undefined;
    case 'Number': return Number(value.value);
    case 'BigInt': return BigInt(value.value);
    case 'Date': return new Date(value.value);
    case 'RegExp': return new RegExp(value.value, value.flags);
    case 'Map': return new Map(value.entries);
    case 'Set': return new Set(value.items);
    case 'Uint8Array': return new Uint8Array(value.items);
    case 'ArrayBuffer': return new ArrayBuffer(value.bytes);
    case 'DataView': return new DataView(new ArrayBuffer(value.bytes));
    case 'Array': {
      if (value.self) { const array = []; array.push(array); return array; }
      return Array(value.copies).fill(value.shared);
    }
    default: throw Error('unknown fixture native type');
  }
}
// Date text follows the actual browser's locale and timezone.
export function nativeString(value) { return String(value); }
