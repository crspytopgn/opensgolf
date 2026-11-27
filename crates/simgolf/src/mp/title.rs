//! The title menu's "This week's challenge" button (the port's own): gold, in the middle of the top row of the port's own
//! buttons (between Mods... and Check for updates, where both title pictures leave the background bare), it says the
//! week's goal and the days left, and once Multiplayer is on, how many are on the board or the player's own place there (one look
//! at the public board a session). A click opens Start New Game's Multiplayer page on the weekly challenge (its note
//! first while Multiplayer is off: picking it is still the player's yes). A player who sent results and then turned
//! Multiplayer off (Preferences > More) doesn't see it.
//!
//! The hooks outside this folder, each behind the `multiplayer` feature: render.rs (drawing it), main.rs (pointing at it
//! and clicking it).
use super::challenge::{self, Challenge};
use super::{lobby, rival, settings};
use crate::app::App;
use crate::render::Rect;
use sg_core::lang::{tr, trf};
use std::cell::RefCell;

/// The button's hover value (after content::TITLE_HOVER).
pub const HOVER: i32 = 14;

/// What the board said this session: the week, the players on it and the player's own place.
#[derive(Clone, Debug, Default, PartialEq)]
struct Seen {
    week: String,
    players: usize,
    place: Option<i64>,
}

thread_local! {
    static ASKED: RefCell<bool> = const { RefCell::new(false) };
    static REQUEST: RefCell<Option<(String, crate::net::Request)>> = const { RefCell::new(None) };
    static SEEN: RefCell<Option<Seen>> = const { RefCell::new(None) };
}

fn now_s() -> i64 {
    miniquad::date::now() as i64
}

/// Whether the title shows the button: always, but for a player who used multiplayer and then turned it off.
pub fn shown() -> bool {
    let s = settings::get();
    s.on || !settings::is_player(&s.player)
}

/// The button's width at most (its line fits it, so the button never jumps).
const WIDTH: f32 = 330.0;
/// The room it keeps from its neighbours in the row.
const GAP: f32 = 10.0;

/// The button in the room between `left` (Mods...'s right edge) and `right` (Check for updates' left edge): centred
/// there, as tall as the row's other buttons.
pub fn button_rect_between(left: f32, right: f32) -> Rect {
    let w = (right - left - 2.0 * GAP).min(WIDTH);
    Rect::new((left + right - w) / 2.0, 6.0, w, 24.0)
}

/// Days left of a week at `now_s` (7 on its Monday, 1 on its Sunday).
pub fn days_left(week: &str, now_s: i64) -> i64 {
    challenge::monday_of(&challenge::next_week(week)) - now_s.div_euclid(86_400)
}

/// The button's second line.
pub fn note(c: &Challenge, seen: Option<&(usize, Option<i64>)>, days: i64) -> String {
    super::texts();
    let goal = super::hud::goal_text(c.measure);
    let what = match seen {
        // below the board's top 50: the place alone
        Some((n, Some(place))) if *place > *n as i64 => trf("STR_MP_TITLE_PLACE_ONLY", &[("PLACE", &lobby::place_text(*place))]),
        Some((n, Some(place))) => trf("STR_MP_TITLE_PLACE", &[("PLACE", &lobby::place_text(*place)), ("N", &players(*n))]),
        Some((n, None)) if *n > 0 => trf("STR_MP_TITLE_PLAYING", &[("GOAL", &goal), ("N", &players(*n))]),
        _ => trf("STR_MP_TITLE_GOAL", &[("GOAL", &goal)]),
    };
    let left = if days <= 1 { tr("STR_MP_TITLE_LAST_DAY").to_string() } else { trf("STR_MP_TITLE_DAYS", &[("N", &days)]) };
    let line = trf("STR_MP_TITLE_NOTE", &[("WHAT", &what), ("LEFT", &left)]);
    // the goal starts the line in lower case ("most cash")
    let mut chars = line.chars();
    chars.next().map(|f| f.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The board lists its top 50 (multiplayer/worker/src/index.js BOARD_SIZE).
fn players(n: usize) -> String {
    if n >= 50 {
        "50+".into()
    } else {
        n.to_string()
    }
}

/// Once a session, with Multiplayer on: asks this week's public board how many are on it and where the player is.
fn ask(week: &str) {
    if ASKED.with(|a| a.replace(true)) {
        return;
    }
    let s = settings::get();
    if !s.on {
        return;
    }
    let mut url = format!("{}/board?c={week}", rival::board_url());
    if settings::is_player(&s.player) {
        url.push_str(&format!("&p={}", s.player));
    }
    REQUEST.with(|r| *r.borrow_mut() = Some((week.to_string(), crate::net::get(&url, 64 * 1024))));
}

/// Reads the board's answer into what the button shows.
fn read(week: &str, body: &[u8]) -> Option<Seen> {
    let lobby::Board::Rows { rows, me } = lobby::read_board(body) else { return None };
    let place = rows.iter().find(|r| r.mine).map(|r| r.place).or(me.map(|m| m.0));
    Some(Seen { week: week.to_string(), players: rows.len(), place })
}

/// Each frame on the title: the board's answer, once it is in.
fn poll() {
    let Some((week, reply)) = REQUEST.with(|r| {
        let mut r = r.borrow_mut();
        let reply = r.as_mut()?.1.poll()?;
        let week = r.take()?.0;
        Some((week, reply))
    }) else {
        return;
    };
    if let Some(seen) = reply.ok().and_then(|b| read(&week, &b)) {
        SEEN.with(|s| *s.borrow_mut() = Some(seen));
    }
}

/// The button's line, the longest that `fits`: the full title, the short one, the short one without the days left, and
/// last the short one cut with "...".
pub fn fit_line(note: &str, fits: impl Fn(&str) -> bool) -> String {
    super::texts();
    fn line(title: &'static str, note: &str) -> String {
        trf("STR_MP_TITLE_LINE", &[("TITLE", &tr(title)), ("NOTE", &note)])
    }
    let without_days = note.rsplit_once(", ").map_or(note, |(a, _)| a);
    let tries = [line("STR_MP_TITLE_BUTTON", note), line("STR_MP_TITLE_SHORT", note), line("STR_MP_TITLE_SHORT", without_days)];
    if let Some(t) = tries.iter().find(|t| fits(t)) {
        return t.clone();
    }
    let mut t = tries[2].clone();
    while !t.is_empty() && !fits(&format!("{t}...")) {
        t.pop();
    }
    format!("{t}...")
}

impl App {
    /// The button's place in the title's top row.
    pub fn weekly_button(&self) -> Rect {
        let l = self.mods_button();
        let right = self.update_button().map_or(792.0, |u| u.x);
        button_rect_between(l.x + l.w, right)
    }

    /// Draws the button on the title menu (render.rs), gold like the board's first place.
    pub fn draw_weekly_button(&self, g: &mut crate::gfx::Gfx, s: &crate::ui::Screen) {
        use crate::ui::{rgba, F_MANUAL15};
        if !shown() {
            return;
        }
        let now = now_s();
        let c = challenge::this_week(now);
        ask(&c.id);
        poll();
        let seen = SEEN.with(|x| x.borrow().as_ref().filter(|x| x.week == c.id).map(|x| (x.players, x.place)));
        let r = self.weekly_button();
        let lit = self.hover == HOVER;
        let fill = if lit { [1.0, 0.86, 0.36, 1.0] } else { [0.96, 0.74, 0.12, 1.0] };
        if crate::glass_ui::active() {
            crate::glass_ui::capsule(g, s, r.x - 2.0, r.y - 1.0, r.w + 4.0, r.h + 3.0, fill);
        } else {
            s.fill(g, r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, rgba(1.0, 1.0, 1.0, 0.9));
            s.fill(g, r.x, r.y, r.w, r.h, fill);
        }
        let ink = [0.24, 0.13, 0.0, 1.0];
        let note = note(&c, seen.as_ref(), days_left(&c.id, now));
        let line = fit_line(&note, |t| F_MANUAL15.width(t) <= r.w - 14.0);
        s.put_centered(g, F_MANUAL15, r.x + r.w / 2.0, r.y + 4.0, &line, ink);
    }

    /// The button clicked: Start New Game's Multiplayer page, on the weekly challenge's own page once Multiplayer is on.
    pub fn open_weekly_from_title(&mut self) {
        self.ui_sound(0x18);
        self.open_scenarios();
        self.set_scenarios_mode(crate::scenario_ui::Mode::Multi);
        if settings::get().on {
            let id = format!("{}{}", rival::ID_PREFIX, challenge::week_id(now_s()));
            self.open_rival_lobby(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_days_left_count_down_to_sunday() {
        // 2026-11-30 (a Monday, 2026-W49) at noon, and 2026-12-06 (its Sunday)
        let monday = challenge::monday_of("2026-W49") * 86_400 + 12 * 3600;
        assert_eq!(days_left("2026-W49", monday), 7);
        assert_eq!(days_left("2026-W49", monday + 6 * 86_400), 1);
    }

    #[test]
    fn the_note_says_the_place_the_players_or_the_goal() {
        let c = challenge::challenge("2026-W49");
        assert_eq!(note(&c, Some(&(12, Some(3))), 4), "You're 3rd of 12, 4 days left");
        assert_eq!(note(&c, Some(&(12, None)), 4), "Most cash, 12 playing, 4 days left");
        assert_eq!(note(&c, None, 1), "Most cash, last day");
        assert_eq!(note(&c, Some(&(60, Some(70))), 2), "You're 70th, 2 days left");
    }

    #[test]
    fn the_line_shortens_to_fit() {
        let n = "You're 2nd of 3, 4 days left";
        assert_eq!(fit_line(n, |_| true), "This week's challenge: You're 2nd of 3, 4 days left");
        assert_eq!(fit_line(n, |t| t.len() <= 40), "Weekly: You're 2nd of 3, 4 days left");
        assert_eq!(fit_line(n, |t| t.len() <= 30), "Weekly: You're 2nd of 3");
        assert_eq!(fit_line(n, |t| t.len() <= 12), "Weekly: Y...");
    }

    #[test]
    fn the_place_is_read_from_the_players_row_or_the_one_told() {
        let b =
            br#"{"ok":true,"rows":[{"place":1,"nick":"a","score":5,"mine":false},{"place":2,"nick":"b","score":4,"mine":true}],"me":null}"#;
        assert_eq!(read("2026-W49", b), Some(Seen { week: "2026-W49".into(), players: 2, place: Some(2) }));
        let b = br#"{"ok":true,"rows":[{"place":1,"nick":"a","score":5,"mine":false}],"me":{"place":60,"score":1}}"#;
        assert_eq!(read("2026-W49", b).unwrap().place, Some(60));
        assert_eq!(read("2026-W49", b"<html>"), None);
    }
}
