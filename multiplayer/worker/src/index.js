// The rival-courses leaderboard (README.md): a Cloudflare Worker with a D1 database.
//
// * POST /result: a player's result in this week's challenge (JSON, see readResult). Only their best of the week is kept.
//   GET /result?d=<the same JSON, base64url> does the same, for the game's own HTTP client, which only GETs (net.rs).
// * GET /board?c=<week>&g=<group>&p=<player>: the top 50 of a week's board (the public one, or a friends' group), and
//   the asking player's own place. One place per nickname (board()).
// * GET /clubs: the shared clubs' list. GET /clubs/new?d=<JSON, base64url> makes a club (newClub). GET /clubs/<id>/ws is
//   a club's WebSocket, served by its Durable Object (club.js).
// * Neighbours leagues (league.js): GET /leagues/new?d= makes one; GET /leagues/<code>?p=<player> is its table;
//   GET /leagues/<code>/join?d= takes a seat; GET /leagues/<code>/report?d= sends a player's numbers; POST
//   /leagues/<code>/save?p=<player> (the save's bytes) and GET /leagues/<code>/save/<seat> keep and hand out saves.
// * GET /admin (password): the latest results, hidden ones too; POST /admin/hide hides a result or a nickname.
//
// Nothing about the player is kept but the random id their game made, the nickname they typed, their group code, the
// result's numbers and the game's platform and version: no address, no country, no cookies.
//
// Copyright the OpenSGolf contributors. GPL-2.0-or-later.

import { breaks, challenge, isWeekId, previousWeekId, score, weekId } from "./challenge.js";
import { clean, cleanNick, generatedNick } from "./names.js";
import { isCompat } from "./club.js";
import { noteActivity } from "./activity.js";
import {
  isLeagueCode, joinLeague, league, LEAGUE_SAVE_MAX, makeLeague, readFacts, readNewLeague, report, seatSave, storeSave,
} from "./league.js";


/** Rows a board shows. */
export const BOARD_SIZE = 50;
/** Platforms the game names (as the play-time count does). */
export const PLATFORMS = new Set([
  "windows", "macos", "linux", "android", "web", "other",
  "windows-arm64", "macos-arm64", "linux-arm64", "other-arm64",
]);
/** The network protocol this Worker speaks (crates/simgolf/src/mp/version.rs PROTOCOL). */
export const PROTOCOL = 1;
/** The largest request body read. */
export const MAX_BODY = 4096;

const isPlayer = (s) => typeof s === "string" && /^[0-9a-f]{16,32}$/.test(s);
const isGroup = (s) => s === "" || (typeof s === "string" && /^[A-Z0-9]{6}$/.test(s));
const isVersion = (v) => typeof v === "string" && /^\d{1,3}\.\d{1,3}\.\d{1,3}$/.test(v);
/** Whether version `a` is newer than `b` ("1.2.3"). */
export function newer(a, b) {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] > pb[i];
  return false;
}
const int = (v, lo, hi) => Number.isInteger(v) && v >= lo && v <= hi;

/** The weeks a result may be for at `now`: this one, and last week's (a week's game may end after midnight Sunday). */
export function openWeeks(now) {
  return [weekId(now), previousWeekId(now)];
}

/** A result's body, checked: what to store, or a string saying why not. */
export function readResult(body, now, minVersion = "") {
  if (!body || typeof body !== "object") return "not a result";
  const { protocol, challenge: c, player, nick, group = "", platform, version, cheats, facts } = body;
  if (protocol !== PROTOCOL) return "update the game";
  if (!isWeekId(c) || !openWeeks(now).includes(c)) return "not an open challenge";
  if (!isPlayer(player)) return "bad player id";
  if (!isGroup(group)) return "bad group code";
  if (!PLATFORMS.has(platform)) return "bad platform";
  if (!isVersion(version)) return "bad version";
  if (minVersion && newer(minVersion, version)) return "update the game";
  if (cheats !== false) return "cheats were on";
  if (!facts || typeof facts !== "object") return "no facts";
  const f = facts;
  if (!(int(f.year, 2001, 2100) && int(f.cash, -1e9, 1e10) && int(f.holes, 0, 36) && int(f.fun, -1e6, 1e6)
    && int(f.skill, -1e6, 1e6) && int(f.members, 0, 1e5) && int(f.rank, 0, 3))) return "bad facts";
  const ch = challenge(c);
  if (f.year <= ch.endYear) return "the challenge's year is not over";
  // the game sends its numbers as the year ends, never from a game played on for more years
  if (f.year !== ch.endYear + 1) return "not the challenge's year end";
  // one course of at most 18 holes (the Second course extra is off in multiplayer games)
  if (f.holes > 18) return "more than 18 holes";
  const broke = breaks(ch, f);
  if (broke) return broke;
  const n = cleanNick(nick);
  if (!n) return "bad nickname";
  return {
    challenge: c, player, nick: n, grp: group, score: score(ch, f), platform, version,
    cash: f.cash, fun: f.fun, skill: f.skill, members: f.members, holes: f.holes, rank: f.rank,
  };
}

/** Stores a result: a new row, or a better score for a row that is there (a worse or equal one changes nothing). */
export async function storeResult(db, r, now) {
  const at = now.toISOString();
  const res = await db.prepare(
    `INSERT INTO results (challenge, player, nick, grp, score, cash, fun, skill, members, holes, rank, platform, version, at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
     ON CONFLICT (challenge, player) DO UPDATE SET nick = ?3, grp = ?4, score = ?5, cash = ?6, fun = ?7, skill = ?8,
       members = ?9, holes = ?10, rank = ?11, platform = ?12, version = ?13, at = ?14
     WHERE excluded.score > results.score`,
  ).bind(r.challenge, r.player, r.nick, r.grp, r.score, r.cash, r.fun, r.skill, r.members, r.holes, r.rank,
    r.platform, r.version, at).run();
  return (res.meta && res.meta.changes) > 0;
}

/**
 * Who a board row belongs to: one place per nickname (any capitals), so a person whose results came from several
 * installs (another browser, a cleared save) is on the board once, with their best. A nickname an admin hid, or the
 * game's own "Golfer 123" name (two strangers may share it), is told apart by the player id instead.
 */
const WHO = `CASE WHEN h.nick IS NOT NULL OR r.nick GLOB 'Golfer [0-9][0-9][0-9]' THEN 'p:' || r.player
  ELSE 'n:' || lower(r.nick) END`;

/** Each one's best visible row of a week's board (?1 the week, ?2 the group), with `who` and `hid`. */
const BEST = `WITH v AS (
    SELECT r.player, r.nick, h.nick IS NOT NULL AS hid, r.score, r.cash, r.fun, r.skill, r.members, r.holes, r.rank, r.at,
      ${WHO} AS who
    FROM results r LEFT JOIN hidden_nicks h ON h.nick = r.nick
    WHERE r.challenge = ?1 AND r.grp = ?2 AND r.hidden = 0
  ), best AS (
    SELECT v.*, ROW_NUMBER() OVER (PARTITION BY who ORDER BY score DESC, at ASC) AS k FROM v
  )`;

/** A week's board: each one's best row, the top first, and for `player` their own place (1 = first). */
export async function board(db, c, grp, player) {
  const { results } = await db.prepare(
    `${BEST} SELECT * FROM best WHERE k = 1 ORDER BY score DESC, at ASC LIMIT ?3`,
  ).bind(c, grp, BOARD_SIZE).all();
  // the asking player's own place: their nickname's (or their own) best, wherever it came from
  let who = null;
  if (isPlayer(player)) {
    const w = await db.prepare(`${BEST} SELECT who FROM v WHERE player = ?3`).bind(c, grp, player).first();
    who = w ? w.who : null;
  }
  const rows = results.map((r, i) => ({
    place: i + 1, nick: r.hid ? generatedNick(r.player) : r.nick, score: r.score, cash: r.cash, fun: r.fun,
    skill: r.skill, members: r.members, holes: r.holes, rank: r.rank, mine: who !== null && r.who === who,
  }));
  let me = null;
  if (who !== null && !rows.some((r) => r.mine)) {
    const mine = await db.prepare(`${BEST} SELECT score, at FROM best WHERE k = 1 AND who = ?3`).bind(c, grp, who).first();
    const ahead = await db.prepare(
      `${BEST} SELECT COUNT(*) AS n FROM best WHERE k = 1 AND (score > ?3 OR (score = ?3 AND at < ?4))`,
    ).bind(c, grp, mine.score, mine.at).first();
    me = { place: ahead.n + 1, score: mine.score };
  }
  return { challenge: challenge(c), rows, me };
}

// ---------------------------------------------------------------- admin

/** Whether the request carries the admin password (basic auth, any user name). */
export function authorised(request, env) {
  const want = env.ADMIN_PASSWORD;
  if (!want) return false;
  const h = request.headers.get("Authorization") || "";
  if (!h.startsWith("Basic ")) return false;
  let pass = "";
  try {
    pass = atob(h.slice(6)).split(":").slice(1).join(":");
  } catch {
    return false;
  }
  if (pass.length !== want.length) return false;
  let diff = 0;
  for (let i = 0; i < want.length; i++) diff |= pass.charCodeAt(i) ^ want.charCodeAt(i);
  return diff === 0;
}

/** The latest results, hidden ones too, for the admin page. */
export async function recent(db, limit = 100) {
  const { results } = await db.prepare(
    `SELECT challenge, player, nick, grp, score, platform, version, at, hidden FROM results ORDER BY at DESC LIMIT ?1`,
  ).bind(limit).all();
  return results;
}

/** Hides (or shows again) one result, or a nickname everywhere. */
export async function hide(db, body, now) {
  if (body && isWeekId(body.challenge) && isPlayer(body.player)) {
    const r = await db.prepare(`UPDATE results SET hidden = ?3 WHERE challenge = ?1 AND player = ?2`)
      .bind(body.challenge, body.player, body.show ? 0 : 1).run();
    return r.meta.changes > 0;
  }
  if (body && isClubId(body.club)) {
    const r = await db.prepare(`UPDATE clubs SET hidden = ?2 WHERE id = ?1`).bind(body.club, body.show ? 0 : 1).run();
    return r.meta.changes > 0;
  }
  if (body && typeof body.nick === "string" && body.nick.length > 0 && body.nick.length <= 32) {
    if (body.show) await db.prepare(`DELETE FROM hidden_nicks WHERE nick = ?1`).bind(body.nick).run();
    else {
      await db.prepare(`INSERT INTO hidden_nicks (nick, at) VALUES (?1, ?2) ON CONFLICT (nick) DO NOTHING`)
        .bind(body.nick, now.toISOString()).run();
    }
    return true;
  }
  return false;
}

// ---------------------------------------------------------------- shared clubs

/** Clubs the list shows. */
export const CLUB_LIST = 100;
/** Days a club with nobody on stays in the list. */
export const CLUB_LIST_DAYS = 60;

/** A club's name, tidied, or null: 3 to 32 letters, digits, spaces and a few marks. */
export function cleanClubName(name) {
  if (typeof name !== "string") return null;
  const n = name.trim().replace(/\s+/g, " ");
  if (n.length < 3 || n.length > 32 || !/^[A-Za-z0-9 _.,'&!\-]+$/.test(n)) return null;
  return clean(n) ? n : null;
}

/** A new club's request, checked: what the club is made with, or a string saying why not. */
export function readNewClub(b) {
  if (!b || typeof b !== "object") return "not a club";
  if (b.protocol !== PROTOCOL) return "update the game";
  if (!isPlayer(b.player)) return "bad player id";
  const name = cleanClubName(b.name);
  if (!name) return "bad club name";
  if (!isCompat(b.compat)) return "bad game record";
  return { name, owner: b.player, compat: b.compat, listed: b.listed !== false };
}

/** The clubs list: those with someone on first, then the latest played. */
export async function clubs(db, now) {
  const since = new Date(now.getTime() - CLUB_LIST_DAYS * 86400e3).toISOString();
  const { results } = await db.prepare(
    `SELECT id, name, version, compat, players, tick, at FROM clubs WHERE listed = 1 AND hidden = 0 AND at >= ?1
     ORDER BY players > 0 DESC, at DESC LIMIT ?2`,
  ).bind(since, CLUB_LIST).all();
  return results.map((r) => ({ ...r, compat: JSON.parse(r.compat) }));
}

/** A private club's code: its id's first 8 hex digits. */
export const CLUB_CODE = 8;

/** The club a code names (listed or not, while not hidden), or null. */
export async function clubByCode(db, code) {
  if (typeof code !== "string" || !/^[0-9a-f]{8}$/.test(code)) return null;
  const r = await db.prepare(
    `SELECT id, name, version, compat, players, tick, at FROM clubs WHERE substr(id, 1, ${CLUB_CODE}) = ?1 AND hidden = 0
     ORDER BY at DESC LIMIT 1`,
  ).bind(code).first();
  return r ? { ...r, compat: JSON.parse(r.compat) } : null;
}

const isClubId = (s) => typeof s === "string" && /^[0-9a-f]{64}$/.test(s);

// ---------------------------------------------------------------- the Worker

const json = (v, status = 200, extra = {}) => new Response(JSON.stringify(v), {
  status, headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*", ...extra },
});

async function readJson(request) {
  const text = await request.text();
  if (text.length > MAX_BODY) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** The JSON in a base64url text (GET /result's `d`), or null. */
export function fromBase64Url(d) {
  if (typeof d !== "string" || d.length > MAX_BODY) return null;
  try {
    const bin = atob(d.replace(/-/g, "+").replace(/_/g, "/"));
    return JSON.parse(new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0))));
  } catch {
    return null;
  }
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const now = new Date();
    if (request.method === "OPTIONS") {
      return new Response(null, { headers: {
        "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Methods": "GET, POST", "Access-Control-Allow-Headers": "Content-Type",
      } });
    }
    if (url.pathname === "/result" && (request.method === "POST" || request.method === "GET")) {
      const r = readResult(request.method === "POST" ? await readJson(request) : fromBase64Url(url.searchParams.get("d")), now,
        env.MIN_VERSION || "");
      if (typeof r === "string") return json({ ok: false, error: r }, 400);
      const better = await storeResult(env.DB, r, now);
      await noteActivity(env.DB, "challenge", r.challenge, r.player, now);
      return json({ ok: true, better }, 200, { "Cache-Control": "no-store" });
    }
    if (url.pathname === "/board" && request.method === "GET") {
      const c = url.searchParams.get("c") || weekId(now);
      const g = url.searchParams.get("g") || "";
      if (!isWeekId(c) || !isGroup(g)) return json({ ok: false, error: "bad board" }, 400);
      const b = await board(env.DB, c, g, url.searchParams.get("p") || "");
      // a board a minute old is fine: the edge may keep it that long
      return json({ ok: true, ...b }, 200, { "Cache-Control": url.searchParams.get("p") ? "no-store" : "public, max-age=60" });
    }
    if (url.pathname === "/challenge" && request.method === "GET") {
      return json({ ok: true, challenge: challenge(weekId(now)) }, 200, { "Cache-Control": "public, max-age=3600" });
    }
    if (url.pathname === "/badge" && request.method === "GET") {
      // this week's challenge as a shields.io endpoint badge (README.md): the goal, the land and how many are on the board
      const c = challenge(weekId(now));
      const n = (await board(env.DB, c.id, "", "")).rows.length;
      const goal = { cash: "most cash", fun: "highest fun", members: "most members", skill: "highest skill" }[c.measure];
      const who = n ? ` · ${n >= BOARD_SIZE ? `${BOARD_SIZE}+` : n} playing` : " · be the first";
      return json({ schemaVersion: 1, label: "this week's challenge", message: `${goal} · ${c.theme}${who}`, color: "2f8f4e" },
        200, { "Cache-Control": "public, max-age=600" });
    }
    if (url.pathname === "/clubs" && request.method === "GET") {
      if (!env.CLUB) return json({ ok: false, error: "no shared clubs here" }, 404);
      return json({ ok: true, protocol: PROTOCOL, clubs: await clubs(env.DB, now) }, 200, { "Cache-Control": "no-store" });
    }
    const code = url.pathname.match(/^\/clubs\/code\/([0-9A-Fa-f]{8})$/);
    if (code && request.method === "GET") {
      if (!env.CLUB) return json({ ok: false, error: "no shared clubs here" }, 404);
      const club = await clubByCode(env.DB, code[1].toLowerCase());
      if (!club) return json({ ok: false, error: "no club has that code" }, 404, { "Cache-Control": "no-store" });
      return json({ ok: true, protocol: PROTOCOL, club }, 200, { "Cache-Control": "no-store" });
    }
    if (url.pathname === "/clubs/new" && request.method === "GET") {
      if (!env.CLUB) return json({ ok: false, error: "no shared clubs here" }, 404);
      const c = readNewClub(fromBase64Url(url.searchParams.get("d")));
      if (typeof c === "string") return json({ ok: false, error: c }, 400);
      if (env.MIN_VERSION && newer(env.MIN_VERSION, c.compat.version)) return json({ ok: false, error: "update the game" }, 400);
      const id = env.CLUB.newUniqueId();
      const r = await env.CLUB.get(id).fetch("https://club/init", {
        method: "POST", body: JSON.stringify({ id: id.toString(), ...c }),
      });
      if (!r.ok) return json({ ok: false, error: "the club could not be made" }, 500);
      return json({ ok: true, id: id.toString() }, 200, { "Cache-Control": "no-store" });
    }
    const m = url.pathname.match(/^\/clubs\/([0-9a-f]+)\/(ws|state)$/);
    if (m && request.method === "GET" && env.CLUB) {
      if (!isClubId(m[1])) return json({ ok: false, error: "no such club" }, 404);
      let id;
      try {
        id = env.CLUB.idFromString(m[1]);
      } catch {
        return json({ ok: false, error: "no such club" }, 404);
      }
      if (m[2] === "state" && !authorised(request, env)) return new Response("password needed", { status: 401 });
      return env.CLUB.get(id).fetch(new Request(`https://club/${m[2]}`, request));
    }
    const lg = url.pathname.match(/^\/leagues\/([A-Z0-9]{6})(?:\/(join|report|save)(?:\/([0-3]))?)?$/);
    if (url.pathname === "/leagues/new" && request.method === "GET") {
      const b = fromBase64Url(url.searchParams.get("d"));
      if (b && b.protocol !== PROTOCOL) return json({ ok: false, error: "update the game" }, 400);
      const l = readNewLeague(b);
      if (typeof l === "string") return json({ ok: false, error: l }, 400);
      const code = await makeLeague(env.DB, l, now);
      await noteActivity(env.DB, "league", code, l.player, now);
      return json({ ok: true, ...(await league(env.DB, code, l.player)) }, 200, { "Cache-Control": "no-store" });
    }
    if (lg && isLeagueCode(lg[1])) {
      const code = lg[1];
      const noStore = { "Cache-Control": "no-store" };
      if (!lg[2] && request.method === "GET") {
        const l = await league(env.DB, code, url.searchParams.get("p") || "");
        return l ? json({ ok: true, ...l }, 200, noStore) : json({ ok: false, error: "no league has that code" }, 404, noStore);
      }
      if (lg[2] === "join" && request.method === "GET") {
        const b = fromBase64Url(url.searchParams.get("d")) || {};
        if (b.protocol !== PROTOCOL) return json({ ok: false, error: "update the game" }, 400);
        const seat = await joinLeague(env.DB, code, b.player, b.nick, now);
        if (typeof seat === "string") return json({ ok: false, error: seat }, 400, noStore);
        await noteActivity(env.DB, "league", code, b.player, now);
        return json({ ok: true, seat, ...(await league(env.DB, code, b.player)) }, 200, noStore);
      }
      if (lg[2] === "report" && request.method === "GET") {
        const b = fromBase64Url(url.searchParams.get("d")) || {};
        const f = readFacts(b.facts);
        if (!f || typeof b.player !== "string") return json({ ok: false, error: "bad report" }, 400);
        if (b.cheats !== false) return json({ ok: false, error: "cheats were on" }, 400);
        const ok = await report(env.DB, code, b.player, f, now);
        if (ok) await noteActivity(env.DB, "league", code, b.player, now);
        return json({ ok }, 200, noStore);
      }
      if (lg[2] === "save" && lg[3] === undefined && request.method === "POST") {
        const bytes = new Uint8Array(await request.arrayBuffer());
        if (bytes.length === 0 || bytes.length > LEAGUE_SAVE_MAX) return json({ ok: false, error: "bad save" }, 400);
        return json({ ok: await storeSave(env.DB, code, url.searchParams.get("p") || "", bytes, now) }, 200, noStore);
      }
      if (lg[2] === "save" && lg[3] !== undefined && request.method === "GET") {
        const bytes = await seatSave(env.DB, code, Number(lg[3]));
        if (!bytes) return json({ ok: false, error: "no save yet" }, 404, noStore);
        return new Response(bytes, { headers: { "Content-Type": "application/octet-stream", "Access-Control-Allow-Origin": "*", ...noStore } });
      }
    }
    if (url.pathname.startsWith("/admin")) {
      if (!authorised(request, env)) {
        return new Response("password needed", { status: 401, headers: { "WWW-Authenticate": 'Basic realm="board admin"' } });
      }
      if (url.pathname === "/admin" && request.method === "GET") return json({ ok: true, results: await recent(env.DB) });
      if (url.pathname === "/admin/hide" && request.method === "POST") {
        return json({ ok: await hide(env.DB, await readJson(request), now) });
      }
    }
    return json({ ok: false, error: "not found" }, 404);
  },
};
