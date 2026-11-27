// The weekly challenge on the website (the port's own): this week's goal and board, read from the multiplayer Worker's
// public /board (multiplayer/worker), for the start page's "This week's challenge" box (index.html) and the board page
// (weekly.html). The words match the game's (multiplayer/lang/en.txt) and the Discord posts' (tools/weekly_challenge.py).
"use strict";
var SG_BOARD = "https://opensgolf-board.openmcm2-laps.workers.dev";
// the first week with a board (crates/simgolf/src/mp/lobby.rs FIRST_WEEK)
var SG_FIRST_WEEK = "2026-W48";
var SG_GOALS = { cash: "the most cash", fun: "the highest fun rating", members: "the most members", skill: "the highest skill rating" };
var SG_MEASURES = { cash: "Cash", fun: "Fun", members: "Members", skill: "Skill" };
var SG_KINDS = { budget: "Twist: you start with half the usual money.", nine: "Twist: your course may have 9 holes at most." };

// The ISO week id ('YYYY-Www') of a Date, in UTC (multiplayer/worker/src/challenge.js weekId).
function sgWeekId(d) {
    var t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()));
    var day = t.getUTCDay() || 7;
    t.setUTCDate(t.getUTCDate() + 4 - day);
    var year = t.getUTCFullYear();
    var week = Math.floor((t - Date.UTC(year, 0, 1)) / 86400000 / 7) + 1;
    return year + "-W" + (week < 10 ? "0" : "") + week;
}

// The Sunday that ends a week id, as a Date (UTC midnight of the Monday after).
function sgWeekEnd(id) {
    var year = +id.slice(0, 4), week = +id.slice(6);
    var jan4 = new Date(Date.UTC(year, 0, 4));
    var monday = Date.UTC(year, 0, 4) - ((jan4.getUTCDay() || 7) - 1) * 86400000 + 7 * (week - 1) * 86400000;
    return new Date(monday + 7 * 86400000);
}

// The week ids from the first board to this week, newest first.
function sgWeeks(now) {
    var out = [], d = new Date(now.getTime());
    for (var i = 0; i < 520; i++) {
        var id = sgWeekId(d);
        out.push(id);
        if (id === SG_FIRST_WEEK) break;
        d = new Date(d.getTime() - 7 * 86400000);
    }
    return out;
}

// A score as the game shows it (crates/simgolf/src/mp/lobby.rs score_text): the club keeps skill in hundredths.
function sgScore(measure, n) {
    if (measure === "skill") return (n / 100).toFixed(2);
    var s = Math.abs(n).toLocaleString("en-GB");
    return measure === "cash" ? (n < 0 ? "-$" : "$") + s : (n < 0 ? "-" : "") + s;
}

// "3 players"; the board lists the top 50 (multiplayer/worker/src/index.js BOARD_SIZE)
function sgPlayers(n) { return n >= 50 ? "50+ players" : n + " player" + (n === 1 ? "" : "s"); }

function sgCap(s) { return s.charAt(0).toUpperCase() + s.slice(1); }

// The challenge's lines: the goal, the land and difficulty, and the week's twist if any.
function sgDescribe(c) {
    var lines = ["Have <b>" + SG_GOALS[c.measure] + "</b> when " + c.endYear + " ends.",
        sgCap(c.theme) + " land, " + c.difficulty + " difficulty."];
    if (SG_KINDS[c.kind]) lines.push(SG_KINDS[c.kind]);
    return lines;
}

// How long is left of this week, in words ("3 days", "5 hours").
function sgTimeLeft(id, now) {
    var ms = sgWeekEnd(id) - now;
    if (ms <= 0) return "";
    var h = Math.floor(ms / 3600000);
    if (h >= 48) return Math.floor(h / 24) + " days";
    if (h >= 1) return h + " hour" + (h === 1 ? "" : "s");
    return "under an hour";
}

// The board of a week (and group), as the Worker gives it: { challenge, rows }.
function sgBoard(week, group) {
    var q = "?c=" + encodeURIComponent(week) + (group ? "&g=" + encodeURIComponent(group) : "");
    return fetch(SG_BOARD + "/board" + q).then(function (r) { return r.json(); }).then(function (b) {
        if (!b || !b.ok) throw new Error("the board answered " + JSON.stringify(b));
        return b;
    });
}

// A table of a board's rows into `table` (its <tbody>), `count` rows at most (0: all).
function sgFillRows(tbody, b, count) {
    var m = b.challenge.measure, medals = ["🥇", "🥈", "🥉"];
    tbody.textContent = "";
    b.rows.slice(0, count || b.rows.length).forEach(function (r) {
        var tr = document.createElement("tr");
        [r.place <= 3 ? medals[r.place - 1] : r.place + ".", r.nick, sgScore(m, r.score), r.holes].forEach(function (v, i) {
            var td = document.createElement("td");
            td.textContent = v; // nicknames are shown as typed text, never as markup
            if (i === 0) td.className = "place";
            if (i === 2) td.className = "score";
            tr.appendChild(td);
        });
        tbody.appendChild(tr);
    });
}

if (typeof module !== "undefined") module.exports = { sgWeekId: sgWeekId, sgWeekEnd: sgWeekEnd, sgWeeks: sgWeeks, sgScore: sgScore, sgTimeLeft: sgTimeLeft };
