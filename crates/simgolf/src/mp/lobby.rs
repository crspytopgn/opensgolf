//! The weekly challenge's page: what Start New Game's "Weekly challenge" row opens. It says what the week is, takes the
//! nickname and friends' group code, shows the week's board, and turns Multiplayer on (nothing is ever sent before that)
//! and starts the game.
//!
//! It is drawn in place of Start New Game while open, so it needs no screen of its own: scenario_ui.rs's draw, click and
//! key functions ask it first (one cfg-gated line each), and main.rs passes it typed characters.

use super::challenge::{Challenge, Measure};
use super::{rival, settings};
use crate::app::App;
use crate::extras::draw_button;
use crate::gfx::Gfx;
use crate::info_ui::c15;
use crate::render::Rect;
use crate::ui::{Screen as Ui, F_ARIAL10, F_INFO14, F_MANUAL20};
use sg_core::lang::{tr, trf};
use std::cell::RefCell;

const BOX: Rect = Rect { x: 40.0, y: 36.0, w: 720.0, h: 528.0 };
const NICK: Rect = Rect { x: BOX.x + 200.0, y: BOX.y + 150.0, w: 220.0, h: 22.0 };
const GROUP: Rect = Rect { x: BOX.x + 200.0, y: BOX.y + 180.0, w: 220.0, h: 22.0 };
const BACK: Rect = Rect { x: BOX.x + BOX.w - 330.0, y: BOX.y + BOX.h - 46.0, w: 100.0, h: 22.0 };
const PLAY: Rect = Rect { x: BOX.x + BOX.w - 220.0, y: BOX.y + BOX.h - 46.0, w: 190.0, h: 22.0 };
/// "Compare with friends", in place of the group's field until it is clicked (or a group is kept).
const FRIENDS: Rect = Rect { x: BOX.x + 24.0, y: BOX.y + 180.0, w: 240.0, h: 22.0 };
/// The board's week: earlier and later (past weeks keep their final boards).
const EARLIER: Rect = Rect { x: BOX.x + BOX.w - 250.0, y: BOX.y + 232.0, w: 110.0, h: 20.0 };
const LATER: Rect = Rect { x: BOX.x + BOX.w - 134.0, y: BOX.y + 232.0, w: 110.0, h: 20.0 };
/// The first week with a board (multiplayer came out in it): no earlier one to show.
pub const FIRST_WEEK: &str = "2026-W48";
const BOARD_TOP: f32 = BOX.y + 262.0;
/// Board rows shown.
const ROWS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Nick,
    Group,
}

/// A row of the board, as the Worker sends it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub place: i64,
    pub nick: String,
    pub score: i64,
    pub mine: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Board {
    Loading,
    Rows { rows: Vec<Row>, me: Option<(i64, i64)> },
    Failed(String),
}

#[derive(Clone)]
struct Lobby {
    /// The row's scenario id (`*rival/<week>`).
    id: String,
    challenge: Challenge,
    nick: String,
    group: String,
    editing: Option<Field>,
    board: Board,
    /// The group code of the board shown (empty: the public one).
    board_group: String,
    /// The group's field is shown (else "Compare with friends" in its place).
    friends: bool,
    /// The week of the board shown: this week's, or a past one's final board.
    week: String,
}

thread_local! {
    static LOBBY: RefCell<Option<Lobby>> = const { RefCell::new(None) };
    static FETCH: RefCell<Option<crate::net::Request>> = const { RefCell::new(None) };
    /// Last week's board asked for the player's place when the game starts: (the week, the request).
    static LAST_WEEK: RefCell<Option<(String, crate::net::Request)>> = const { RefCell::new(None) };
    static LAST_WEEK_ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn lobby() -> Option<Lobby> {
    LOBBY.with(|l| l.borrow().clone())
}

fn change(f: impl FnOnce(&mut Lobby)) {
    LOBBY.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            f(l)
        }
    })
}

/// Whether a field is being typed in (a phone shows its keyboard: touch.rs).
pub fn editing() -> bool {
    LOBBY.with(|l| l.borrow().as_ref().is_some_and(|l| l.editing.is_some()))
}

/// Whether the page is open.
pub fn is_open() -> bool {
    LOBBY.with(|l| l.borrow().is_some())
}

/// The board's answer read: its rows and the player's own place.
pub fn read_board(body: &[u8]) -> Board {
    let v: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Board::Failed("the board's answer couldn't be read".into()),
    };
    if v["ok"].as_bool() != Some(true) {
        return Board::Failed(v["error"].as_str().unwrap_or("the board said no").to_string());
    }
    let rows = v["rows"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| Row {
                    place: r["place"].as_i64().unwrap_or(0),
                    nick: r["nick"].as_str().unwrap_or("?").chars().take(16).collect(),
                    score: r["score"].as_i64().unwrap_or(0),
                    mine: r["mine"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let me = v["me"].as_object().map(|m| (m["place"].as_i64().unwrap_or(0), m["score"].as_i64().unwrap_or(0)));
    Board::Rows { rows, me }
}

/// A score as the measure shows it.
pub fn score_text(m: Measure, v: i64) -> String {
    match m {
        Measure::Cash => format!("${}", sg_core::economy::money_digits(v)),
        // the club keeps skill in hundredths
        Measure::Skill => format!("{:.2}", v as f64 / 100.0),
        Measure::Fun | Measure::Members => v.to_string(),
    }
}

pub fn measure_name(m: Measure) -> &'static str {
    super::texts();
    match m {
        Measure::Cash => tr("STR_MP_MEASURE_CASH"),
        Measure::Fun => tr("STR_MP_MEASURE_FUN"),
        Measure::Members => tr("STR_MP_MEASURE_MEMBERS"),
        Measure::Skill => tr("STR_MP_MEASURE_SKILL"),
    }
}

/// Whether a typed character may go into a field (the Worker's rules: names.js, index.js isGroup).
pub fn allowed(f: Field, text: &str, c: char) -> bool {
    match f {
        Field::Nick => text.chars().count() < 16 && (c.is_ascii_alphanumeric() || " _.-".contains(c)),
        Field::Group => text.len() < 6 && c.is_ascii_alphanumeric(),
    }
}

fn start_fetch(l: &Lobby) {
    let s = settings::get();
    let mut url = format!("{}/board?c={}", rival::board_url(), l.week);
    let g = settings::group(&settings::Settings { group: l.group.clone(), ..Default::default() });
    if !g.is_empty() {
        url.push_str(&format!("&g={g}"));
    }
    if settings::is_player(&s.player) {
        url.push_str(&format!("&p={}", s.player));
    }
    FETCH.with(|f| *f.borrow_mut() = Some(crate::net::get(&url, 64 * 1024)));
    change(|l| {
        l.board = Board::Loading;
        l.board_group = g;
    });
}

/// The note for the player's final place in a past week ("1st", "2nd", ...).
pub fn place_text(place: i64) -> String {
    let suffix = match (place % 10, place % 100) {
        (1, p) if p != 11 => "st",
        (2, p) if p != 12 => "nd",
        (3, p) if p != 13 => "rd",
        _ => "th",
    };
    format!("{place}{suffix}")
}

/// Once a session, with multiplayer on: asks last week's public board for the player's place (nothing when they
/// weren't on it, or were told already).
fn ask_last_week(now_s: i64) {
    if LAST_WEEK_ASKED.with(|a| a.replace(true)) {
        return;
    }
    let s = settings::get();
    let week = super::challenge::previous_week(&super::challenge::week_id(now_s));
    if !s.on || !settings::is_player(&s.player) || s.told_week == week || week.as_str() < FIRST_WEEK {
        return;
    }
    let url = format!("{}/board?c={week}&p={}", rival::board_url(), s.player);
    LAST_WEEK.with(|f| *f.borrow_mut() = Some((week, crate::net::get(&url, 64 * 1024))));
}

/// Last week's answer: the note to show ("You finished 3rd in last week's challenge."), once.
fn poll_last_week() -> Option<String> {
    let (week, reply) = LAST_WEEK.with(|f| {
        let mut f = f.borrow_mut();
        let (w, r) = f.as_mut()?;
        let reply = r.poll()?;
        let w = w.clone();
        *f = None;
        Some((w, reply))
    })?;
    // the player's row among the board's top ones, else the place the board told them
    let Board::Rows { rows, me } = read_board(&reply.ok()?) else { return None };
    let place = rows.iter().find(|r| r.mine).map(|r| r.place).or(me.map(|m| m.0))?;
    let mut s = settings::get();
    s.told_week = week;
    settings::save(s);
    super::texts();
    Some(if place == 1 { tr("STR_MP_LAST_WEEK_WON").to_string() } else { trf("STR_MP_LAST_WEEK_PLACE", &[("PLACE", &place_text(place))]) })
}

/// Each frame: the board's answer, once it is in; and the note of last week's place (returned once).
pub fn poll(now_s: i64) -> Option<String> {
    ask_last_week(now_s);
    let note = poll_last_week();
    poll_board();
    note
}

fn poll_board() {
    let Some(reply) = FETCH.with(|f| {
        let mut f = f.borrow_mut();
        let r = f.as_mut()?.poll()?;
        *f = None;
        Some(r)
    }) else {
        return;
    };
    let b = match reply {
        Ok(body) => read_board(&body),
        Err(e) => {
            println!("board: couldn't be fetched: {e}");
            Board::Failed(tr("STR_MP_CHECK_CONNECTION").into())
        }
    };
    change(|l| l.board = b);
}

impl App {
    /// Opens the page for a Start New Game row (`id`: `*rival/<week>`).
    pub fn open_rival_lobby(&mut self, id: &str) -> bool {
        let Some(c) = rival::challenge_of(Some(id)) else { return false };
        let s = settings::get();
        let l = Lobby {
            id: id.to_string(),
            nick: s.nick.clone(),
            group: s.group.clone(),
            editing: None,
            board: Board::Loading,
            board_group: String::new(),
            friends: !s.group.is_empty(),
            week: c.id.clone(),
            challenge: c,
        };
        LOBBY.with(|x| *x.borrow_mut() = Some(l.clone()));
        start_fetch(&l);
        self.hover = -1;
        true
    }

    fn close_rival_lobby(&mut self) {
        LOBBY.with(|x| *x.borrow_mut() = None);
        FETCH.with(|f| *f.borrow_mut() = None);
        self.hover = -1;
    }

    /// Shows a week's board (this week's, or a past week's final one).
    fn show_board_week(&mut self, week: String) {
        change(|l| l.week = week);
        if let Some(l) = lobby() {
            start_fetch(&l);
        }
    }

    /// Keeps the typed nickname and group code (Multiplayer stays as it was).
    fn keep_lobby_fields(&self, l: &Lobby) {
        let mut s = settings::get();
        if (s.nick.as_str(), s.group.as_str()) != (l.nick.trim(), l.group.as_str()) {
            s.nick = l.nick.trim().to_string();
            s.group = l.group.clone();
            settings::save(s);
        }
    }

    fn finish_editing(&mut self) {
        let Some(l) = lobby() else { return };
        if l.editing == Some(Field::Group) {
            start_fetch(&l);
        }
        change(|l| l.editing = None);
        self.keep_lobby_fields(&l);
    }

    fn lobby_play(&mut self, g: &mut Gfx) {
        let Some(l) = lobby() else { return };
        self.keep_lobby_fields(&l);
        let mut s = settings::with_player();
        if !s.on {
            s.on = true;
            settings::save(s);
        }
        self.close_rival_lobby();
        self.start_rival(g, &l.id);
    }

    /// A click while the page is open (false: it isn't).
    pub fn rival_lobby_click(&mut self, g: &mut Gfx, x: f32, y: f32) -> bool {
        if self.clubs_click(x, y) || self.leagues_click(g, x, y) {
            return true;
        }
        let Some(l) = lobby() else { return false };
        let group_hit = l.friends && GROUP.has(x, y);
        if l.editing.is_some() && !NICK.has(x, y) && !group_hit {
            self.finish_editing();
        }
        if NICK.has(x, y) {
            change(|l| l.editing = Some(Field::Nick));
        } else if group_hit || (!l.friends && FRIENDS.has(x, y)) {
            change(|l| {
                l.friends = true;
                l.editing = Some(Field::Group);
            });
        } else if PLAY.has(x, y) {
            self.lobby_play(g);
        } else if BACK.has(x, y) || !BOX.has(x, y) {
            self.close_rival_lobby();
        } else if EARLIER.has(x, y) && l.week.as_str() > FIRST_WEEK {
            self.show_board_week(super::challenge::previous_week(&l.week));
        } else if LATER.has(x, y) && l.week != l.challenge.id {
            self.show_board_week(super::challenge::next_week(&l.week));
        }
        true
    }

    /// A key while the page is open (false: it isn't).
    pub fn rival_lobby_key(&mut self, g: &mut Gfx, k: miniquad::KeyCode) -> bool {
        use miniquad::KeyCode;
        if self.clubs_key(k) || self.leagues_key(g, k) {
            return true;
        }
        let Some(l) = lobby() else { return false };
        match (l.editing, k) {
            (Some(_), KeyCode::Backspace) => change(|l| {
                let t = if l.editing == Some(Field::Nick) { &mut l.nick } else { &mut l.group };
                t.pop();
            }),
            (Some(Field::Nick), KeyCode::Tab) if l.friends => {
                self.finish_editing();
                change(|l| l.editing = Some(Field::Group));
            }
            (Some(_), KeyCode::Escape | KeyCode::Enter | KeyCode::KpEnter | KeyCode::Tab) => self.finish_editing(),
            (Some(_), _) => {}
            (None, KeyCode::Escape) => self.close_rival_lobby(),
            (None, KeyCode::Enter | KeyCode::KpEnter) => self.lobby_play(g),
            (None, KeyCode::Tab) => change(|l| l.editing = Some(Field::Nick)),
            _ => {}
        }
        true
    }

    /// A typed character (nothing unless a field is being edited).
    pub fn rival_lobby_char(&mut self, c: char) {
        if self.clubs_char(c) || self.leagues_char(c) {
            return;
        }
        change(|l| match l.editing {
            Some(Field::Nick) if allowed(Field::Nick, &l.nick, c) => l.nick.push(c),
            Some(Field::Group) if allowed(Field::Group, &l.group, c) => l.group.push(c.to_ascii_uppercase()),
            _ => {}
        });
    }

    /// Draws the page while it is open (false: it isn't).
    pub fn rival_lobby_draw(&mut self, g: &mut Gfx) -> bool {
        // the shared clubs' page (clubs_ui.rs) is drawn the same way
        if self.clubs_draw(g) || self.leagues_draw(g) {
            return true;
        }
        let Some(l) = lobby() else { return false };
        let s = Ui::new(self.draw_w, self.draw_h);
        self.view = s.view;
        crate::info_ui::dim(g, &s);
        self.draw_info_frame(g, &s, BOX.x, BOX.y, BOX.w, BOX.h);
        let (white, yellow, navy, grey) = (c15(0x7fff), c15(0x7ff0), c15(0x0018), c15(0x6318));
        let (px, py) = self.info.pointer;
        let c = &l.challenge;
        let f = crate::textsize::big(F_INFO14);
        let x = BOX.x + 24.0;
        s.put_shadowed(g, F_MANUAL20, 400.0, BOX.y + 14.0, &trf("STR_MP_WEEKLY_NAME", &[("WEEK", &c.id)]), yellow, true);
        let sc = rival::scenario(c);
        let place = sg_core::data::properties().get(sc.property_index()).map(|p| p.name().to_string()).unwrap_or_default();
        let diff = sg_core::lang::tr(crate::render::DIFFICULTY_KEYS[c.difficulty.level() as usize]);
        s.put(
            g,
            f,
            x,
            BOX.y + 50.0,
            format!(
                "{} {}",
                trf(
                    "STR_MP_WEEKLY_PLACE",
                    &[
                        ("PLACE", &place),
                        ("DIFFICULTY", &diff),
                        ("MEASURE", &measure_name(c.measure).to_lowercase()),
                        ("YEAR", &c.end_year)
                    ],
                ),
                rival::kind_text(c.kind)
            )
            .trim_end(),
            white,
        );
        s.put(g, f, x, BOX.y + 72.0, tr("STR_MP_WEEKLY_SAME"), white);
        s.put_fit(g, F_ARIAL10, x, BOX.y + 104.0, BOX.w - 48.0, tr("STR_MP_ON_NOTE"), navy);
        s.put_fit(g, F_ARIAL10, x, BOX.y + 118.0, BOX.w - 48.0, tr("STR_MP_NOTHING_ELSE"), navy);
        if !l.friends {
            let lit = FRIENDS.has(px, py);
            s.put(g, f, x, FRIENDS.y + 2.0, tr("STR_MP_COMPARE_FRIENDS"), if lit { white } else { yellow });
            s.fill(
                g,
                x,
                FRIENDS.y + 19.0,
                crate::ui::text_width(tr("STR_MP_COMPARE_FRIENDS"), 14.0),
                1.0,
                if lit { white } else { yellow },
            );
        }
        let fields = [
            (tr("STR_MP_NICKNAME"), NICK, &l.nick, Field::Nick, tr("STR_MP_NICKNAME_HINT")),
            (tr("STR_MP_GROUP"), GROUP, &l.group, Field::Group, tr("STR_MP_GROUP_HINT")),
        ];
        for (label, r, text, field, hint) in fields.into_iter().take(if l.friends { 2 } else { 1 }) {
            let editing = l.editing == Some(field);
            s.put(g, f, x, r.y + 2.0, label, white);
            s.fill(g, r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, c15(if editing { 0x7ff0 } else { 0x2d6b }));
            s.fill(g, r.x, r.y, r.w, r.h, c15(0x7fff));
            let shown = if text.is_empty() && !editing {
                if field == Field::Nick {
                    let s = settings::get();
                    if settings::is_player(&s.player) {
                        settings::nick(&s)
                    } else {
                        tr("STR_MP_NICKNAME_MADE").into()
                    }
                } else {
                    tr("STR_MP_GROUP_NONE").into()
                }
            } else {
                text.clone()
            };
            let caret = if editing && (miniquad::date::now() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
            s.put(g, f, r.x + 6.0, r.y + 2.0, &format!("{shown}{caret}"), if text.is_empty() && !editing { grey } else { navy });
            s.put_fit(g, F_ARIAL10, r.x + r.w + 12.0, r.y + 6.0, BOX.x + BOX.w - 24.0 - (r.x + r.w + 12.0), hint, navy);
        }
        let grp = &l.board_group;
        let past = l.week != c.id;
        let title = match (past, grp.is_empty()) {
            (false, true) => tr("STR_MP_BOARD_TITLE").to_string(),
            (false, false) => trf("STR_MP_BOARD_TITLE_GROUP", &[("GROUP", grp)]),
            (true, true) => trf("STR_MP_BOARD_TITLE_PAST", &[("WEEK", &l.week)]),
            (true, false) => trf("STR_MP_BOARD_TITLE_PAST_GROUP", &[("WEEK", &l.week), ("GROUP", grp)]),
        };
        s.put_shadowed(g, crate::ui::F_MANUAL15, x, BOARD_TOP - 26.0, &title, yellow, false);
        // the week's own measure (a past week's may differ from this week's)
        let shown = if past { super::challenge::challenge(&l.week) } else { c.clone() };
        for (r, label, ok) in [(EARLIER, tr("STR_MP_BOARD_EARLIER"), l.week.as_str() > FIRST_WEEK), (LATER, tr("STR_MP_BOARD_LATER"), past)]
        {
            // (none before there is a past week to show)
            if l.week.as_str() > FIRST_WEEK || past {
                draw_button(g, &s, r, label, ok, ok && r.has(px, py));
            }
        }
        match &l.board {
            Board::Loading => s.put(g, f, x, BOARD_TOP, tr("STR_MP_BOARD_LOADING"), white),
            Board::Failed(e) => s.put_fit(g, f, x, BOARD_TOP, BOX.w - 48.0, &trf("STR_MP_BOARD_UNREACHABLE", &[("ERROR", e)]), white),
            Board::Rows { rows, me } => {
                if rows.is_empty() {
                    s.put(g, f, x, BOARD_TOP, tr("STR_MP_BOARD_EMPTY"), white);
                }
                for (i, r) in rows.iter().take(ROWS).enumerate() {
                    let y = BOARD_TOP + i as f32 * 18.0;
                    let col = if r.mine { yellow } else { white };
                    s.put(g, f, x, y, &format!("{}.", r.place), col);
                    s.put(g, f, x + 40.0, y, &r.nick, col);
                    s.put(g, f, x + 300.0, y, &score_text(shown.measure, r.score), col);
                }
                if let Some((place, score)) = me {
                    let y = BOARD_TOP + ROWS as f32 * 18.0 + 4.0;
                    s.put(g, f, x, y, &trf("STR_MP_BOARD_YOU", &[("PLACE", place), ("SCORE", &score_text(shown.measure, *score))]), yellow);
                }
            }
        }
        draw_button(g, &s, BACK, tr("STR_MP_BACK"), true, BACK.has(px, py));
        draw_button(g, &s, PLAY, tr("STR_MP_PLAY"), true, PLAY.has(px, py));
        g.flush();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_read_as_english_ordinals() {
        let got: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111].into_iter().map(place_text).collect();
        assert_eq!(got, ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st", "111th"]);
    }

    #[test]
    fn weeks_step_back_and_on() {
        use super::super::challenge::{next_week, previous_week};
        assert_eq!(previous_week("2026-W48"), "2026-W47");
        assert_eq!(previous_week("2027-W01"), "2026-W53");
        assert_eq!(next_week("2026-W53"), "2027-W01");
        assert_eq!(next_week(&previous_week("2026-W10")), "2026-W10");
    }

    #[test]
    fn boards_read() {
        let b = read_board(br#"{"ok":true,"rows":[{"place":1,"nick":"Test Pro","score":40000,"mine":true}],"me":{"place":60,"score":5}}"#);
        assert_eq!(b, Board::Rows { rows: vec![Row { place: 1, nick: "Test Pro".into(), score: 40000, mine: true }], me: Some((60, 5)) });
        assert_eq!(read_board(br#"{"ok":true,"rows":[],"me":null}"#), Board::Rows { rows: vec![], me: None });
        assert!(matches!(read_board(br#"{"ok":false,"error":"bad board"}"#), Board::Failed(e) if e == "bad board"));
        assert!(matches!(read_board(b"nope"), Board::Failed(_)));
    }

    #[test]
    fn typing_follows_the_boards_rules() {
        assert!(allowed(Field::Nick, "Big", ' ') && allowed(Field::Nick, "Big", '-') && !allowed(Field::Nick, "Big", '<'));
        assert!(!allowed(Field::Nick, &"x".repeat(16), 'a'));
        assert!(allowed(Field::Group, "FRIE", 'n') && !allowed(Field::Group, "FRIEND", 'S') && !allowed(Field::Group, "", '-'));
    }

    #[test]
    fn scores_read_as_the_measure() {
        assert_eq!(score_text(Measure::Skill, 1250), "12.50");
        assert_eq!(score_text(Measure::Members, 40), "40");
        assert!(score_text(Measure::Cash, 40000).starts_with('$'));
    }
}
