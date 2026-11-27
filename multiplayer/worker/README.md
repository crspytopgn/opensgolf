# The multiplayer server

A Cloudflare Worker with a D1 database that keeps the weekly rival-courses challenge's board. Part of the multiplayer
work in `multiplayer/`. Deployed at https://opensgolf-board.openmcm2-laps.workers.dev.

* **The weekly challenge** is worked out from the ISO week's id alone (`src/challenge.js`): the measure the board sorts
  by (cash, fun, members or skill at the end of the first game year), the course theme, the difficulty and the new game's
  seed. The game has the same function in Rust; both are tested against `test/vectors.json`, so they cannot drift apart.
  `GET /challenge` gives this week's, but the game never needs to ask.
* `POST /result`: `{protocol, challenge, player, nick, group, platform, version, cheats, facts}`. `player` is a random id the
  game makes once (16 to 32 hex digits); `group` a friends' code of 6 capitals or digits, or `""` for the public board;
  `facts` the year, cash, holes, fun, skill (hundredths), members and rank at the end of the challenge's year. Results
  are taken for this week's challenge and last week's. `GET /result?d=<the same JSON, base64url>` does the same for
  the game's own HTTP client, which only sends GETs, never with cheats on, and only with this Worker's `protocol` (1) and a game version at least the
  `MIN_VERSION` variable, when set; otherwise the game is told to update. Only a player's best of the week is kept, so
  a worse result writes nothing.
* `GET /board?c=<week>&g=<group>&p=<player>`: the top 50 and the asking player's own place. Without `p` the answer may be
  cached for a minute. Each nickname (any capitals) has one place, its best, so one person sending from several installs
  (another browser, a cleared save) can't fill the board; a hidden nickname, or the game's own "Golfer 123", counts by
  player id instead.
* `GET /admin` and `POST /admin/hide` (basic auth, any user name, the `ADMIN_PASSWORD` secret): the latest results, and
  hiding a result (`{challenge, player}`) or a nickname everywhere (`{nick}`; `show: true` undoes either). A hidden
  nickname shows as a generated one (`src/names.js`).

For the play-time stats page's Multiplayer section, `src/activity.js` counts, per day, each player's club joins, league
visits (made, joined, reported to) and results sent, in the `activity` table (day, mode, club id / league code / week,
player id, count). The stats page reads it and shows only totals.

Kept per result: the player's random id, nickname, group code, the numbers above, the platform and the version. No
address, no country, no cookies.

## Free-plan use

One result is at most one row written (two with the index); a board view reads about 50 rows. The Worker's requests
count towards the account's 100,000 a day, shared with the play-time stats Worker.

## Deploy (when it is time)

```sh
cd multiplayer/worker
npx wrangler@4.152.0 d1 create opensgolf-board        # once; put the id in wrangler.toml
npx wrangler@4.152.0 d1 execute opensgolf-board --remote --file schema.sql
npx wrangler@4.152.0 secret put ADMIN_PASSWORD
npx wrangler@4.152.0 deploy
```

## Tests

`node --test multiplayer/worker/test/*.test.mjs` (Node 22.5 or later: the tests stand D1 in with node:sqlite).

## Shared clubs

The same Worker serves the shared courses: `src/club.js` is a Durable Object per club (SQLite storage, hibernating
WebSockets, so a club with nobody on costs nothing), `src/room.js` its clock and the order of actions, the same logic as
the game's `crates/simgolf/src/mp/session.rs` Room and tested against the same vectors (`test/room_vectors.json`, written by
the Rust test `the_server_room_matches_the_vectors` with `SG_WRITE_VECTORS=1`).

* `GET /clubs`: the listed clubs, those with someone on first (clubs nobody has played for 60 days drop off).
* `GET /clubs/new?d=<JSON, base64url>`: `{protocol, player, nick, name, compat, listed}` makes a club; the answer is its id.
* `GET /clubs/<id>/ws`: the club's WebSocket. Messages are the game's `ToServer` and `ToGame` as serde JSON. A game says
  `Hello` with its `Compat`; one that differs is refused, naming what differs. A new club starts from its owner's game
  (the room asks for a save); a joiner is sent the latest save and every action since it. The longest-on player is asked
  for a new save every 2,000 ticks, which trims the log. Nobody on: the clock stops, and it goes on at the same speed when
  someone joins. Hashes sent with `Check` are compared, and a game whose hash differs is told to `Reload`.
* `GET /clubs/<id>/state` (admin password): the club's record, clock, save tick and who is on.

Free-plan use: each WebSocket message is a small part of a request (20 to 1), and a hibernating club uses no time. A club
of 4 playing for an hour sends a few thousand messages.

Local check: `npx wrangler@4.152.0 d1 execute opensgolf-board --local --file schema.sql`, `npx wrangler@4.152.0 dev`, then
`node test/live.mjs http://127.0.0.1:8787`.
