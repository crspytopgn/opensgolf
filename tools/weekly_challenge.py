#!/usr/bin/env python3
"""The weekly challenge's posts in the community Discord (.github/workflows/weekly-challenge.yml).

  weekly_challenge.py new      Monday: this week's challenge, and last week's top 3
  weekly_challenge.py leaders  this week's leaders so far (by hand only)
  weekly_challenge.py test     one short line, to check the webhook works
  weekly_challenge.py text     this week's challenge as plain text (for Reddit or anywhere else)

Prints a Discord webhook payload (JSON) for new, leaders and test. Everything comes from the multiplayer Worker's public
/board (OPENSGOLF_BOARD_URL overrides it), so the post always says what the game and the board say. Nicknames are shown
as typed and never ping anyone.

Copyright the OpenSGolf contributors. GPL-2.0-or-later.
"""

import datetime
import json
import os
import sys
import urllib.request

BOARD = os.environ.get("OPENSGOLF_BOARD_URL", "https://opensgolf-board.openmcm2-laps.workers.dev").rstrip("/")
PAGE = "https://crspytopgn.github.io/opensgolf/weekly.html"
PLAY = "https://crspytopgn.github.io/opensgolf/"
DOWNLOAD = "https://github.com/crspytopgn/opensgolf/releases/latest"

# the same words as the game (multiplayer/lang/en.txt STR_MP_GOAL_*, STR_MP_KIND_*)
GOALS = {
    "cash": "the most cash",
    "fun": "the highest fun rating",
    "members": "the most members",
    "skill": "the highest skill rating",
}
MEASURES = {"cash": "Cash", "fun": "Fun", "members": "Members", "skill": "Skill"}
KINDS = {
    "budget": "Twist: you start with half the usual money.",
    "nine": "Twist: your course may have 9 holes at most.",
}
MEDALS = ["🥇", "🥈", "🥉"]


def week_id(d):
    y, w, _ = d.isocalendar()
    return f"{y}-W{w:02d}"


def fetch_board(week):
    req = urllib.request.Request(f"{BOARD}/board?c={week}", headers={"User-Agent": "opensgolf-weekly"})
    with urllib.request.urlopen(req, timeout=30) as r:
        b = json.load(r)
    if not b.get("ok"):
        raise SystemExit(f"the board answered: {b}")
    return b


def score_text(measure, n):
    # as the game shows it (crates/simgolf/src/mp/lobby.rs score_text): the club keeps skill in hundredths
    if measure == "skill":
        return f"{n / 100:.2f}"
    if measure == "cash":
        return f"${n:,}" if n >= 0 else f"-${-n:,}"
    return f"{n:,}"


def rows_text(b, count):
    m = b["challenge"]["measure"]
    lines = []
    for r in b["rows"][:count]:
        mark = MEDALS[r["place"] - 1] if r["place"] <= 3 else f"{r['place']}."
        lines.append(f"{mark} **{clean(r['nick'])}**: {MEASURES[m]} {score_text(m, r['score'])}")
    return "\n".join(lines)


def clean(nick):
    # a nickname is shown as text: no Markdown or links of its own
    return "".join(c for c in str(nick) if c.isalnum() or c in " -_.")[:24] or "anon"


def describe(c):
    land = f"{c['theme'].capitalize()} land, {c['difficulty']} difficulty."
    lines = [f"Goal: have **{GOALS[c['measure']]}** when {c['endYear']} ends.", land]
    if c["kind"] in KINDS:
        lines.append(KINDS[c["kind"]])
    return lines


def how_to_play():
    return (f"Everyone gets the same land and the same start. Play it in the game: **Start New Game > Multiplayer > "
            f"Weekly challenge**. [Play in your browser]({PLAY}) or [download]({DOWNLOAD}).")


def players(n):
    # the board lists the top 50 (multiplayer/worker/src/index.js BOARD_SIZE)
    return "50+ players" if n >= 50 else f"{n} player{'s' if n != 1 else ''}"


def week_end_text(now):
    monday = now.date() - datetime.timedelta(days=now.weekday())
    sunday = monday + datetime.timedelta(days=6)
    return f"The week runs until the end of Sunday {sunday.day} {sunday.strftime('%B')} (UTC)."


def payload(embeds):
    return {"username": "OpenSGolf Weekly Challenge", "allowed_mentions": {"parse": []}, "embeds": embeds}


def new_week(now):
    this = fetch_board(week_id(now))
    last_week = week_id(now - datetime.timedelta(days=7))
    c = this["challenge"]
    embeds = [{
        "title": f"⛳ This week's challenge ({c['id']})",
        "url": PAGE,
        "description": "\n".join(describe(c) + ["", how_to_play(), week_end_text(now)]),
        "color": 0x2F8F4E,
    }]
    last = fetch_board(last_week)
    if last["rows"]:
        lc = last["challenge"]
        n = len(last["rows"])
        embeds.append({
            "title": f"🏆 Last week ({lc['id']}): {GOALS[lc['measure']]}",
            "url": f"{PAGE}?c={lc['id']}",
            "description": rows_text(last, 3) + f"\n\n{players(n)} on the board. Well played!",
            "color": 0xE0A800,
        })
    return payload(embeds)


def leaders(now):
    b = fetch_board(week_id(now))
    c = b["challenge"]
    if b["rows"]:
        n = len(b["rows"])
        body = rows_text(b, 5) + f"\n\n{players(n)} so far. Still time to beat them! " + week_end_text(now)
    else:
        body = "Nobody is on the board yet this week: be the first!"
    body += "\n\n" + "\n".join(describe(c)) + "\n" + how_to_play()
    return payload([{
        "title": f"📊 Leaders so far ({c['id']})",
        "url": PAGE,
        "description": body,
        "color": 0x5865F2,
    }])


def text(now):
    c = fetch_board(week_id(now))["challenge"]
    return "\n".join([f"OpenSGolf weekly challenge {c['id']}"] + [s.replace("**", "") for s in describe(c)]
                     + ["", "Same land and start for everyone. Start New Game > Multiplayer > Weekly challenge.",
                        f"Board: {PAGE}"])


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    now = datetime.datetime.now(datetime.timezone.utc)
    if os.environ.get("OPENSGOLF_NOW"):  # for trying other weeks: an ISO date
        now = datetime.datetime.fromisoformat(os.environ["OPENSGOLF_NOW"]).replace(tzinfo=datetime.timezone.utc)
    if mode == "new":
        print(json.dumps(new_week(now)))
    elif mode == "test":
        print(json.dumps({"username": "OpenSGolf Weekly Challenge", "allowed_mentions": {"parse": []},
                          "content": "✅ Connected. Each Monday at midday (UK time) the new weekly challenge and last week's top 3 will be posted here."}))
    elif mode == "leaders":
        print(json.dumps(leaders(now)))
    elif mode == "text":
        print(text(now))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
