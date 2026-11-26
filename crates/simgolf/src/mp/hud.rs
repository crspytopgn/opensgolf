//! The multiplayer reminder on the course (the port's own): a small box under the name plate saying what the game being
//! played is about, so a player coming back to it can see the challenge again.
//!
//! * Weekly challenge: the week, the goal and its end, the week's twist (half money, nine holes), the months left, the
//!   player's score now and where it would place on the week's board (fetched when the game starts and every few
//!   minutes; nothing is sent but the usual board request).
//! * Neighbours: the league's name, its goal and season end, the months left and the player's place in its table.
//! * Shared club: the club's name and who is on.
//!
//! A click opens it out to the board's top rows (weekly), the league's table (neighbours) or the players (club), and
//! another click folds it again. It hides while a tournament's leader board has that corner.
use super::challenge::{Challenge, Measure};
use super::{league, lobby, rival, settings};
use crate::app::{App, Screen};
use crate::gfx::Gfx;
use crate::info_ui::c15;
use crate::render::Rect;
use crate::ui::{Screen as Ui, F_ARIAL10};
use sg_core::lang::{tr, trf};
use std::cell::RefCell;

/// The box's left edge, top and width (under the name plate, in 800x600 units).
const X: f32 = 8.0;
const Y: f32 = 66.0;
const W: f32 = 250.0;
const LINE_H: f32 = 13.0;
/// Seconds between fetches of the board or the table while playing.
const REFRESH_S: f64 = 180.0;
/// Rows shown opened out.
const OPEN_ROWS: usize = 5;

/// What the reminder knows from the server: the week's board, or the league's table.
#[derive(Default)]
struct Known {
    /// The game it is for (the scenario id), so a new game fetches again.
    game: String,
    board: Option<lobby::Board>,
    table: Option<league::Table>,
    asked_at: f64,
    request: Option<crate::net::Request>,
    open: bool,
}

thread_local! {
    static KNOWN: RefCell<Known> = RefCell::new(Known::default());
}

/// The months of the game left before the end of `end_year` (the game's year has 8 months, March to October).
pub fn months_left(month_index: i32, end_year: i32) -> i32 {
    ((end_year - 2001 + 1) * sg_core::economy::MONTHS_PER_YEAR - month_index).max(0)
}

/// "5 months left", "1 month left", or that it is over.
fn left_text(n: i32) -> String {
    match n {
        0 => tr("STR_MP_HUD_OVER").into(),
        1 => tr("STR_MP_HUD_ONE_MONTH").into(),
        _ => trf("STR_MP_HUD_MONTHS", &[("N", &n)]),
    }
}

/// The goal in a few words ("highest fun").
pub(super) fn goal_text(m: Measure) -> &'static str {
    match m {
        Measure::Cash => tr("STR_MP_HUD_GOAL_CASH"),
        Measure::Fun => tr("STR_MP_HUD_GOAL_FUN"),
        Measure::Members => tr("STR_MP_HUD_GOAL_MEMBERS"),
        Measure::Skill => tr("STR_MP_HUD_GOAL_SKILL"),
    }
}

/// Where a score would place among the board's rows (the player's own row left out): 1 is first.
pub fn would_place(rows: &[lobby::Row], score: i64) -> i64 {
    rows.iter().filter(|r| !r.mine && r.score > score).count() as i64 + 1
}

/// What the box says: its title and lines.
struct Lines {
    title: String,
    lines: Vec<String>,
    /// Lines shown when opened out.
    open: Vec<(String, bool)>,
}

impl App {
    /// The game being played, as the reminder sees it (none: not a multiplayer game).
    fn mp_hud_lines(&self) -> Option<Lines> {
        let scenario = self.scen.goals.scenario.as_deref();
        let f = crate::scenario_ui::facts(self);
        let month = self.econ.month_index();
        if let Some(c) = rival::challenge_of(scenario) {
            return Some(self.weekly_lines(&c, &f, month));
        }
        if let Some((code, _)) = league::league_of(scenario) {
            return Some(neighbours_lines(&code, &f, month));
        }
        let live = self.mp.live.as_ref()?;
        let mut open: Vec<(String, bool)> = live.players.iter().map(|p| (p.nick.clone(), false)).collect();
        open.truncate(league::SEATS);
        Some(Lines {
            title: trf("STR_MP_HUD_CLUB", &[("NAME", &live.name)]),
            lines: vec![if live.players.len() == 1 {
                tr("STR_MP_HUD_CLUB_ALONE").to_string()
            } else {
                trf("STR_MP_HUD_CLUB_ON", &[("N", &live.players.len())])
            }],
            open,
        })
    }

    fn weekly_lines(&self, c: &Challenge, f: &sg_core::scenario::Facts, month: i32) -> Lines {
        let mut lines = vec![trf("STR_MP_HUD_GOAL", &[("GOAL", &goal_text(c.measure)), ("YEAR", &c.end_year)])];
        let twist = match c.kind {
            super::challenge::Kind::Standard => String::new(),
            super::challenge::Kind::Budget => tr("STR_MP_HUD_BUDGET").into(),
            super::challenge::Kind::Nine => trf("STR_MP_HUD_NINE", &[("N", &super::challenge::NINE_HOLES)]),
        };
        if !twist.is_empty() {
            lines.push(twist);
        }
        let score = c.measure.score(f);
        let mut now = trf("STR_MP_HUD_NOW", &[("SCORE", &lobby::score_text(c.measure, score))]);
        let mut open = Vec::new();
        KNOWN.with(|k| {
            if let Some(lobby::Board::Rows { rows, .. }) = &k.borrow().board {
                let place = would_place(rows, score);
                now = trf("STR_MP_HUD_NOW_PLACE", &[("SCORE", &lobby::score_text(c.measure, score)), ("PLACE", &lobby::place_text(place))]);
                open = rows
                    .iter()
                    .take(OPEN_ROWS)
                    .map(|r| (format!("{}. {}  {}", r.place, r.nick, lobby::score_text(c.measure, r.score)), r.mine))
                    .collect();
                if open.is_empty() {
                    open.push((tr("STR_MP_BOARD_EMPTY").to_string(), false));
                }
            }
        });
        lines.push(format!("{}  ·  {}", left_text(months_left(month, c.end_year)), now));
        if self.unfair_game() {
            lines.push(tr("STR_MP_HUD_CHEATED").into());
        } else if self.challenge_played_on() || months_left(month, c.end_year) == 0 {
            lines.push(tr("STR_MP_HUD_PLAYED_ON").into());
        }
        Lines { title: trf("STR_MP_WEEKLY_NAME", &[("WEEK", &c.id)]), lines, open }
    }

    /// The box's rectangle as drawn now (none: nothing to show).
    fn mp_hud_rect(&self, n: usize) -> Rect {
        Rect::new(X, Y, W, 22.0 + LINE_H * n as f32)
    }

    fn mp_hud_shown(&self) -> bool {
        self.screen == Screen::Play && self.club.game & sg_core::golfer::game::TOURNAMENT == 0 && self.great_shot.is_none()
    }

    /// Each frame while playing: the board or the table asked for now and then, and their answers read.
    pub fn mp_hud_poll(&mut self) {
        let scenario = self.scen.goals.scenario.clone().unwrap_or_default();
        let weekly = rival::challenge_of(Some(&scenario));
        let league = league::league_of(Some(&scenario));
        if weekly.is_none() && league.is_none() {
            return;
        }
        let now = miniquad::date::now();
        KNOWN.with(|k| {
            let mut k = k.borrow_mut();
            if k.game != scenario {
                *k = Known { game: scenario.clone(), open: k.open, ..Default::default() };
            }
            if let Some(reply) = k.request.as_mut().and_then(|r| r.poll()) {
                k.request = None;
                match reply {
                    Ok(body) if weekly.is_some() => k.board = Some(lobby::read_board(&body)),
                    Ok(body) => k.table = league::read_table(&body).ok().or(k.table.take()),
                    Err(e) => println!("hud: {e}"),
                }
            }
            if k.request.is_some() || (k.asked_at > 0.0 && now - k.asked_at < REFRESH_S) {
                return;
            }
            let s = settings::get();
            let url = if let Some(c) = &weekly {
                let mut url = format!("{}/board?c={}", rival::board_url(), c.id);
                let g = settings::group(&s);
                if !g.is_empty() {
                    url.push_str(&format!("&g={g}"));
                }
                if settings::is_player(&s.player) {
                    url.push_str(&format!("&p={}", s.player));
                }
                url
            } else if let Some((code, _)) = &league {
                format!("{}/leagues/{code}?p={}", super::club::server(), s.player)
            } else {
                return;
            };
            k.asked_at = now;
            k.request = Some(crate::net::get(&url, 64 * 1024));
        });
    }

    /// Draws the box (nothing outside a multiplayer game).
    pub fn draw_mp_hud(&self, g: &mut Gfx, s: &Ui) {
        if !self.mp_hud_shown() {
            return;
        }
        let Some(l) = self.mp_hud_lines() else { return };
        let open = KNOWN.with(|k| k.borrow().open) && !l.open.is_empty();
        let n = l.lines.len() + if open { l.open.len() } else { 0 };
        let r = self.mp_hud_rect(n);
        self.art.trans_frame(g, s, r.x, r.y, r.w, r.h);
        let (pale, white, yellow, shadow) = (c15(0x7ff0), c15(0x7fff), c15(0x7fe0), crate::ui::rgb(0.0, 0.0, 0.0));
        let put = |g: &mut Gfx, y: f32, t: &str, c: [f32; 4]| {
            let t = crate::content::fit(t, W - 16.0);
            s.put(g, F_ARIAL10, X + 8.0, y + 1.0, &t, shadow);
            s.put(g, F_ARIAL10, X + 7.0, y, &t, c);
        };
        put(g, Y + 5.0, &l.title, pale);
        let mut y = Y + 5.0 + LINE_H + 2.0;
        for t in &l.lines {
            put(g, y, t, white);
            y += LINE_H;
        }
        if open {
            for (t, mine) in &l.open {
                put(g, y, t, if *mine { yellow } else { white });
                y += LINE_H;
            }
        }
    }

    /// A click on the box opens it out or folds it (true: the click was the box's).
    pub fn mp_hud_click(&mut self, vx: f32, vy: f32) -> bool {
        if !self.mp_hud_shown() {
            return false;
        }
        let Some(l) = self.mp_hud_lines() else { return false };
        let (x, y) = crate::enhanced::unshift(vx, vy);
        let open = KNOWN.with(|k| k.borrow().open) && !l.open.is_empty();
        let n = l.lines.len() + if open { l.open.len() } else { 0 };
        if !self.mp_hud_rect(n).has(x, y) {
            return false;
        }
        KNOWN.with(|k| {
            let mut k = k.borrow_mut();
            k.open = !k.open;
        });
        true
    }
}

/// A neighbours league's lines: from the table once it is in.
fn neighbours_lines(code: &str, f: &sg_core::scenario::Facts, month: i32) -> Lines {
    KNOWN.with(|k| {
        let k = k.borrow();
        let left = left_text(months_left(month, league::END_YEAR));
        let Some(t) = &k.table else {
            return Lines { title: trf("STR_MP_HUD_LEAGUE", &[("NAME", &code)]), lines: vec![left], open: Vec::new() };
        };
        let m: Measure = t.league.measure;
        let ranked = t.ranked();
        // the player's own numbers now, in place of the table's last report
        let mine = m.score(f);
        let place = ranked.iter().filter(|r| !r.mine && r.facts.as_ref().is_some_and(|x| m.score(x) > mine)).count() + 1;
        let open = ranked
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let score = if r.mine { Some(mine) } else { r.facts.as_ref().map(|x| m.score(x)) };
                let score = score.map(|v| lobby::score_text(m, v)).unwrap_or_else(|| "-".into());
                (format!("{}. {}  {}", i + 1, r.nick, score), r.mine)
            })
            .collect();
        Lines {
            title: trf("STR_MP_HUD_LEAGUE", &[("NAME", &t.league.name)]),
            lines: vec![
                trf("STR_MP_HUD_GOAL", &[("GOAL", &goal_text(m)), ("YEAR", &league::END_YEAR)]),
                format!(
                    "{}  ·  {}",
                    left,
                    trf("STR_MP_HUD_LEAGUE_PLACE", &[("PLACE", &lobby::place_text(place as i64)), ("N", &t.rows.len())])
                ),
            ],
            open,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_left_count_to_the_end_of_the_year() {
        // March 2001 is month 0; a 2001 challenge has the 8 months of 2001
        assert_eq!(months_left(0, 2001), 8);
        assert_eq!(months_left(7, 2001), 1);
        assert_eq!(months_left(8, 2001), 0);
        assert_eq!(months_left(30, 2001), 0);
        assert_eq!(months_left(0, 2002), 16);
    }

    #[test]
    fn a_score_places_among_the_others() {
        let row = |score, mine| lobby::Row { place: 0, nick: "x".into(), score, mine };
        let rows = [row(90, false), row(50, true), row(40, false)];
        assert_eq!(would_place(&rows, 95), 1);
        assert_eq!(would_place(&rows, 60), 2);
        assert_eq!(would_place(&rows, 10), 3);
    }
}
