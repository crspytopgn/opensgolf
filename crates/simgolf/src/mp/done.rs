//! "Challenge complete": the box over the course when the weekly challenge's year ends (the port's own). It comes up
//! whatever the game is doing, a tournament included (the year end ticks on through one), once the year-end report is
//! closed, and the game waits under it. It says the score, whether it went on the board and its place there, and offers
//! the board's top rows, going back to the title menu, or playing on (nothing more is sent, rival.rs sends only at the
//! year's end). A result the board couldn't be reached for can be sent again from here. A result on the board can be
//! shared: Share my result copies a line with the place, the score and the weekly challenge page's address.
use super::challenge::Challenge;
use super::{lobby, settings};
use crate::app::{App, Screen};
use crate::gfx::Gfx;
use crate::info_ui::c15;
use crate::render::Rect;
use crate::ui::{Screen as Ui, F_ARIAL10, F_MANUAL15, F_MANUAL20};
use sg_core::lang::{tr, trf};
use std::cell::RefCell;

/// Board rows the box shows opened out.
const ROWS: usize = 8;
const BOX_W: f32 = 440.0;

/// Where the result stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Sent {
    /// On its way to the board.
    Sending,
    /// On the board (a new best for the week).
    OnBoard,
    /// The board kept the player's better score from an earlier game this week.
    NotBetter,
    /// Never sent, and why (a cheat, a rule-changing extra).
    Kept(String),
    /// The board couldn't be reached, or said no: why, and whether sending again may help.
    Failed(String, bool),
}

struct Done {
    challenge: Challenge,
    score: i64,
    sent: Sent,
    /// The address that sends the result (kept to send it again).
    url: Option<String>,
    request: Option<crate::net::Request>,
    /// The week's board, asked for once the result is in.
    board: Option<lobby::Board>,
    board_request: Option<crate::net::Request>,
    /// The box is up (else the game goes on, unscored).
    up: bool,
    /// The board's rows are shown.
    rows: bool,
    /// The game was paused before the box came up.
    was_paused: bool,
}

thread_local! {
    static DONE: RefCell<Option<Done>> = const { RefCell::new(None) };
}

/// What the box shows: the lines under the title, the board rows, the box and its buttons.
type Layout = (Vec<String>, Vec<(String, bool)>, Rect, Vec<(Button, Rect)>);

/// The box's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Board,
    Again,
    Share,
    Menu,
    Play,
}

/// The player's place on the board (theirs among the rows, else the place the board told them).
fn my_place(b: &lobby::Board) -> Option<i64> {
    match b {
        lobby::Board::Rows { rows, me } => rows.iter().find(|r| r.mine).map(|r| r.place).or(me.map(|m| m.0)),
        _ => None,
    }
}

/// Reads the board's answer to a result.
pub fn answer(body: &[u8]) -> Sent {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    match (v["ok"].as_bool(), v["better"].as_bool(), v["error"].as_str()) {
        (Some(true), Some(true), _) => Sent::OnBoard,
        (Some(true), _, _) => Sent::NotBetter,
        (_, _, Some("update the game")) => Sent::Failed(tr("STR_MP_RESULT_UPDATE").into(), false),
        (_, _, Some(e)) => Sent::Failed(trf("STR_MP_RESULT_REFUSED", &[("ERROR", &e)]), false),
        _ => Sent::Failed(tr("STR_MP_RESULT_UNREADABLE").into(), true),
    }
}

/// The weekly challenge's page on the website (web/weekly.html): this week's goal, its board and the earlier weeks'.
pub const PAGE_URL: &str = "https://crspytopgn.github.io/opensgolf/weekly.html";

/// Share my result's line: the place and score of a new best, the place alone when an earlier game's score stands, the
/// score alone while the place isn't known.
pub fn share_text(c: &Challenge, score: i64, sent: &Sent, place: Option<i64>) -> String {
    let goal = super::rival::measure_text(c.measure);
    let score = lobby::score_text(c.measure, score);
    match (sent, place) {
        (Sent::OnBoard, Some(p)) => trf(
            "STR_MP_SHARE_PLACE_SCORE",
            &[("PLACE", &lobby::place_text(p)), ("WEEK", &c.id), ("GOAL", &goal), ("SCORE", &score), ("URL", &PAGE_URL)],
        ),
        (_, Some(p)) => {
            trf("STR_MP_SHARE_PLACE", &[("PLACE", &lobby::place_text(p)), ("WEEK", &c.id), ("GOAL", &goal), ("URL", &PAGE_URL)])
        }
        _ => trf("STR_MP_SHARE_SCORE", &[("WEEK", &c.id), ("GOAL", &goal), ("SCORE", &score), ("URL", &PAGE_URL)]),
    }
}

fn board_url(c: &Challenge) -> String {
    let s = settings::get();
    let mut url = format!("{}/board?c={}", super::rival::board_url(), c.id);
    let g = settings::group(&s);
    if !g.is_empty() {
        url.push_str(&format!("&g={g}"));
    }
    if settings::is_player(&s.player) {
        url.push_str(&format!("&p={}", s.player));
    }
    url
}

impl App {
    /// The challenge's year is over (rival.rs): the box comes up, with the result on its way when `url` is given, else
    /// kept back for `why`.
    pub fn challenge_done(&mut self, c: &Challenge, score: i64, url: Result<String, String>) {
        super::texts();
        let (sent, url) = match url {
            Ok(u) => (Sent::Sending, Some(u)),
            Err(why) => (Sent::Kept(why), None),
        };
        let request = url.as_ref().map(|u| crate::net::get(u, 4096));
        let was_paused = self.paused;
        self.paused = true;
        DONE.with(|d| {
            *d.borrow_mut() = Some(Done {
                challenge: c.clone(),
                score,
                sent,
                url,
                request,
                board: None,
                board_request: None,
                up: true,
                rows: false,
                was_paused,
            })
        });
    }

    /// Whether the game being played is a finished challenge played on (nothing more is sent).
    pub fn challenge_played_on(&self) -> bool {
        DONE.with(|d| d.borrow().as_ref().is_some_and(|d| !d.up))
            && super::rival::challenge_of(self.scen.goals.scenario.as_deref()).is_some()
    }

    /// Each frame: the board's answers. A new game (or one loaded) lets the box go.
    pub fn challenge_done_poll(&mut self) {
        let scenario = self.scen.goals.scenario.clone();
        DONE.with(|d| {
            let mut d = d.borrow_mut();
            if d.as_ref().is_some_and(|x| super::rival::challenge_of(scenario.as_deref()).is_none_or(|c| c.id != x.challenge.id)) {
                *d = None;
            }
            let Some(x) = d.as_mut() else { return };
            if let Some(reply) = x.request.as_mut().and_then(|r| r.poll()) {
                x.request = None;
                x.sent = match reply {
                    Ok(body) => answer(&body),
                    Err(e) => Sent::Failed(trf("STR_MP_BOARD_UNREACHABLE", &[("ERROR", &e)]), true),
                };
                println!("rival: result {:?}", x.sent);
                if matches!(x.sent, Sent::OnBoard | Sent::NotBetter) {
                    x.board_request = Some(crate::net::get(&board_url(&x.challenge), 64 * 1024));
                }
            }
            if let Some(reply) = x.board_request.as_mut().and_then(|r| r.poll()) {
                x.board_request = None;
                x.board = Some(match reply {
                    Ok(body) => lobby::read_board(&body),
                    Err(e) => lobby::Board::Failed(e),
                });
            }
        });
    }

    fn done_box(&self, lines: usize, rows: usize) -> Rect {
        let h = 132.0 + 18.0 * lines as f32 + if rows > 0 { 8.0 + 14.0 * rows as f32 } else { 0.0 };
        Rect::new(400.0 - BOX_W / 2.0, (300.0 - h / 2.0).max(40.0), BOX_W, h)
    }

    /// The lines under the title, the board rows shown, and the buttons with their places.
    fn done_layout(&self, x: &Done) -> Layout {
        let c = &x.challenge;
        let m = c.measure;
        let mut lines = vec![
            trf("STR_MP_DONE_WEEK", &[("WEEK", &c.id), ("GOAL", &super::rival::measure_text(m)), ("YEAR", &c.end_year)]),
            trf("STR_MP_DONE_SCORE", &[("SCORE", &lobby::score_text(m, x.score))]),
        ];
        let place = x.board.as_ref().and_then(my_place);
        lines.push(match (&x.sent, place) {
            (Sent::Sending, _) => tr("STR_MP_DONE_SENDING").into(),
            (Sent::OnBoard, Some(p)) => trf("STR_MP_DONE_ON_BOARD_PLACE", &[("PLACE", &lobby::place_text(p))]),
            (Sent::OnBoard, None) => tr("STR_MP_RESULT_ON_BOARD").into(),
            (Sent::NotBetter, Some(p)) => trf("STR_MP_DONE_NOT_BETTER_PLACE", &[("PLACE", &lobby::place_text(p))]),
            (Sent::NotBetter, None) => tr("STR_MP_RESULT_NOT_BETTER").into(),
            (Sent::Kept(why), _) => why.clone(),
            (Sent::Failed(why, _), _) => why.clone(),
        });
        let rows: Vec<(String, bool)> = if x.rows {
            match &x.board {
                Some(lobby::Board::Rows { rows, .. }) if !rows.is_empty() => rows
                    .iter()
                    .take(ROWS)
                    .map(|r| (format!("{}. {}", r.place, r.nick), r.mine, lobby::score_text(m, r.score)))
                    .map(|(a, mine, s)| (format!("{a}\t{s}"), mine))
                    .collect(),
                Some(lobby::Board::Rows { .. }) => vec![(tr("STR_MP_BOARD_EMPTY").into(), false)],
                Some(lobby::Board::Failed(e)) => vec![(e.clone(), false)],
                _ => vec![(tr("STR_MP_BOARD_LOADING").into(), false)],
            }
        } else {
            Vec::new()
        };
        let b = self.done_box(lines.len(), rows.len());
        let first = match &x.sent {
            Sent::Failed(_, true) => Some(Button::Again),
            Sent::OnBoard | Sent::NotBetter if !x.rows => Some(Button::Board),
            _ => None,
        };
        let share = matches!(x.sent, Sent::OnBoard | Sent::NotBetter).then_some(Button::Share);
        let buttons: Vec<Button> = first.into_iter().chain(share).chain([Button::Menu, Button::Play]).collect();
        let (bw, gap) = if buttons.len() > 3 { (98.0, 8.0) } else { (130.0, 10.0) };
        let n = buttons.len() as f32;
        let x0 = 400.0 - (n * bw + (n - 1.0) * gap) / 2.0;
        let placed =
            buttons.into_iter().enumerate().map(|(i, k)| (k, Rect::new(x0 + i as f32 * (bw + gap), b.y + b.h - 62.0, bw, 26.0))).collect();
        (lines, rows, b, placed)
    }

    fn done_up(&self) -> bool {
        self.screen == Screen::Play && DONE.with(|d| d.borrow().as_ref().is_some_and(|d| d.up))
    }

    /// Draws the box (over the course, centred).
    pub fn draw_challenge_done(&self, g: &mut Gfx, s: &Ui) {
        if !self.done_up() {
            return;
        }
        DONE.with(|d| {
            let d = d.borrow();
            let Some(x) = d.as_ref() else { return };
            let (lines, rows, b, buttons) = self.done_layout(x);
            s.fill(g, -2000.0, -2000.0, 4800.0, 4600.0, crate::ui::rgba(0.0, 0.0, 0.0, 0.35));
            self.draw_info_frame(g, s, b.x, b.y, b.w, b.h);
            let (white, yellow, navy, pale) = (c15(0x7fff), c15(0x7ff0), c15(0x0018), c15(0x6318));
            s.put_shadowed(g, F_MANUAL20, 400.0, b.y + 16.0, tr("STR_MP_DONE_TITLE"), yellow, true);
            let mut y = b.y + 52.0;
            for (i, l) in lines.iter().enumerate() {
                let f = if i == 1 { F_MANUAL15 } else { F_ARIAL10 };
                let c = if i == 1 { white } else { navy };
                s.put_centered(g, f, 400.0, y, &crate::content::fit(l, b.w - 40.0), c);
                y += 18.0 + if i == 1 { 4.0 } else { 0.0 };
            }
            if !rows.is_empty() {
                y += 6.0;
                for (l, mine) in &rows {
                    let c = if *mine { yellow } else { white };
                    let (left, right) = l.split_once('\t').unwrap_or((l.as_str(), ""));
                    s.put(g, F_ARIAL10, b.x + 90.0, y, &crate::content::fit(left, 200.0), c);
                    s.put(g, F_ARIAL10, b.x + 300.0, y, right, c);
                    y += 14.0;
                }
            }
            let (px, py) = self.info.pointer;
            for (k, r) in &buttons {
                let label = match k {
                    Button::Board => tr("STR_MP_DONE_VIEW_BOARD"),
                    Button::Again => tr("STR_MP_DONE_AGAIN"),
                    Button::Share => tr("STR_MP_DONE_SHARE"),
                    Button::Menu => tr("STR_MP_DONE_MENU"),
                    Button::Play => tr("STR_MP_DONE_PLAY_ON"),
                };
                crate::glass_ui::button_fill(g, s, r.x, r.y, r.w, r.h, r.has(px, py));
                s.put_centered(g, F_ARIAL10, r.x + r.w / 2.0, r.y + 7.0, &crate::content::fit(label, r.w - 8.0), navy);
            }
            s.put_centered(g, F_ARIAL10, 400.0, b.y + b.h - 28.0, &crate::content::fit(tr("STR_MP_DONE_PLAY_ON_NOTE"), b.w - 30.0), pale);
        });
    }

    /// A click while the box is up: always the box's (true), whatever it lands on.
    pub fn challenge_done_click(&mut self, vx: f32, vy: f32) -> bool {
        if !self.done_up() {
            return false;
        }
        let hit = DONE.with(|d| {
            let d = d.borrow();
            let x = d.as_ref()?;
            let (_, _, _, buttons) = self.done_layout(x);
            buttons.into_iter().find(|(_, r)| r.has(vx, vy)).map(|(k, _)| k)
        });
        if let Some(k) = hit {
            self.done_button(k);
        }
        true
    }

    /// Keys while the box is up: Esc or Enter plays on; the rest wait (true: the key was the box's).
    pub fn challenge_done_key(&mut self, k: miniquad::KeyCode) -> bool {
        if !self.done_up() {
            return false;
        }
        if matches!(k, miniquad::KeyCode::Escape | miniquad::KeyCode::Enter | miniquad::KeyCode::KpEnter) {
            self.done_button(Button::Play);
        }
        true
    }

    fn done_button(&mut self, k: Button) {
        self.ui_sound(0x18);
        if k == Button::Share {
            let text =
                DONE.with(|d| d.borrow().as_ref().map(|x| share_text(&x.challenge, x.score, &x.sent, x.board.as_ref().and_then(my_place))));
            if let Some(text) = text {
                miniquad::window::clipboard_set(&text);
                self.show_toast(tr("STR_MP_SHARE_COPIED"));
            }
            return;
        }
        let was_paused = DONE.with(|d| {
            let mut d = d.borrow_mut();
            let x = d.as_mut()?;
            match k {
                Button::Board => {
                    x.rows = true;
                    if !matches!(x.board, Some(lobby::Board::Rows { .. })) && x.board_request.is_none() {
                        x.board = None;
                        x.board_request = Some(crate::net::get(&board_url(&x.challenge), 64 * 1024));
                    }
                }
                Button::Again => {
                    if let Some(u) = &x.url {
                        x.sent = Sent::Sending;
                        x.request = Some(crate::net::get(u, 4096));
                    }
                }
                Button::Share => {}
                Button::Menu | Button::Play => {
                    x.up = false;
                    return Some(x.was_paused);
                }
            }
            None
        });
        let Some(was_paused) = was_paused else { return };
        self.paused = was_paused;
        if k == Button::Menu {
            // the game is kept as the title's quit keeps it (an autosave in the Load list)
            self.end_career();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boards_answers_read() {
        super::super::texts();
        assert_eq!(answer(br#"{"ok":true,"better":true}"#), Sent::OnBoard);
        assert_eq!(answer(br#"{"ok":true,"better":false}"#), Sent::NotBetter);
        assert!(matches!(answer(br#"{"ok":false,"error":"cheats were on"}"#), Sent::Failed(_, false)));
        assert!(matches!(answer(b"<html>"), Sent::Failed(_, true)));
    }

    #[test]
    fn the_shared_line_has_the_place_score_and_page() {
        super::super::texts();
        let c = super::super::challenge::challenge("2026-W49");
        assert_eq!(
            share_text(&c, 125_000, &Sent::OnBoard, Some(3)),
            "I'm 3rd in OpenSGolf's weekly challenge 2026-W49 (the most cash) with $125,000. Can you beat it? https://crspytopgn.github.io/opensgolf/weekly.html"
        );
        assert!(share_text(&c, 1, &Sent::NotBetter, Some(2)).starts_with("I'm 2nd in OpenSGolf's weekly challenge 2026-W49"));
        assert!(share_text(&c, 125_000, &Sent::OnBoard, None).starts_with("I scored $125,000 in"));
    }

    #[test]
    fn the_place_is_the_players_row_or_the_one_told() {
        let row = |place, mine| lobby::Row { place, nick: "x".into(), score: 1, mine };
        assert_eq!(my_place(&lobby::Board::Rows { rows: vec![row(1, false), row(2, true)], me: None }), Some(2));
        assert_eq!(my_place(&lobby::Board::Rows { rows: vec![row(1, false)], me: Some((60, 5)) }), Some(60));
        assert_eq!(my_place(&lobby::Board::Loading), None);
    }
}
