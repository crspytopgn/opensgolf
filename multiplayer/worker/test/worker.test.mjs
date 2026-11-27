import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { memoryDb } from "./d1shim.mjs";
import worker, { readResult, storeResult, board, hide, authorised, BOARD_SIZE } from "../src/index.js";
import { challenge, weekId, fnv1a } from "../src/challenge.js";
import { cleanNick, generatedNick } from "../src/names.js";

const vectors = JSON.parse(readFileSync(new URL("./vectors.json", import.meta.url), "utf8"));
const NOW = new Date("2026-11-18T12:00:00Z"); // 2026-W47

const facts = (over = {}) => ({ year: 2002, cash: 50000, holes: 9, fun: 120, skill: 1250, members: 40, rank: 1, ...over });
const body = (over = {}) => ({
  protocol: 1, challenge: "2026-W47", player: "0123456789abcdef", nick: "Michael", group: "", platform: "macos-arm64",
  version: "0.9.0", cheats: false, facts: facts(), ...over,
});

test("the weekly challenge matches the shared vectors", () => {
  for (const [day, id] of Object.entries(vectors.weeks)) assert.equal(weekId(new Date(`${day}T12:00:00Z`)), id, day);
  for (const c of vectors.challenges) assert.deepEqual(challenge(c.id), c);
  assert.equal(fnv1a(""), 0x811c9dc5);
  assert.equal(fnv1a("a"), 0xe40c292c);
});

test("only sound results are read", () => {
  const r = readResult(body(), NOW);
  assert.equal(r.score, 50000); // 2026-W47 is a cash week
  assert.equal(readResult(body({ challenge: "2026-W46" }), NOW).challenge, "2026-W46"); // last week's still open
  const bad = {
    "not an open challenge": [{ challenge: "2026-W45" }, { challenge: "2026-W48" }, { challenge: "nope" }],
    "bad player id": [{ player: "short" }, { player: "0123456789ABCDEF" }],
    "bad group code": [{ group: "abc123" }, { group: "TOOLONG1" }],
    "bad platform": [{ platform: "amiga" }],
    "bad version": [{ version: "1.0" }],
    "cheats were on": [{ cheats: true }, { cheats: undefined }],
    "bad facts": [{ facts: facts({ rank: 4 }) }, { facts: facts({ cash: 1.5 }) }, { facts: facts({ holes: 99 }) }],
    "the challenge's year is not over": [{ facts: facts({ year: 2001 }) }],
    "not the challenge's year end": [{ facts: facts({ year: 2003 }) }],
    "more than 18 holes": [{ facts: facts({ holes: 19 }) }],
    "update the game": [{ protocol: 2 }, { protocol: undefined }],
    "bad nickname": [{ nick: "ab" }, { nick: "x".repeat(17) }, { nick: "<script>" }, { nick: "sh1t head" }],
  };
  for (const [why, list] of Object.entries(bad)) for (const o of list) assert.equal(readResult(body(o), NOW), why, JSON.stringify(o));
  assert.equal(readResult(null, NOW), "not a result");
  assert.equal(readResult(body({ version: "0.8.9" }), NOW, "0.9.0"), "update the game");
  assert.equal(readResult(body({ version: "0.10.0" }), NOW, "0.9.0").version, "0.10.0");
});

test("nicknames: tidied, blocked words caught through look-alikes, hidden ones get a stable generated name", () => {
  assert.equal(cleanNick("  Big   Hitter "), "Big Hitter");
  assert.equal(cleanNick("F.U.C.K"), null);
  assert.equal(cleanNick("Scunthorpe Golf"), null); // the cost of a short list: the admin page can't un-block, so kept simple
  assert.equal(cleanNick("Par_3-Master.99"), "Par_3-Master.99");
  assert.equal(generatedNick("0123456789abcdef"), generatedNick("0123456789abcdef"));
  assert.notEqual(generatedNick("0123456789abcdef"), generatedNick("fedcba9876543210"));
});

test("only a better score replaces a player's result", async () => {
  const db = memoryDb();
  assert.equal(await storeResult(db, readResult(body(), NOW), NOW), true);
  assert.equal(await storeResult(db, readResult(body({ facts: facts({ cash: 40000 }) }), NOW), NOW), false);
  assert.equal(await storeResult(db, readResult(body({ facts: facts({ cash: 50000 }) }), NOW), NOW), false);
  assert.equal(await storeResult(db, readResult(body({ facts: facts({ cash: 70000 }), nick: "Mike" }), NOW), NOW), true);
  const rows = db.raw.prepare("SELECT nick, score FROM results").all();
  assert.deepEqual(rows.map((r) => [r.nick, r.score]), [["Mike", 70000]]);
});

test("boards: sorted, per group, the player's own place beyond the top rows, hidden rows and names", async () => {
  const db = memoryDb();
  const player = (i) => i.toString(16).padStart(16, "0");
  for (let i = 1; i <= BOARD_SIZE + 5; i++) {
    const t = new Date(NOW.getTime() + i * 1000);
    await storeResult(db, readResult(body({ player: player(i), nick: `Player ${i}`, facts: facts({ cash: i * 100 }) }), t), t);
  }
  await storeResult(db, readResult(body({ player: player(999), group: "FRIEND", facts: facts({ cash: 1 }) }), NOW), NOW);
  let b = await board(db, "2026-W47", "", player(1));
  assert.equal(b.rows.length, BOARD_SIZE);
  assert.equal(b.rows[0].nick, `Player ${BOARD_SIZE + 5}`);
  assert.deepEqual(b.me, { place: BOARD_SIZE + 5, score: 100 });
  b = await board(db, "2026-W47", "FRIEND", player(999));
  assert.equal(b.rows.length, 1);
  assert.equal(b.rows[0].mine, true);
  assert.equal(b.me, null);

  await hide(db, { challenge: "2026-W47", player: player(BOARD_SIZE + 5) }, NOW);
  await hide(db, { nick: "player 54" }, NOW); // any case
  b = await board(db, "2026-W47", "", "");
  assert.equal(b.rows[0].nick, generatedNick(player(54)));
  assert.equal(b.rows[0].score, 5400);
  await hide(db, { nick: "Player 54", show: true }, NOW);
  b = await board(db, "2026-W47", "", "");
  assert.equal(b.rows[0].nick, "Player 54");
});

test("boards: one place per nickname, whichever install sent it", async () => {
  const db = memoryDb();
  const send = async (player, nick, cash, s) => {
    const t = new Date(NOW.getTime() + s * 1000);
    await storeResult(db, readResult(body({ player, nick, facts: facts({ cash }) }), t), t);
  };
  const a1 = "aaaaaaaaaaaaaaa1", a2 = "aaaaaaaaaaaaaaa2", a3 = "aaaaaaaaaaaaaaa3", b = "bbbbbbbbbbbbbbbb";
  await send(a1, "RuneAldoo", 300, 1);
  await send(a2, "runealdoo", 500, 2); // another browser, other capitals: the same name
  await send(a3, "RuneAldoo", 100, 3);
  await send(b, "Vannes", 400, 4);
  // two strangers who never typed a name may share the game's "Golfer 123": both stay
  await send("ccccccccccccccc1", "Golfer 123", 50, 5);
  await send("ccccccccccccccc2", "Golfer 123", 40, 6);
  let r = await board(db, "2026-W47", "", a3);
  assert.deepEqual(r.rows.map((x) => [x.nick, x.score, x.mine]),
    [["runealdoo", 500, true], ["Vannes", 400, false], ["Golfer 123", 50, false], ["Golfer 123", 40, false]]);
  assert.equal(r.me, null);
  // the place beyond the shown rows counts each one once
  r = await board(db, "2026-W47", "", "ccccccccccccccc2");
  assert.deepEqual(r.rows.filter((x) => x.mine).map((x) => x.score), [40]);
  // an admin hiding the best row brings back that name's next best
  await hide(db, { challenge: "2026-W47", player: a2 }, NOW);
  r = await board(db, "2026-W47", "", "");
  assert.deepEqual(r.rows.slice(0, 2).map((x) => [x.nick, x.score]), [["Vannes", 400], ["RuneAldoo", 300]]);
});

test("admin needs the password", () => {
  const req = (auth) => new Request("https://x.test/admin", { headers: auth ? { Authorization: auth } : {} });
  const env = { ADMIN_PASSWORD: "secret" };
  assert.equal(authorised(req(`Basic ${btoa("any:secret")}`), env), true);
  assert.equal(authorised(req(`Basic ${btoa("any:wrong!")}`), env), false);
  assert.equal(authorised(req(null), env), false);
  assert.equal(authorised(req(`Basic ${btoa("any:secret")}`), {}), false);
});

test("the Worker's routes", async () => {
  const env = { DB: memoryDb(), ADMIN_PASSWORD: "secret" };
  const call = (path, init) => worker.fetch(new Request(`https://x.test${path}`, init), env);
  const wk = weekId(new Date());
  const post = (b) => call("/result", { method: "POST", body: JSON.stringify(b), headers: { "Content-Type": "application/json" } });
  let r = await post(body({ challenge: wk }));
  assert.equal(r.status, 200);
  assert.deepEqual(await r.json(), { ok: true, better: true });
  r = await post(body({ challenge: wk, cheats: true }));
  assert.equal(r.status, 400);
  const b64 = Buffer.from(JSON.stringify(body({ challenge: wk, player: "fedcba9876543210", nick: "Someone" }))).toString("base64url");
  r = await call(`/result?d=${b64}`);
  assert.deepEqual(await r.json(), { ok: true, better: true });
  r = await call("/result?d=not-json");
  assert.equal(r.status, 400);
  r = await call("/result", { method: "POST", body: "x".repeat(5000) });
  assert.equal(r.status, 400);
  r = await call(`/board?c=${wk}`);
  assert.equal((await r.json()).rows.length, 2);
  assert.equal(env.DB.raw.prepare(`SELECT SUM(n) AS n FROM activity WHERE mode = 'challenge' AND place = ?`).get(wk).n, 2);
  assert.match(r.headers.get("Cache-Control"), /max-age=60/);
  r = await call("/board?c=bad");
  assert.equal(r.status, 400);
  r = await call("/challenge");
  assert.equal((await r.json()).challenge.id, wk);
  r = await call("/badge");
  const badge = await r.json();
  assert.equal(badge.schemaVersion, 1);
  assert.match(badge.message, / · 2 playing$/);
  r = await call("/admin");
  assert.equal(r.status, 401);
  r = await call("/admin", { headers: { Authorization: `Basic ${btoa("x:secret")}` } });
  assert.equal((await r.json()).results.length, 2);
  r = await call("/nope");
  assert.equal(r.status, 404);
});
