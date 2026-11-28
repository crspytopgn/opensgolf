# Play-time stats

A Cloudflare Worker with a D1 database that keeps OpenSGolf's anonymous play-time count (docs/PLAYING.md "Play-time
count", crates/simgolf/src/playcount.rs) and shows it on a password-protected page.

* `GET /count?e=<event>&v=<version>&s=<run id>&n=<count>&t=<seconds>`: what the game sends. `e` is
  `start/<platform>/<version>` once at start, and `play/<platform>` with `t` the whole seconds played in the run so far,
  every 5 minutes of play and when the game is left or quit. `t` only grows within a run: the Worker keeps the highest per
  run (`run_time`) and adds only the increase to that hour (`seconds_hourly`), so lost or repeated counts change nothing and
  the total is exact to the second. Builds up to 0.8.7 send `play-15min/<platform>` for each 15 minutes instead, still
  taken (`play_hourly`, 900 s each). `s` is a random id made at each start (or `kept` for an older build's unsent marks,
  sent once as seconds); `n` the count's number in that run. Only the country Cloudflare reads from the address is kept,
  never the address.
  Builds after 0.9.10 add `mc`, `ml` and `mw` to a play count when some of `t` was multiplayer: its seconds in a shared
  club, a Neighbours league and the weekly challenge (each no more than `t`). Kept as `t` is, per run and mode (`run_mp`),
  only the increase added to the hour (`mp_hourly`); part of the play time, not added to it. The Multiplayer section shows
  them as hours per mode. These tables come from `schema.sql`: run it again (below) before deploying this.
* `GET /` and `GET /api/stats`: the stats (basic auth, any user name, the `STATS_PASSWORD` secret). The Multiplayer section
  (`src/mp.js`) reads the multiplayer server's database (`multiplayer/worker`, its `activity`, `clubs`, `leagues` and `results`
  tables), bound as `BOARD`: counts only, never player ids, nicknames, club names or league codes. Players count on a day they
  join a shared club, make, join or report to a Neighbours league, or send a weekly challenge result.
* Daily at 04:00 UTC (the cron in `wrangler.toml`) and `POST /admin/import[?from=YYYY-MM-DD]`: GoatCounter's counts from builds
  before the switch, by hour (needs the `GOATCOUNTER_TOKEN` secret). A failed GoatCounter call is tried again three
  times over about 2.5 minutes before the run is logged as failed. The first import takes everything since 1 Oct 2026;
  later ones re-read the last 15 days, replacing those hours. The page says when older builds have stayed under an hour a
  week for three full weeks: then remove the cron and deploy again.

## Deploy

```sh
cd stats
npx wrangler@4.152.0 d1 create opensgolf-stats        # once; put the id in wrangler.toml
npx wrangler@4.152.0 d1 execute opensgolf-stats --remote --file schema.sql   # again after it changes: it only adds
npx wrangler@4.152.0 secret put GOATCOUNTER_TOKEN
npx wrangler@4.152.0 secret put STATS_PASSWORD
npx wrangler@4.152.0 deploy
```

## Tests

`node --test stats/test/*.test.mjs` (Node 22.5 or later: the tests stand D1 in with node:sqlite).
