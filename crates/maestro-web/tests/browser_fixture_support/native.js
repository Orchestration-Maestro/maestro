export function identity(missing, empty, value) { if (missing !== undefined || empty !== null) throw new Error("argument kinds"); return value; }
export async function promise(value) { return value; }
