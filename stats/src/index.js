// OpenSGolf's play-time stats (stats/README.md): a Cloudflare Worker with a D1 database.
//
// * GET /count?e=<event>&v=<version>&s=<run id>&n=<count>&t=<seconds>: the game's play-time count
//   (crates/simgolf/src/playcount.rs). `e` is start/<platform>/<version> once at start, or play/<platform> with `t` the
//   whole seconds played in the run so far, sent every 5 minutes of play and when the game is left or quit (builds before
//   the exact count sent play-15min/<platform> at each 15 minutes instead, still taken). `s` a random id the game makes at
//   each start (or "kept" for play sent late, with no run); `n` the count's number in the run. Nothing else is kept: no
//   address, only the country Cloudflare reads from it. Builds after 0.9.10 add `mc`, `ml` and `mw` to a play count: the
//   seconds of `t` played in a shared club, a Neighbours league and the weekly challenge (MP_PARAMS).
// * Every day (the cron in wrangler.toml), and on POST /admin/import: GoatCounter's counts from older builds, by hour.
// * GET / (password): the stats page; GET /api/stats the same as JSON. Its Multiplayer section reads the multiplayer
//   server's database (src/mp.js), bound as BOARD.
//
// Copyright the OpenSGolf contributors. GPL-2.0-or-later.

import { mpStats } from "./mp.js";

/** Platforms the game names (playcount.rs `platform`). */
export const PLATFORMS = new Set([
  "windows", "macos", "linux", "android", "web", "other",
  "windows-arm64", "macos-arm64", "linux-arm64", "other-arm64",
]);
/** GoatCounter's counter (the old builds' COUNT_URL). */
export const GOATCOUNTER = "https://opensgolf.goatcounter.com";
/** The first day an import with nothing yet imported starts from (before the first play-time count). */
export const FIRST_DAY = "2026-10-01";
/** How far back the daily import looks again (late counts from older builds land in recent hours). */
export const REIMPORT_DAYS = 15;
/** The most play time one run (or one kept count) can report: a week. */
export const MAX_SECONDS = 7 * 24 * 3600;
/** Seconds in one of the older builds' 15-minute marks. */
export const MARK = 900;
/** Old builds' weekly play below this many hours, for [RETIRE_WEEKS] full weeks in a row: the import can be retired. */
export const RETIRE_HOURS = 1;
export const RETIRE_WEEKS = 3;

/** A play count's multiplayer seconds: query parameter and mode (the same names as src/mp.js MODES). */
export const MP_PARAMS = [["mc", "club"], ["ml", "league"], ["mw", "challenge"]];

/** An event's parts, or null when it is not one of the game's. */
export function parseEvent(e) {
  if (typeof e !== "string" || e.length > 64) return null;
  const p = e.split("/");
  if (p[0] === "start" && p.length === 3 && PLATFORMS.has(p[1]) && isVersion(p[2])) {
    return { kind: "start", platform: p[1], version: p[2] };
  }
  if (p[0] === "play-15min" && p.length === 2 && PLATFORMS.has(p[1])) return { kind: "play", platform: p[1] };
  if (p[0] === "play" && p.length === 2 && PLATFORMS.has(p[1])) return { kind: "seconds", platform: p[1] };
  return null;
}

export function isVersion(v) {
  return typeof v === "string" && /^\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(v);
}

/** A run id: 8 to 32 hex digits. */
export function isRunId(s) {
  return typeof s === "string" && /^[0-9a-f]{8,32}$/.test(s);
}

/** 'YYYY-MM-DDTHH' of a Date, in UTC. */
export function hourOf(d) {
  return d.toISOString().slice(0, 13);
}

/** A count's request, checked: what to store, or null to ignore it. */
export function readCount(url, country) {
  const q = url.searchParams;
  const ev = parseEvent(q.get("e"));
  if (!ev) return null;
  const version = ev.kind === "start" ? ev.version : q.get("v");
  if (!isVersion(version)) return null;
  const s = q.get("s") || "";
  const kept = s === "kept";
  if (!kept && !isRunId(s)) return null;
  const n = Number.parseInt(q.get("n") || "0", 10);
  if (!Number.isFinite(n) || n < 0 || n > 100000) return null;
  let t = 0;
  if (ev.kind === "seconds") {
    const tq = q.get("t") || "";
    if (!/^\d{1,7}$/.test(tq)) return null;
    t = Number(tq);
    if (t > MAX_SECONDS) return null;
  }
  // multiplayer seconds: part of `t`, so never more than it; a bad one is left out, the count still taken
  const mp = {};
  if (ev.kind === "seconds" && !kept) {
    for (const [k, mode] of MP_PARAMS) {
      const v = q.get(k) || "";
      if (/^\d{1,7}$/.test(v) && Number(v) > 0 && Number(v) <= t) mp[mode] = Number(v);
    }
  }
  const c = typeof country === "string" && /^[A-Z0-9]{2}$/.test(country) ? country : "";
  return { kind: ev.kind, platform: ev.platform, version, sid: kept ? null : s, n, t, country: c, mp };
}

/** Stores one count. A mark already counted for its run (a repeat) changes nothing; nor does a play time no higher than
 * the run's (`t` only grows within a run: only the increase is added). */
export async function storeCount(db, c, now) {
  const at = now.toISOString();
  const hour = hourOf(now);
  const playRow = (kept) =>
    db.prepare(
      `INSERT INTO play_hourly (hour, platform, version, country, kept, beats) VALUES (?1, ?2, ?3, ?4, ?5, 1)
       ON CONFLICT (hour, platform, version, country, kept) DO UPDATE SET beats = beats + 1`,
    ).bind(hour, c.platform, c.version, c.country, kept);
  const secondsRow = (kept, n) =>
    db.prepare(
      `INSERT INTO seconds_hourly (hour, platform, version, country, kept, seconds) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
       ON CONFLICT (hour, platform, version, country, kept) DO UPDATE SET seconds = seconds + excluded.seconds`,
    ).bind(hour, c.platform, c.version, c.country, kept, n);
  if (c.kind === "seconds" && c.t <= 0) return; // nothing played yet
  if (!c.sid) {
    if (c.kind === "play") await playRow(1).run();
    if (c.kind === "seconds") await secondsRow(1, c.t).run();
    return;
  }
  await db.prepare(
    `INSERT INTO runs (sid, platform, version, country, started_at, last_at, beats) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 0)
     ON CONFLICT (sid) DO NOTHING`,
  ).bind(c.sid, c.platform, c.version, c.country, at).run();
  if (c.kind === "seconds") {
    const had = await db.prepare(`SELECT seconds FROM run_time WHERE sid = ?1`).bind(c.sid).first();
    let added = 0;
    if (!had) {
      const r = await db.prepare(`INSERT INTO run_time (sid, seconds) VALUES (?1, ?2) ON CONFLICT (sid) DO NOTHING`).bind(c.sid, c.t).run();
      if (r.meta && r.meta.changes > 0) added = c.t;
    } else if (c.t > had.seconds) {
      // only from the value just read, so two counts arriving together never add the same seconds twice
      const r = await db.prepare(`UPDATE run_time SET seconds = ?2 WHERE sid = ?1 AND seconds = ?3`).bind(c.sid, c.t, had.seconds).run();
      if (r.meta && r.meta.changes > 0) added = c.t - had.seconds;
    }
    if (added > 0) {
      await secondsRow(0, added).run();
      await db.prepare(`UPDATE runs SET last_at = ?2 WHERE sid = ?1`).bind(c.sid, at).run();
    }
    // the multiplayer seconds never cost the count itself (before schema.sql added their tables, say)
    try {
      for (const [mode, t] of Object.entries(c.mp || {})) await storeMp(db, c, hour, mode, t);
    } catch (e) {
      console.log("multiplayer seconds not stored:", String(e && e.message || e));
    }
    return;
  }
  if (c.kind !== "play") return;
  const r = await db.prepare(`UPDATE runs SET beats = ?2, last_at = ?3 WHERE sid = ?1 AND beats < ?2`)
    .bind(c.sid, Math.max(1, c.n), at).run();
  if (r.meta && r.meta.changes > 0) await playRow(0).run();
}

/** A run's multiplayer seconds in `mode`: as with its play time, the highest seen is kept and only the increase added. */
async function storeMp(db, c, hour, mode, t) {
  const had = await db.prepare(`SELECT seconds FROM run_mp WHERE sid = ?1 AND mode = ?2`).bind(c.sid, mode).first();
  let added = 0;
  if (!had) {
    const r = await db.prepare(`INSERT INTO run_mp (sid, mode, seconds) VALUES (?1, ?2, ?3) ON CONFLICT (sid, mode) DO NOTHING`)
      .bind(c.sid, mode, t).run();
    if (r.meta && r.meta.changes > 0) added = t;
  } else if (t > had.seconds) {
    const r = await db.prepare(`UPDATE run_mp SET seconds = ?3 WHERE sid = ?1 AND mode = ?2 AND seconds = ?4`)
      .bind(c.sid, mode, t, had.seconds).run();
    if (r.meta && r.meta.changes > 0) added = t - had.seconds;
  }
  if (added > 0) {
    await db.prepare(
      `INSERT INTO mp_hourly (hour, platform, version, mode, seconds) VALUES (?1, ?2, ?3, ?4, ?5)
       ON CONFLICT (hour, platform, version, mode) DO UPDATE SET seconds = seconds + excluded.seconds`,
    ).bind(hour, c.platform, c.version, mode, added).run();
  }
}

// ---------------------------------------------------------------- GoatCounter import

/** GoatCounter paths worth keeping: the game's own events (not the page's visits or start-button clicks). */
export function gameGcPath(path) {
  return typeof path === "string" && (path.startsWith("play-15min/") || path.startsWith("start/"));
}

/** Rows [hour, path, count] from one page of GoatCounter's /api/v0/stats/hits, only hours on or after `fromDay`. */
export function gcRows(hits, fromDay) {
  const rows = [];
  for (const h of hits || []) {
    if (!gameGcPath(h.path)) continue;
    for (const s of h.stats || []) {
      if (!s.day || s.day < fromDay || !Array.isArray(s.hourly)) continue;
      s.hourly.forEach((n, i) => {
        if (n > 0) rows.push([`${s.day}T${String(i).padStart(2, "0")}`, h.path, n]);
      });
    }
  }
  return rows;
}

/** Imports GoatCounter's counts from `fromDay` to now; re-imported hours replace what was there. */
/** Waits before each retry of a GoatCounter call that failed (it now and then answers 404 or 5xx for a moment). */
export const GC_RETRY_WAITS_MS = [5e3, 30e3, 120e3];

/** One GoatCounter call, tried again after [GC_RETRY_WAITS_MS] when it fails with anything but a sign-in error. */
export async function gcFetch(url, init, fetcher = fetch, waits = GC_RETRY_WAITS_MS, sleep = (ms) => new Promise((r) => setTimeout(r, ms))) {
  for (let i = 0; ; i++) {
    let res = null, err = null;
    try { res = await fetcher(url, init); } catch (e) { err = e; }
    if (res && (res.ok || res.status === 401 || res.status === 403)) return res;
    if (i >= waits.length) {
      if (res) return res;
      throw err;
    }
    await sleep(waits[i]);
  }
}

export async function importGoatCounter(env, fromDay, now = new Date(), fetcher = fetch, waits = GC_RETRY_WAITS_MS, sleep) {
  const db = env.DB;
  let rows = [];
  try {
    if (!env.GOATCOUNTER_TOKEN) throw new Error("GOATCOUNTER_TOKEN is not set");
    const seen = [];
    for (let page = 0; page < 20; page++) {
      const u = new URL(`${GOATCOUNTER}/api/v0/stats/hits`);
      u.searchParams.set("start", `${fromDay}T00:00:00Z`);
      u.searchParams.set("end", now.toISOString().slice(0, 19) + "Z");
      u.searchParams.set("limit", "100");
      if (seen.length) u.searchParams.set("exclude_paths", seen.join(","));
      const res = await gcFetch(u.toString(), { headers: { Authorization: `Bearer ${env.GOATCOUNTER_TOKEN}` } }, fetcher, waits, sleep);
      if (!res.ok) throw new Error(`GoatCounter answered ${res.status}`);
      const d = await res.json();
      rows = rows.concat(gcRows(d.hits, fromDay));
      for (const h of d.hits || []) seen.push(h.path_id);
      if (!d.more || !(d.hits || []).length) break;
    }
    // one statement per 1,000 rows (D1 limits statements per run and parameters per statement, not a JSON value's size)
    const stmts = [];
    for (let i = 0; i < rows.length; i += 1000) {
      stmts.push(db.prepare(
        `INSERT INTO gc_hourly (hour, path, count)
         SELECT j.value ->> 0, j.value ->> 1, j.value ->> 2 FROM json_each(?1) AS j WHERE true
         ON CONFLICT (hour, path) DO UPDATE SET count = excluded.count`,
      ).bind(JSON.stringify(rows.slice(i, i + 1000))));
    }
    if (stmts.length) await db.batch(stmts);
    const marks = rows.filter((r) => r[1].startsWith("play-15min/")).reduce((a, r) => a + r[2], 0);
    await logImport(db, now, fromDay, rows.length, marks, null);
    return { from: fromDay, rows: rows.length, play_marks: marks };
  } catch (e) {
    await logImport(db, now, fromDay, rows.length, 0, String(e && e.message || e));
    return { from: fromDay, error: String(e && e.message || e) };
  }
}

async function logImport(db, now, fromDay, rows, marks, error) {
  await db.prepare(`INSERT INTO gc_imports (at, start_day, end_day, rows, play_marks, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6)`)
    .bind(now.toISOString(), fromDay, now.toISOString().slice(0, 10), rows, marks, error).run();
}

/** Where the daily import starts: from [FIRST_DAY] when nothing was imported yet, else [REIMPORT_DAYS] back. */
export async function importStart(db, now) {
  const any = await db.prepare(`SELECT 1 AS x FROM gc_hourly LIMIT 1`).first();
  if (!any) return FIRST_DAY;
  return new Date(now.getTime() - REIMPORT_DAYS * 86400e3).toISOString().slice(0, 10);
}

// ---------------------------------------------------------------- stats

/** Session-length buckets, by seconds played in the run (older builds' runs: 900 s for each 15-minute mark reached). */
export const LENGTHS = [
  ["under 15 min", 0, 899], ["15–30 min", 900, 1799], ["30 min–1 h", 1800, 3599], ["1–2 h", 3600, 7199], ["2–4 h", 7200, 14399],
  ["4 h or more", 14400, 1e12],
];

/** The stats, from every source: GoatCounter's and this counter's 15-minute marks (900 s each) and the exact seconds. */
export async function stats(db, now = new Date()) {
  const all = async (sql, ...p) => (await db.prepare(sql).bind(...p).all()).results || [];
  const day30 = new Date(now.getTime() - 30 * 86400e3).toISOString().slice(0, 10);
  const day7 = new Date(now.getTime() - 7 * 86400e3).toISOString().slice(0, 13);
  // this counter's play in seconds, per hour: the older builds' marks and the exact counts together
  const here = `SELECT hour, platform, version, country, beats * ${MARK} AS s FROM play_hourly
    UNION ALL SELECT hour, platform, version, country, seconds FROM seconds_hourly`;
  // each run's play: exact when it sent any, else its marks
  const played = `COALESCE(t.seconds, r.beats * ${MARK})`;

  const daily = await all(`
    SELECT day, SUM(old) AS old, SUM(new) AS new FROM (
      SELECT substr(hour, 1, 10) AS day, SUM(count) * ${MARK} AS old, 0 AS new FROM gc_hourly WHERE path LIKE 'play-15min/%' GROUP BY day
      UNION ALL
      SELECT substr(hour, 1, 10) AS day, 0, SUM(s) FROM (${here}) GROUP BY day
    ) GROUP BY day ORDER BY day`);
  const platforms = await all(`
    SELECT platform, SUM(s) AS s FROM (
      SELECT substr(path, 12) AS platform, SUM(count) * ${MARK} AS s FROM gc_hourly WHERE path LIKE 'play-15min/%' GROUP BY platform
      UNION ALL SELECT platform, SUM(s) FROM (${here}) GROUP BY platform
    ) GROUP BY platform ORDER BY s DESC`);
  // launches by version: GoatCounter's start/<platform>/<version> and this Worker's runs
  const launches = await all(`
    SELECT version, SUM(n) AS launches FROM (
      SELECT substr(path, instr(substr(path, 7), '/') + 7) AS version, SUM(count) AS n FROM gc_hourly WHERE path LIKE 'start/%' GROUP BY version
      UNION ALL SELECT version, COUNT(*) FROM runs GROUP BY version
    ) GROUP BY version`);
  const versionSeconds = await all(`SELECT version, SUM(s) AS s FROM (${here}) GROUP BY version`);
  const hourOfDay = await all(`
    SELECT h, SUM(s) AS s FROM (
      SELECT CAST(substr(hour, 12, 2) AS INTEGER) AS h, SUM(count) * ${MARK} AS s FROM gc_hourly WHERE path LIKE 'play-15min/%' GROUP BY h
      UNION ALL SELECT CAST(substr(hour, 12, 2) AS INTEGER), SUM(s) FROM (${here}) GROUP BY 1
    ) GROUP BY h ORDER BY h`);
  // by whole minutes, so the rows stay few
  const lengths = await all(`
    SELECT (${played} / 60) * 60 AS s, COUNT(*) AS runs FROM runs r LEFT JOIN run_time t ON t.sid = r.sid GROUP BY 1 ORDER BY 1`);
  const countries = await all(`
    SELECT r.country AS country, COUNT(*) AS runs, SUM(${played}) AS s FROM runs r LEFT JOIN run_time t ON t.sid = r.sid
    GROUP BY r.country ORDER BY s DESC, runs DESC LIMIT 25`);
  const recentRuns = await all(`
    SELECT r.platform AS platform, r.version AS version, r.country AS country, r.started_at AS started_at, r.beats AS beats,
      t.seconds AS seconds
    FROM runs r LEFT JOIN run_time t ON t.sid = r.sid WHERE r.started_at >= ?1 ORDER BY r.started_at DESC LIMIT 50`, day7);
  const oldWeekly = await all(`
    SELECT date(substr(hour, 1, 10), '-6 days', 'weekday 1') AS week, SUM(count) AS marks
    FROM gc_hourly WHERE path LIKE 'play-15min/%' GROUP BY week ORDER BY week`);
  const imports = await all(`SELECT at, start_day, rows, play_marks, error FROM gc_imports ORDER BY at DESC LIMIT 8`);
  const runs7 = await db.prepare(`SELECT COUNT(*) AS n FROM runs WHERE started_at >= ?1`).bind(day7).first();
  const exact = await db.prepare(`SELECT COALESCE(SUM(seconds), 0) AS s FROM seconds_hourly`).first();

  const lengthBuckets = LENGTHS.map(([label, lo, hi]) => ({
    label, runs: lengths.filter((r) => r.s >= lo && r.s <= hi).reduce((a, r) => a + r.runs, 0),
  }));
  const totalRuns = lengths.reduce((a, r) => a + r.runs, 0);
  const sorted = lengths.flatMap((r) => Array(Math.min(r.runs, 100000)).fill(r.s));
  const median = sorted.length ? sorted[Math.floor(sorted.length / 2)] : null;

  const H = 3600;
  const vs = new Map(versionSeconds.map((r) => [r.version, r.s]));
  const versions = launches.map((r) => ({ version: r.version, launches: r.launches, hours: (vs.get(r.version) || 0) / H }))
    .sort((a, b) => cmpVersion(b.version, a.version));

  const total = daily.reduce((a, d) => a + d.old + d.new, 0);
  const last7 = daily.filter((d) => d.day >= day7.slice(0, 10)).reduce((a, d) => a + d.old + d.new, 0);
  const last30 = daily.filter((d) => d.day >= day30).reduce((a, d) => a + d.old + d.new, 0);

  return {
    generated_at: now.toISOString(),
    totals: {
      seconds: total, hours: total / H, hours_7d: last7 / H, hours_30d: last30 / H,
      old_hours: daily.reduce((a, d) => a + d.old, 0) / H, new_hours: daily.reduce((a, d) => a + d.new, 0) / H,
      exact_hours: (exact ? exact.s : 0) / H,
      runs: totalRuns, runs_7d: runs7 ? runs7.n : 0, median_session_seconds: median,
    },
    daily: daily.map((d) => ({ day: d.day, old_hours: d.old / H, new_hours: d.new / H })),
    platforms: platforms.map((p) => ({ platform: p.platform, hours: p.s / H })),
    versions,
    hour_of_day: Array.from({ length: 24 }, (_, h) => ({ hour: h, hours: (hourOfDay.find((r) => r.h === h) || { s: 0 }).s / H })),
    session_lengths: lengthBuckets,
    countries: countries.map((c) => ({ country: c.country || "unknown", runs: c.runs, hours: c.s / H })),
    recent_runs: recentRuns,
    old_builds: retirement(oldWeekly, now),
    imports,
  };
}

/** Old builds' play per full week (Monday to Sunday), and whether the import can be retired. */
export function retirement(oldWeekly, now) {
  const thisWeek = new Date(now.getTime());
  thisWeek.setUTCDate(thisWeek.getUTCDate() - ((thisWeek.getUTCDay() + 6) % 7));
  const current = thisWeek.toISOString().slice(0, 10);
  const weeks = oldWeekly.map((w) => ({ week: w.week, hours: w.marks / 4 }));
  // full weeks only: weeks with no counts at all are the best sign, so fill the gaps up to last week
  const full = [];
  if (weeks.length) {
    for (let d = new Date(weeks[0].week + "T00:00:00Z"); d.toISOString().slice(0, 10) < current; d.setUTCDate(d.getUTCDate() + 7)) {
      const k = d.toISOString().slice(0, 10);
      full.push({ week: k, hours: (weeks.find((w) => w.week === k) || { hours: 0 }).hours });
    }
  }
  const tail = full.slice(-RETIRE_WEEKS);
  const canRetire = tail.length === RETIRE_WEEKS && tail.every((w) => w.hours < RETIRE_HOURS);
  return { weeks: full, this_week_hours: (weeks.find((w) => w.week === current) || { hours: 0 }).hours, can_retire: canRetire };
}

export function cmpVersion(a, b) {
  const pa = String(a).split(".").map(Number), pb = String(b).split(".").map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0);
  return 0;
}

// ---------------------------------------------------------------- the Worker

/** Multiplayer play time (builds after 0.9.10): hours in each mode, last 7 and 30 days and in all, and the share of all
 * play in the last 7 days. */
export async function mpPlay(db, now = new Date(), hours7d = 0) {
  const h7 = new Date(now.getTime() - 7 * 86400e3).toISOString().slice(0, 13);
  const h30 = new Date(now.getTime() - 30 * 86400e3).toISOString().slice(0, 13);
  const rows = (await db.prepare(`
    SELECT mode, SUM(seconds) AS s, SUM(CASE WHEN hour >= ?1 THEN seconds ELSE 0 END) AS s7,
      SUM(CASE WHEN hour >= ?2 THEN seconds ELSE 0 END) AS s30, MIN(hour) AS first
    FROM mp_hourly GROUP BY mode`).bind(h7, h30).all()).results || [];
  const H = 3600;
  const modes = MP_PARAMS.map(([, mode]) => {
    const r = rows.find((x) => x.mode === mode) || {};
    return { mode, hours: (r.s || 0) / H, hours_7d: (r.s7 || 0) / H, hours_30d: (r.s30 || 0) / H };
  });
  const firsts = rows.map((r) => r.first).filter(Boolean).sort();
  const total7 = modes.reduce((a, m) => a + m.hours_7d, 0);
  return {
    since: firsts.length ? firsts[0] : null,
    modes,
    hours_7d: total7,
    hours_30d: modes.reduce((a, m) => a + m.hours_30d, 0),
    hours: modes.reduce((a, m) => a + m.hours, 0),
    share_7d: hours7d > 0 ? total7 / hours7d : 0,
  };
}

/** The page's numbers: play time, multiplayer play time, and multiplayer from the BOARD database (src/mp.js) when it
 * answers. */
export async function allStats(env, now = new Date()) {
  const s = await stats(env.DB, now);
  try {
    s.mp_play = await mpPlay(env.DB, now, s.totals.hours_7d);
  } catch (e) {
    s.mp_play = { error: String(e && e.message ? e.message : e).slice(0, 200) };
  }
  try {
    s.mp = await mpStats(env.BOARD, now);
  } catch (e) {
    s.mp = { error: String(e && e.message ? e.message : e).slice(0, 200) };
  }
  return s;
}

/** Basic auth against STATS_PASSWORD (any user name). */
export function authorized(request, env) {
  if (!env.STATS_PASSWORD) return false;
  const h = request.headers.get("Authorization") || "";
  if (!h.startsWith("Basic ")) return false;
  let pass = "";
  try { pass = atob(h.slice(6)).split(":").slice(1).join(":"); } catch { return false; }
  const a = new TextEncoder().encode(pass), b = new TextEncoder().encode(env.STATS_PASSWORD);
  let diff = a.length ^ b.length;
  for (let i = 0; i < Math.max(a.length, b.length); i++) diff |= (a[i] || 0) ^ (b[i] || 0);
  return diff === 0;
}

const CORS = { "Access-Control-Allow-Origin": "*", "Cache-Control": "no-store" };
const ASK = () => new Response("Password needed", { status: 401, headers: { "WWW-Authenticate": 'Basic realm="OpenSGolf stats"' } });

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === "/count") {
      if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: { ...CORS, "Access-Control-Allow-Methods": "GET, POST" } });
      const c = readCount(url, request.cf && request.cf.country);
      if (c) await storeCount(env.DB, c, new Date());
      // the same answer whether it was kept or not: the game only looks at whether it got through
      return new Response(null, { status: 204, headers: CORS });
    }
    if (url.pathname === "/robots.txt") return new Response("User-agent: *\nDisallow: /\n");
    if (!authorized(request, env)) return ASK();
    if (url.pathname === "/api/stats") {
      return Response.json(await allStats(env), { headers: { "Cache-Control": "no-store" } });
    }
    if (url.pathname === "/admin/import" && request.method === "POST") {
      const from = url.searchParams.get("from");
      const day = from && /^\d{4}-\d{2}-\d{2}$/.test(from) ? from : await importStart(env.DB, new Date());
      return Response.json(await importGoatCounter(env, day));
    }
    if (url.pathname === "/") {
      const { page } = await import("./page.js");
      return new Response(page(await allStats(env)), { headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" } });
    }
    return new Response("Not found", { status: 404 });
  },

  async scheduled(event, env, ctx) {
    const now = new Date();
    ctx.waitUntil(importStart(env.DB, now).then((day) => importGoatCounter(env, day, now)));
  },
};
