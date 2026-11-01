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

// Counters live on supplied native values, not on the production console.
export function probe(count) {
  let visits = 0, reads = 0;
  return {
    scalars: Array.from({length: count}, (_, i) => ({toString() { reads++; return String(i); }})),
    iterable: {[Symbol.iterator]: function* () { for (let i = 0; i < count; i++) { visits++; yield 'x'; } }},
    visits: () => visits,
    reads: () => reads,
  };
}
export function stats(probe) { return [probe.visits(), probe.reads()]; }
// A deliberately wrong whole-input rescan, retained as a discriminating control.
export function rescan(count) {
  let visits = 0;
  for (let i = 0; i < count; i++) for (let j = 0; j < count; j++) visits++;
  return visits;
}
// A deliberately wrong global identity filter suppresses completed shared occurrences.
export function deduplicated(value) {
  const seen = new WeakSet();
  function visit(value) {
    if (value == null) return '';
    if (typeof value === 'object' && value[Symbol.iterator]) {
      if (seen.has(value)) return '';
      seen.add(value);
      return [...value].map(visit).join('');
    }
    return String(value);
  }
  return visit(value);
}
