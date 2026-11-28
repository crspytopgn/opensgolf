// A stand-in for an R2 bucket: put, get, head, delete and list (with customMetadata), in memory.
export function memoryBucket() {
  const m = new Map();
  return {
    m,
    async put(k, bytes, o = {}) { m.set(k, { bytes: new Uint8Array(bytes), customMetadata: o.customMetadata || {} }); },
    async get(k) { const v = m.get(k); return v ? { body: v.bytes, customMetadata: v.customMetadata } : null; },
    async head(k) { return m.has(k) ? { key: k } : null; },
    async delete(k) { m.delete(k); },
    async list({ prefix }) {
      const objects = [...m.entries()].filter(([k]) => k.startsWith(prefix))
        .map(([k, v]) => ({ key: k, size: v.bytes.length, customMetadata: v.customMetadata }));
      return { objects, truncated: false };
    },
  };
}
