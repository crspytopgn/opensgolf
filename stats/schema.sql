-- The play-time stats database (stats/README.md). Safe to run again: everything is IF NOT EXISTS.

-- One row per launch of a game that reports here: `sid` is a random number the game makes at start and never saves, so a
-- launch can be told from another but not linked to a player. `beats` is the number of 15-minute marks reached.
CREATE TABLE IF NOT EXISTS runs (
  sid        TEXT PRIMARY KEY,
  platform   TEXT NOT NULL,
  version    TEXT NOT NULL,
  country    TEXT NOT NULL DEFAULT '',
  started_at TEXT NOT NULL,          -- ISO time (UTC) of the first count of the run
  last_at    TEXT NOT NULL,          -- ISO time (UTC) of the latest count
  beats      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS runs_started ON runs (started_at);

-- 15-minute marks per hour (UTC), from every source that reports here. `kept` counts are marks that could not be sent at
-- the time (no internet) and came later with another run: they belong to no run.
CREATE TABLE IF NOT EXISTS play_hourly (
  hour     TEXT NOT NULL,            -- 'YYYY-MM-DDTHH' (UTC)
  platform TEXT NOT NULL,
  version  TEXT NOT NULL,
  country  TEXT NOT NULL DEFAULT '',
  kept     INTEGER NOT NULL DEFAULT 0,
  beats    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (hour, platform, version, country, kept)
);

-- GoatCounter's counts per hour and event path (play-15min/<platform>, start/<platform>/<version>), as older builds
-- reported them. Re-imported hours replace what was there, so an import can be run again safely.
CREATE TABLE IF NOT EXISTS gc_hourly (
  hour  TEXT NOT NULL,               -- 'YYYY-MM-DDTHH' as GoatCounter gives it (the site's time zone, Europe/London)
  path  TEXT NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (hour, path)
);

-- Each GoatCounter import.
CREATE TABLE IF NOT EXISTS gc_imports (
  at         TEXT NOT NULL,          -- when it ran (UTC)
  start_day  TEXT NOT NULL,
  end_day    TEXT NOT NULL,
  rows       INTEGER NOT NULL,
  play_marks INTEGER NOT NULL,       -- play-15min counts in the window
  error      TEXT
);

-- Exact play time (builds after 0.8.7): `play/<platform>` counts carry `t`, the whole seconds played in the run so far.
-- `t` only grows within a run, so the highest one seen is the run's play time: a repeat or a lost count changes nothing,
-- and only the increase over the last one is added to the hour it arrived in.
CREATE TABLE IF NOT EXISTS run_time (
  sid     TEXT PRIMARY KEY,          -- the run (runs.sid)
  seconds INTEGER NOT NULL DEFAULT 0
);

-- Seconds of play per hour (UTC). `kept` seconds were saved by an older build as unsent 15-minute marks (900 s each) and
-- came later with no run.
CREATE TABLE IF NOT EXISTS seconds_hourly (
  hour     TEXT NOT NULL,            -- 'YYYY-MM-DDTHH' (UTC)
  platform TEXT NOT NULL,
  version  TEXT NOT NULL,
  country  TEXT NOT NULL DEFAULT '',
  kept     INTEGER NOT NULL DEFAULT 0,
  seconds  INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (hour, platform, version, country, kept)
);

-- Multiplayer play time (builds after 0.9.10): play counts also carry `mc`, `ml` and `mw`, the whole seconds of the run so
-- far played in a shared club, a Neighbours league and the weekly challenge. Each only grows within a run, kept as `t` is:
-- the highest seen per run and mode, only the increase added to the hour it arrived in. Part of the play time above, not
-- added to it.
CREATE TABLE IF NOT EXISTS run_mp (
  sid     TEXT NOT NULL,             -- the run (runs.sid)
  mode    TEXT NOT NULL,             -- club, league, challenge
  seconds INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (sid, mode)
);

CREATE TABLE IF NOT EXISTS mp_hourly (
  hour     TEXT NOT NULL,            -- 'YYYY-MM-DDTHH' (UTC)
  platform TEXT NOT NULL,
  version  TEXT NOT NULL,
  mode     TEXT NOT NULL,            -- club, league, challenge
  seconds  INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (hour, platform, version, mode)
);
