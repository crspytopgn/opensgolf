import test from "node:test";
import assert from "node:assert/strict";
import { memoryDb } from "./d1shim.mjs";
import worker, { readCount, storeCount, mpPlay, gcRows, importGoatCounter, gcFetch, importStart, stats, allStats, retirement, FIRST_DAY } from "../src/index.js";
import { mpStats } from "../src/mp.js";
import { page } from "../src/page.js";

const u = (q) => new URL(`https://x.test/count?${q}`);

test("only the game's own counts are read", () => {
  assert.deepEqual(readCount(u("e=start/windows/0.8.2&s=0123abcd&n=0"), "GB"),
    { kind: "start", platform: "windows", version: "0.8.2", sid: "0123abcd", n: 0, t: 0, country: "GB", mp: {} });
  assert.equal(readCount(u("e=play/linux&v=0.8.8&s=0123abcd&n=2&t=601"), "GB").t, 601);
  assert.equal(readCount(u("e=play/web&v=0.8.8&s=kept&t=1800"), null).sid, null);
  for (const q of ["e=play/linux&v=0.8.8&s=0123abcd", "e=play/linux&v=0.8.8&s=0123abcd&t=-5", "e=play/linux&v=0.8.8&s=0123abcd&t=1.5",
    "e=play/linux&v=0.8.8&s=0123abcd&t=604801", "e=play/amiga&v=0.8.8&s=0123abcd&t=5"]) {
    assert.equal(readCount(u(q), "GB"), null, q);
  }
  assert.equal(readCount(u("e=play-15min/linux&v=0.8.2&s=0123abcd&n=1"), "XX").platform, "linux");
  assert.equal(readCount(u("e=play-15min/web&v=0.8.2&s=kept&n=0"), null).sid, null);
  for (const q of ["e=play-15min/amiga&v=0.8.2&s=0123abcd", "e=play-15min/web&s=0123abcd", "e=start/web/x&s=0123abcd",
    "e=play-15min/web&v=0.8.2&s=NOTHEX!!", "e=play-15min/web&v=0.8.2", "e=other&v=0.8.2&s=0123abcd", "e=play-15min/web&v=0.8.2&s=0123abcd&n=-1"]) {
    assert.equal(readCount(u(q), "GB"), null, q);
  }
  assert.equal(readCount(u("e=start/web/0.8.2&s=0123abcd"), "not a country").country, "");
});

test("runs, marks, repeats and kept counts", async () => {
  const db = memoryDb();
  const t = new Date("2026-11-20T10:05:00Z");
  const c = (q) => readCount(u(q), "GB");
  await storeCount(db, c("e=start/macos-arm64/0.8.2&s=aaaaaaaa"), t);
  await storeCount(db, c("e=play-15min/macos-arm64&v=0.8.2&s=aaaaaaaa&n=1"), t);
  await storeCount(db, c("e=play-15min/macos-arm64&v=0.8.2&s=aaaaaaaa&n=1"), t); // a repeat
  await storeCount(db, c("e=play-15min/macos-arm64&v=0.8.2&s=aaaaaaaa&n=2"), new Date("2026-11-20T11:01:00Z"));
  await storeCount(db, c("e=play-15min/web&v=0.8.2&s=bbbbbbbb&n=1"), t); // no start seen: the run is made
  await storeCount(db, c("e=play-15min/windows&v=0.8.1&s=kept"), t);
  const runs = db.raw.prepare("SELECT sid, beats FROM runs ORDER BY sid").all();
  assert.deepEqual(runs.map((r) => [r.sid, r.beats]), [["aaaaaaaa", 2], ["bbbbbbbb", 1]]);
  const marks = db.raw.prepare("SELECT SUM(beats) AS n, SUM(kept * beats) AS k FROM play_hourly").get();
  assert.equal(marks.n, 4);
  assert.equal(marks.k, 1);
});

test("exact seconds: only the increase counts, repeats and late copies add nothing", async () => {
  const db = memoryDb();
  const c = (q) => readCount(u(q), "GB");
  const at = (h) => new Date(`2026-11-25T${h}:30:00Z`);
  await storeCount(db, c("e=start/linux/0.8.8&s=aaaaaaaa"), at("10"));
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=1&t=300"), at("10"));
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=1&t=300"), at("10")); // a repeat
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=3&t=1000"), at("11")); // count 2 was lost: nothing missing
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=2&t=600"), at("11")); // ... and came late: nothing added
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=4&t=1037"), at("11")); // the quit
  await storeCount(db, c("e=play/linux&v=0.8.8&s=aaaaaaaa&n=0&t=1037"), at("12")); // re-sent at the next start
  await storeCount(db, c("e=play/web&v=0.8.8&s=bbbbbbbb&n=1&t=42"), at("12")); // no start seen: the run is made
  await storeCount(db, c("e=play/windows&v=0.8.8&s=kept&t=1800"), at("12")); // an older build's two kept marks
  await storeCount(db, c("e=play/windows&v=0.8.8&s=cccccccc&n=0&t=0"), at("12")); // nothing played: nothing stored
  const hours = db.raw.prepare("SELECT hour, kept, SUM(seconds) AS s FROM seconds_hourly GROUP BY hour, kept ORDER BY hour, kept").all();
  assert.deepEqual(hours.map((r) => [r.hour.slice(11), r.kept, r.s]), [["10", 0, 300], ["11", 0, 737], ["12", 0, 42], ["12", 1, 1800]]);
  const runs = db.raw.prepare("SELECT r.sid AS sid, t.seconds AS s FROM runs r LEFT JOIN run_time t ON t.sid = r.sid ORDER BY r.sid").all();
  assert.deepEqual(runs.map((r) => [r.sid, r.s]), [["aaaaaaaa", 1037], ["bbbbbbbb", 42]]);
  const s = await stats(db, new Date("2026-11-25T13:00:00Z"));
  assert.equal(s.totals.seconds, 1037 + 42 + 1800);
  assert.equal(s.totals.runs, 2);
  assert.deepEqual(s.session_lengths.filter((b) => b.runs).map((b) => [b.label, b.runs]), [["under 15 min", 1], ["15–30 min", 1]]);
  assert.equal(s.recent_runs.find((r) => r.platform === "linux").seconds, 1037);
  assert.match(page(s), /17 min/);
});

const gcPage = {
  more: false,
  hits: [
    { path_id: 1, path: "play-15min/windows", stats: [{ day: "2026-11-14", hourly: [0, 3, ...Array(22).fill(0)] }, { day: "2026-09-30", hourly: [5, ...Array(23).fill(0)] }] },
    { path_id: 2, path: "start/windows/0.8.1", stats: [{ day: "2026-11-14", hourly: [1, 2, ...Array(22).fill(0)] }] },
    { path_id: 3, path: "/opensgolf", stats: [{ day: "2026-11-14", hourly: [9, ...Array(23).fill(0)] }] },
    { path_id: 4, path: "start-free-graphics", stats: [{ day: "2026-11-14", hourly: [9, ...Array(23).fill(0)] }] },
  ],
};

test("GoatCounter rows: the game's events only, from the start day", () => {
  assert.deepEqual(gcRows(gcPage.hits, "2026-10-01"), [
    ["2026-11-14T01", "play-15min/windows", 3], ["2026-11-14T00", "start/windows/0.8.1", 1], ["2026-11-14T01", "start/windows/0.8.1", 2],
  ]);
});

test("the import replaces hours, pages, and logs problems", async () => {
  const db = memoryDb();
  const env = { DB: db, GOATCOUNTER_TOKEN: "t" };
  const now = new Date("2026-11-16T04:00:00Z");
  assert.equal(await importStart(db, now), FIRST_DAY);
  const urls = [];
  const fetcher = async (url, init) => {
    urls.push(url);
    assert.equal(init.headers.Authorization, "Bearer t");
    const first = !url.includes("exclude_paths");
    return new Response(JSON.stringify(first ? { ...gcPage, more: true } : { more: false, hits: [] }));
  };
  const r = await importGoatCounter(env, FIRST_DAY, now, fetcher);
  assert.equal(r.rows, 3);
  assert.equal(r.play_marks, 3);
  assert.ok(urls[1].includes("exclude_paths=1%2C2%2C3%2C4"), urls[1]);
  // again: same counts, not doubled
  await importGoatCounter(env, FIRST_DAY, now, fetcher);
  assert.equal(db.raw.prepare("SELECT SUM(count) AS n FROM gc_hourly").get().n, 6);
  assert.equal(await importStart(db, now), "2026-11-01");
  const bad = await importGoatCounter({ DB: db }, FIRST_DAY, now, fetcher);
  assert.match(bad.error, /GOATCOUNTER_TOKEN/);
  assert.equal(db.raw.prepare("SELECT COUNT(*) AS n FROM gc_imports WHERE error IS NOT NULL").get().n, 1);
});

test("a GoatCounter call that fails for a moment is tried again", async () => {
  const waited = [];
  const sleep = async (ms) => { waited.push(ms); };
  let calls = 0;
  const flaky = async () => (++calls < 3 ? new Response("no", { status: 404 }) : new Response("{}"));
  assert.equal((await gcFetch("u", {}, flaky, [1, 2, 3], sleep)).status, 200);
  assert.deepEqual(waited, [1, 2]);
  // gives up after the last wait, with the last answer
  calls = -10;
  assert.equal((await gcFetch("u", {}, flaky, [1, 2], sleep)).status, 404);
  // a sign-in error is not tried again
  waited.length = 0;
  assert.equal((await gcFetch("u", {}, async () => new Response("", { status: 401 }), [1], sleep)).status, 401);
  assert.deepEqual(waited, []);
  // network errors too
  let n = 0;
  const net = async () => { if (n++ === 0) throw new Error("reset"); return new Response("{}"); };
  assert.equal((await gcFetch("u", {}, net, [1], sleep)).status, 200);
  // the import goes through a 404 and logs no error
  const db = memoryDb();
  let first = true;
  const f = async () => { if (first) { first = false; return new Response("", { status: 404 }); } return new Response(JSON.stringify(gcPage)); };
  const r = await importGoatCounter({ DB: db, GOATCOUNTER_TOKEN: "t" }, FIRST_DAY, new Date("2026-11-16T04:00:00Z"), f, [1], sleep);
  assert.equal(r.error, undefined);
  assert.equal(r.rows, 3);
});

test("stats from both sources, and the page draws", async () => {
  const db = memoryDb();
  await importGoatCounter({ DB: db, GOATCOUNTER_TOKEN: "t" }, FIRST_DAY, new Date("2026-11-16T04:00:00Z"), async () => new Response(JSON.stringify(gcPage)));
  const c = (q) => readCount(u(q), "DE");
  await storeCount(db, c("e=start/linux/0.8.2&s=cccccccc"), new Date("2026-11-16T09:00:00Z"));
  for (let n = 1; n <= 5; n++) await storeCount(db, c(`e=play-15min/linux&v=0.8.2&s=cccccccc&n=${n}`), new Date("2026-11-16T10:00:00Z"));
  await storeCount(db, c("e=start/web/0.8.2&s=dddddddd"), new Date("2026-11-16T09:00:00Z"));
  const s = await stats(db, new Date("2026-11-16T12:00:00Z"));
  assert.equal(s.totals.hours, 2); // 3 old marks + 5 new
  assert.equal(s.totals.old_hours, 0.75);
  assert.equal(s.totals.runs, 2);
  assert.deepEqual(s.platforms.map((p) => [p.platform, p.hours]), [["linux", 1.25], ["windows", 0.75]]);
  assert.deepEqual(s.versions.map((v) => [v.version, v.launches, v.hours]), [["0.8.2", 2, 1.25], ["0.8.1", 3, 0]]);
  assert.deepEqual(s.session_lengths.filter((b) => b.runs).map((b) => [b.label, b.runs]), [["under 15 min", 1], ["1–2 h", 1]]);
  assert.equal(s.countries[0].country, "DE");
  assert.equal(s.hour_of_day[1].hours, 0.75);
  // an exact count adds to the same totals: 15 min 36 s more
  await storeCount(db, c("e=play/linux&v=0.8.8&s=ffffffff&n=1&t=936"), new Date("2026-11-16T11:00:00Z"));
  const s2 = await stats(db, new Date("2026-11-16T12:00:00Z"));
  assert.equal(s2.totals.seconds, 8 * 900 + 936);
  assert.equal(s2.totals.exact_hours, 0.26);
  assert.deepEqual(s2.platforms.map((p) => [p.platform, p.hours]), [["linux", 1.51], ["windows", 0.75]]);
  const html = page(s);
  assert.match(html, /OpenSGolf play time/);
  assert.match(html, /2 h/);
});

test("retiring the import: three full quiet weeks", () => {
  const now = new Date("2026-12-09T12:00:00Z"); // a Wednesday; this week starts 7 Dec
  const busy = [{ week: "2026-11-02", marks: 1400 }, { week: "2026-11-09", marks: 200 }];
  assert.equal(retirement(busy, now).can_retire, true, "weeks with nothing count as quiet");
  assert.equal(retirement([...busy, { week: "2026-11-30", marks: 8 }], now).can_retire, false, "2 h last week");
  assert.equal(retirement([...busy, { week: "2026-11-30", marks: 3 }, { week: "2026-12-07", marks: 50 }], now).can_retire, true);
  assert.equal(retirement([], now).can_retire, false);
});

test("the Worker: counts are open, the rest needs the password", async () => {
  const db = memoryDb();
  const env = { DB: db, STATS_PASSWORD: "pw" };
  const r = await worker.fetch(new Request("https://x.test/count?e=start/web/0.8.2&s=eeeeeeee"), env, {});
  assert.equal(r.status, 204);
  assert.equal(r.headers.get("Access-Control-Allow-Origin"), "*");
  assert.equal((await worker.fetch(new Request("https://x.test/count?e=junk"), env, {})).status, 204);
  assert.equal((await worker.fetch(new Request("https://x.test/"), env, {})).status, 401);
  const wrong = { Authorization: "Basic " + btoa("x:nope") }, right = { Authorization: "Basic " + btoa("x:pw") };
  assert.equal((await worker.fetch(new Request("https://x.test/", { headers: wrong }), env, {})).status, 401);
  const ok = await worker.fetch(new Request("https://x.test/api/stats", { headers: right }), env, {});
  assert.equal((await ok.json()).totals.runs, 1);
  assert.equal((await worker.fetch(new Request("https://x.test/", { headers: right }), { DB: db }, {})).status, 401, "no password set: closed");
});

test("multiplayer: counts from the server's database, without hidden clubs, ids or names", async () => {
  const board = memoryDb(new URL("../../multiplayer/worker/schema.sql", import.meta.url));
  const now = new Date("2026-11-27T12:00:00Z");
  const q = (sql, ...p) => board.raw.prepare(sql).run(...p);
  const club = (id, hidden, players, at, made = "2026-11-25T10:00:00Z") =>
    q(`INSERT INTO clubs (id, name, version, compat, listed, hidden, players, tick, made, at) VALUES (?, 'Secret Name', '0.9.0', '{}', 1, ?, ?, 0, ?, ?)`, id, hidden, players, made, at);
  club("c1", 0, 2, "2026-11-27T11:40:00Z");
  club("c2", 0, 1, "2026-11-26T08:00:00Z"); // its row is old: not counted as on now
  club("test", 1, 3, "2026-11-27T11:50:00Z");
  const act = (day, mode, place, player, n = 1) => q(`INSERT INTO activity (day, mode, place, player, n) VALUES (?, ?, ?, ?, ?)`, day, mode, place, player, n);
  act("2026-11-27", "club", "c1", "aaaa000000000001", 2);
  act("2026-11-27", "club", "c1", "aaaa000000000002");
  act("2026-11-27", "club", "test", "aaaa000000000009"); // a hidden club
  act("2026-11-27", "league", "ABC123", "aaaa000000000001");
  act("2026-11-26", "challenge", "2026-W48", "aaaa000000000003", 3);
  act("2026-11-10", "challenge", "2026-W46", "aaaa000000000004");
  q(`INSERT INTO leagues (code, name, property, difficulty, measure, seed, owner, made) VALUES ('ABC123', 'L', 0, 0, 'cash', 1, 'x', '2026-11-27T09:00:00Z')`);
  q(`INSERT INTO league_players (code, seat, player, nick, at) VALUES ('ABC123', 0, 'aaaa000000000001', 'Nick', '2026-11-27T09:00:00Z')`);
  q(`INSERT INTO results (challenge, player, nick, grp, score, cash, fun, skill, members, holes, rank, platform, version, at)
     VALUES ('2026-W48', 'aaaa000000000003', 'Nick', 'FRIEND', 1, 1, 1, 1, 1, 1, 0, 'web', '0.9.0', '2026-11-26T10:00:00Z')`);
  const m = await mpStats(board, now);
  assert.deepEqual(m.players, { today: 2, d7: 3, d30: 4, all: 4 });
  assert.deepEqual(m.clubs, { total: 2, public: 2, busy: 1, on_now: 2, new_7d: 2 });
  assert.deepEqual(m.leagues, { total: 1, seats: 1, new_7d: 1 });
  assert.deepEqual(m.modes_7d.map((r) => [r.mode, r.players, r.places, r.events]), [["club", 2, 1, 3], ["league", 1, 1, 1], ["challenge", 1, 1, 3]]);
  assert.equal(m.daily.length, 30);
  assert.deepEqual(m.daily.at(-1), { day: "2026-11-27", players: 2, club: 2, league: 1, challenge: 0 });
  assert.deepEqual(m.weekly.map((w) => [w.week, w.players]), [["2026-11-09", 1], ["2026-11-23", 3]]);
  assert.deepEqual(m.challenge_weeks, [{ week: "2026-W48", players: 1, in_groups: 1, results: 3 }]);
  const json = JSON.stringify(m);
  for (const secret of ["aaaa", "Nick", "Secret Name", "ABC123", "FRIEND", "c1"]) assert.ok(!json.includes(`"${secret}`) && !json.includes(secret + '"'), secret);
  assert.equal(await mpStats(undefined, now), null);

  const db = memoryDb();
  const html = page({ ...(await stats(db, now)), mp: m });
  assert.match(html, /Multiplayer players per day/);
  assert.ok(!html.includes("aaaa0000") && !html.includes("Secret Name") && !html.includes("ABC123"));
  assert.match(page({ ...(await stats(db, now)), mp: null }), /not bound/);
  const env = { DB: db, BOARD: { prepare() { throw new Error("down"); } } };
  assert.match((await allStats(env, now)).mp.error, /down/, "a board that fails leaves the rest of the page");
});

test("multiplayer seconds: read with the play count, each mode kept as t is, shown on the page", async () => {
  // read: part of t, so never more; a bad one is left out and the count still taken
  assert.deepEqual(readCount(u("e=play/web&v=0.9.11&s=0123abcd&n=1&t=600&mc=300&ml=0&mw=100"), "GB").mp, { club: 300, challenge: 100 });
  assert.deepEqual(readCount(u("e=play/web&v=0.9.11&s=0123abcd&n=1&t=600&mc=601&mw=x&ml=-5"), "GB").mp, {});
  assert.deepEqual(readCount(u("e=play/web&v=0.9.11&s=kept&t=600&mc=300"), "GB").mp, {}, "kept seconds have no run");
  assert.equal(readCount(u("e=play/web&v=0.9.11&s=0123abcd&n=1&t=600&mc=601"), "GB").t, 600);

  const db = memoryDb();
  const c = (q) => readCount(u(q), "GB");
  const at = (h) => new Date(`2026-11-28T${h}:10:00Z`);
  await storeCount(db, c("e=start/windows/0.9.11&s=aaaaaaaa"), at("10"));
  await storeCount(db, c("e=play/windows&v=0.9.11&s=aaaaaaaa&n=1&t=300&mc=200"), at("10"));
  await storeCount(db, c("e=play/windows&v=0.9.11&s=aaaaaaaa&n=1&t=300&mc=200"), at("10")); // a repeat
  await storeCount(db, c("e=play/windows&v=0.9.11&s=aaaaaaaa&n=3&t=900&mc=500&mw=250"), at("11"));
  await storeCount(db, c("e=play/windows&v=0.9.11&s=aaaaaaaa&n=2&t=600&mc=400"), at("11")); // late: nothing added
  await storeCount(db, c("e=play/windows&v=0.9.11&s=aaaaaaaa&n=0&t=900&mc=500&mw=250"), at("12")); // re-sent at the next start
  await storeCount(db, c("e=play/web&v=0.9.11&s=bbbbbbbb&n=1&t=120"), at("12")); // single player: no multiplayer seconds
  const rows = db.raw.prepare("SELECT hour, mode, seconds AS s FROM mp_hourly ORDER BY hour, mode").all();
  assert.deepEqual(rows.map((r) => [r.hour.slice(11), r.mode, r.s]), [["10", "club", 200], ["11", "challenge", 250], ["11", "club", 300]]);
  const all = db.raw.prepare("SELECT SUM(seconds) AS s FROM seconds_hourly").get();
  assert.equal(all.s, 900 + 120, "the multiplayer seconds are part of the play time, not added to it");

  const t = await mpPlay(db, new Date("2026-11-28T13:00:00Z"), (900 + 120) / 3600);
  assert.equal(t.since, "2026-11-28T10");
  assert.equal(t.hours_7d, 750 / 3600);
  assert.ok(Math.abs(t.share_7d - 750 / 1020) < 1e-9);
  assert.deepEqual(t.modes.map((m) => [m.mode, m.hours * 3600]), [["club", 500], ["league", 0], ["challenge", 250]]);
  const later = await mpPlay(db, new Date("2026-12-10T13:00:00Z"), 0);
  assert.equal(later.hours_7d, 0);
  assert.equal(later.hours, 750 / 3600);

  // the page shows it in the Multiplayer section
  const s = await allStats({ DB: db, BOARD: null }, new Date("2026-11-28T13:00:00Z"));
  const html = page(s);
  assert.ok(html.includes("Multiplayer play time"));
  assert.ok(html.includes("Shared clubs") && html.includes("0.1 h"));
});

test("a count still goes in without the multiplayer tables", async () => {
  const db = memoryDb();
  db.raw.exec("DROP TABLE run_mp; DROP TABLE mp_hourly;");
  await storeCount(db, readCount(u("e=play/linux&v=0.9.11&s=aaaaaaaa&n=1&t=300&mc=200"), "GB"), new Date("2026-11-28T10:00:00Z"));
  assert.equal(db.raw.prepare("SELECT SUM(seconds) AS s FROM seconds_hourly").get().s, 300);
  const s = await allStats({ DB: db, BOARD: null }, new Date("2026-11-28T13:00:00Z"));
  assert.ok(s.mp_play.error);
  assert.ok(page(s).includes("Multiplayer play time"));
});
