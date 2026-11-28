// Cloud saves (an option of the browser version's start page, off unless the player turns it on; web/opensgolf.js
// sgCloud): a copy of the player's saved games in an R2 bucket, under a sync code their browser made. The code is the
// only key: whoever has it can read and replace those saves, and nothing else about the player is kept (no account, no
// address, no cookies). Typing the code into another browser, the Home Screen app or another device brings them back.
//
// * GET /cloud/<code>: the files kept for that code, [{name, size, at}] (`at` the time the player's browser wrote the
//   file, in ms).
// * GET /cloud/<code>/<name>: a file's bytes. PUT (the bytes, ?at=<ms>) keeps or replaces one; DELETE takes it away.
//
// <name> is the file's path in the save folder, URL-encoded: saved games, championship courses and pros, settings.json,
// the Top 10 table and the saves' dates (cloudName). At most CLOUD_FILES files and CLOUD_FILE_MAX bytes each a code.
//
// Copyright the OpenSGolf contributors. GPL-2.0-or-later.

/** Files kept a code at most. */
export const CLOUD_FILES = 60;
/** The largest file kept. */
export const CLOUD_FILE_MAX = 2 * 1024 * 1024;

/** A sync code: 4 groups of 5 of Crockford's base-32 digits (100 random bits), as the page makes them. */
export function isCloudCode(s) {
  return typeof s === "string" && /^[0-9A-HJKMNP-TV-Z]{5}(-[0-9A-HJKMNP-TV-Z]{5}){3}$/.test(s);
}

/** Whether a path in the save folder is one cloud saves keep (any capitals, as the game's own lookups). */
export function cloudName(name) {
  if (typeof name !== "string" || name.length > 140 || /[\\\u0000-\u001f]|\/\.\.?\//.test(name)) return false;
  return /^saved games\/[^/]{1,100}\.sgs$/i.test(name) || /^saved games\/saves\.json$/i.test(name)
    || /^championship\/[^/]{1,100}\.(sgch|pro)$/i.test(name) || /^(settings\.json|top10\.sve)$/i.test(name);
}

const key = (code, name) => `saves/${code}/${name.toLowerCase()}`;

/** The files kept for a code. */
export async function cloudList(bucket, code) {
  const files = [];
  let cursor;
  do {
    const r = await bucket.list({ prefix: `saves/${code}/`, cursor, include: ["customMetadata"] });
    for (const o of r.objects) {
      const m = o.customMetadata || {};
      files.push({ name: m.path || o.key.substring(`saves/${code}/`.length), size: o.size, at: Number(m.at) || 0 });
    }
    cursor = r.truncated ? r.cursor : undefined;
  } while (cursor);
  return files;
}

/** Keeps a file: true, or a string saying why not. */
export async function cloudPut(bucket, code, name, bytes, at) {
  if (bytes.length === 0 || bytes.length > CLOUD_FILE_MAX) return "bad file";
  const k = key(code, name);
  // a new file counts towards the code's files (a file replaced does not, and costs no listing)
  if (!(await bucket.head(k)) && (await cloudList(bucket, code)).length >= CLOUD_FILES) return "too many files";
  await bucket.put(k, bytes, { customMetadata: { path: name, at: String(at) } });
  return true;
}

/** Answers /cloud/... requests; null for any other path. */
export async function cloudRequest(request, url, bucket, json) {
  const m = url.pathname.match(/^\/cloud\/([^/]+)(?:\/(.+))?$/);
  if (!m) return null;
  const noStore = { "Cache-Control": "no-store" };
  if (!bucket) return json({ ok: false, error: "no cloud saves here" }, 404, noStore);
  const code = m[1];
  if (!isCloudCode(code)) return json({ ok: false, error: "bad sync code" }, 400, noStore);
  if (m[2] === undefined) {
    if (request.method !== "GET") return json({ ok: false, error: "not found" }, 404, noStore);
    return json({ ok: true, files: await cloudList(bucket, code) }, 200, noStore);
  }
  let name;
  try {
    name = decodeURIComponent(m[2]);
  } catch {
    name = "";
  }
  if (!cloudName(name)) return json({ ok: false, error: "not a file cloud saves keep" }, 400, noStore);
  if (request.method === "GET") {
    const o = await bucket.get(key(code, name));
    if (!o) return json({ ok: false, error: "no such file" }, 404, noStore);
    return new Response(o.body, { headers: {
      "Content-Type": "application/octet-stream", "Access-Control-Allow-Origin": "*",
      "X-Saved-At": (o.customMetadata && o.customMetadata.at) || "0", "Access-Control-Expose-Headers": "X-Saved-At", ...noStore,
    } });
  }
  if (request.method === "PUT") {
    const len = Number(request.headers.get("Content-Length") || 0);
    if (len > CLOUD_FILE_MAX) return json({ ok: false, error: "bad file" }, 413, noStore);
    const at = url.searchParams.has("at") ? Number(url.searchParams.get("at")) : NaN;
    if (!Number.isFinite(at) || at < 0) return json({ ok: false, error: "bad time" }, 400, noStore);
    const r = await cloudPut(bucket, code, name, new Uint8Array(await request.arrayBuffer()), Math.floor(at));
    return r === true ? json({ ok: true }, 200, noStore) : json({ ok: false, error: r }, 400, noStore);
  }
  if (request.method === "DELETE") {
    await bucket.delete(key(code, name));
    return json({ ok: true }, 200, noStore);
  }
  return json({ ok: false, error: "not found" }, 404, noStore);
}
