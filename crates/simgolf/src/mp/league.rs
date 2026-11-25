//! Neighbours leagues: 2 to 4 players each run their own course on their own plot of the same property, each on their own
//! computer, and the league's table (the Worker's league.js) compares them as they play. A league game reports its numbers
//! each game month and, every few months, uploads its latest save, which the neighbours can download to look round.
//! Nothing is played in step with anyone: each plot is a game of its own.
//!
//! The page (Start New Game's "Neighbours" row) lists the player's leagues and a league's table, makes a league, joins one
//! by its code, plays the player's own plot and visits a neighbour's.

use super::challenge::Measure;
use super::club::server;
use super::settings;
use crate::app::App;
use crate::extras::draw_button;
use crate::gfx::Gfx;
use crate::info_ui::c15;
use crate::render::Rect;
use crate::scenario_ui::ScenarioEntry;
use crate::ui::{Screen as Ui, F_ARIAL10, F_INFO14, F_MANUAL20};
use sg_core::land::{ExeRng, Noise};
use sg_core::lang::{tr, trf};
use sg_core::scenario::{Difficulty, GoalDef, Scenario, ScenarioDef};
use std::cell::RefCell;

/// The Start New Game row's id.
pub const ROW_ID: &str = "*neighbours";
/// A league game's scenario id: this, the league's code, '/', the player's seat.
pub const ID_PREFIX: &str = "*league/";
/// A neighbour's course being looked round: this and the league's code (nothing is reported from it).
pub const VISIT_PREFIX: &str = "*visit/";
/// Players in a league (league.js LEAGUE_SEATS).
pub const SEATS: usize = 4;
/// A league's season ends when this game year does (league.js LEAGUE_END_YEAR).
pub const END_YEAR: i32 = 2002;
/// Game months between uploads of the save.
pub const SAVE_EVERY_MONTHS: i32 = 2;
/// Leagues remembered.
pub const MY_LEAGUES: usize = 8;

const BOX: Rect = Rect { x: 40.0, y: 36.0, w: 720.0, h: 528.0 };
const ROW_H: f32 = 20.0;
const MINE_TOP: f32 = BOX.y + 98.0;
const TABLE_TOP: f32 = BOX.y + 212.0;
const PLAY: Rect = Rect { x: BOX.x + 24.0, y: BOX.y + 318.0, w: 200.0, h: 22.0 };
const VISIT: Rect = Rect { x: BOX.x + 236.0, y: BOX.y + 318.0, w: 220.0, h: 22.0 };
const NAME: Rect = Rect { x: BOX.x + 150.0, y: BOX.y + BOX.h - 154.0, w: 230.0, h: 22.0 };
const PROPERTY: Rect = Rect { x: BOX.x + 24.0, y: BOX.y + BOX.h - 122.0, w: 200.0, h: 22.0 };
const DIFFICULTY: Rect = Rect { x: BOX.x + 236.0, y: BOX.y + BOX.h - 122.0, w: 150.0, h: 22.0 };
const MEASURE: Rect = Rect { x: BOX.x + 398.0, y: BOX.y + BOX.h - 122.0, w: 140.0, h: 22.0 };
const MAKE: Rect = Rect { x: BOX.x + 550.0, y: BOX.y + BOX.h - 122.0, w: 146.0, h: 22.0 };
const CODE: Rect = Rect { x: BOX.x + 150.0, y: BOX.y + BOX.h - 82.0, w: 120.0, h: 22.0 };
const JOIN: Rect = Rect { x: BOX.x + 282.0, y: BOX.y + BOX.h - 82.0, w: 150.0, h: 22.0 };
const BACK: Rect = Rect { x: BOX.x + 24.0, y: BOX.y + BOX.h - 46.0, w: 100.0, h: 22.0 };

/// A league, as the Worker has it.
#[derive(Clone, Debug, PartialEq)]
pub struct League {
    pub code: String,
    pub name: String,
    pub property: usize,
    pub difficulty: u8,
    pub measure: Measure,
    pub seed: u32,
}

/// A player's row in a league's table.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub seat: usize,
    pub nick: String,
    pub facts: Option<sg_core::scenario::Facts>,
    pub month: i32,
    pub save: bool,
    pub mine: bool,
}

/// A league and its players.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub league: League,
    pub rows: Vec<Row>,
}

impl Table {
    /// The player's own seat.
    pub fn my_seat(&self) -> Option<usize> {
        self.rows.iter().find(|r| r.mine).map(|r| r.seat)
    }

    /// The rows in the table's order: the league's measure, best first (those with no numbers yet last).
    pub fn ranked(&self) -> Vec<&Row> {
        let mut v: Vec<&Row> = self.rows.iter().collect();
        let m = self.league.measure;
        v.sort_by_key(|r| (r.facts.is_none(), std::cmp::Reverse(r.facts.as_ref().map(|f| m.score(f)).unwrap_or(0)), r.seat));
        v
    }
}

pub fn measure_of(id: &str) -> Option<Measure> {
    Measure::ALL.into_iter().find(|m| m.id() == id)
}

/// The Worker's answer with a league and its players (null: not one).
pub fn read_table(body: &[u8]) -> Result<Table, String> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| tr("STR_MP_CHECK_CONNECTION").to_string())?;
    if v["ok"].as_bool() != Some(true) {
        return Err(v["error"].as_str().unwrap_or("no league").to_string());
    }
    let l = &v["league"];
    let league = (|| {
        Some(League {
            code: l["code"].as_str()?.to_string(),
            name: l["name"].as_str()?.to_string(),
            property: l["property"].as_u64()? as usize,
            difficulty: l["difficulty"].as_u64()? as u8,
            measure: measure_of(l["measure"].as_str()?)?,
            seed: l["seed"].as_u64()? as u32,
        })
    })()
    .ok_or_else(|| "not a league".to_string())?;
    let rows = v["players"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let f = &p["facts"];
                    let facts = f.is_object().then(|| sg_core::scenario::Facts {
                        year: f["year"].as_i64().unwrap_or(2001) as i32,
                        cash: f["cash"].as_i64().unwrap_or(0),
                        holes: f["holes"].as_i64().unwrap_or(0) as i32,
                        fun: f["fun"].as_i64().unwrap_or(0) as i32,
                        skill: f["skill"].as_i64().unwrap_or(0) as i32,
                        members: f["members"].as_i64().unwrap_or(0) as i32,
                        rank: f["rank"].as_i64().unwrap_or(0) as i32,
                    });
                    Some(Row {
                        seat: p["seat"].as_u64()? as usize,
                        nick: p["nick"].as_str()?.to_string(),
                        month: f["month"].as_i64().unwrap_or(0) as i32,
                        facts,
                        save: p["save"].as_bool().unwrap_or(false),
                        mine: p["mine"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Table { league, rows })
}

/// A game date: its month (0 is March: the game's year runs March to October) and year.
pub fn date_text(month: i32, year: i32) -> String {
    let months: Vec<&str> = tr("STR_MP_LEAGUE_MONTHS").split(',').collect();
    format!("{} {year}", months.get(month.clamp(0, 7) as usize).copied().unwrap_or("").trim())
}

/// A seat's own land: the league's seed made different for each plot.
pub fn seat_seed(seed: u32, seat: usize) -> u32 {
    (seed ^ (seat as u32).wrapping_mul(0x9e37_79b1) >> 8) & 0x00ff_ffff
}

/// The league game's scenario id.
pub fn game_id(code: &str, seat: usize) -> String {
    format!("{ID_PREFIX}{code}/{seat}")
}

/// (code, seat) of a league game's scenario id.
pub fn league_of(scenario: Option<&str>) -> Option<(String, usize)> {
    let rest = scenario?.strip_prefix(ID_PREFIX)?;
    let (code, seat) = rest.split_once('/')?;
    Some((code.to_string(), seat.parse().ok()?))
}

/// Whether a code looks like a league's (league.js isLeagueCode).
pub fn is_code(c: &str) -> bool {
    c.len() == 6 && c.bytes().all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&b))
}

/// The league's own file of the player's plot (kept each time the save is sent), which Play goes on with.
pub fn own_save_path(code: &str) -> std::path::PathBuf {
    crate::app::save_dir().join("Leagues").join(format!("League {code}.sgs"))
}

/// The league as a scenario: its name, and the one goal saying what the table sorts by.
pub fn scenario(l: &League) -> Scenario {
    super::texts();
    let goal = super::rival::measure_text(l.measure);
    Scenario {
        scenario: ScenarioDef {
            name: Some(trf("STR_MP_LEAGUE_GAME", &[("LEAGUE", &l.name)])),
            description: trf("STR_MP_LEAGUE_GOAL", &[("GOAL", &goal), ("YEAR", &END_YEAR)]),
            property: Some(sg_core::data::property_id(l.property)),
            difficulty: Some(Difficulty::from_level(l.difficulty as i32)),
            intro: trf("STR_MP_LEAGUE_INTRO", &[("LEAGUE", &l.name), ("GOAL", &goal), ("YEAR", &END_YEAR)]),
            ..Default::default()
        },
        goals: vec![GoalDef {
            id: Some("league".into()),
            text: trf("STR_MP_LEAGUE_GOAL", &[("GOAL", &goal), ("YEAR", &END_YEAR)]),
            ..Default::default()
        }],
    }
}

/// The row Start New Game adds.
pub fn entry() -> ScenarioEntry {
    super::texts();
    let sc = Scenario {
        scenario: ScenarioDef {
            name: Some(tr("STR_MP_LEAGUES").into()),
            description: tr("STR_MP_LEAGUES_ROW").into(),
            // (set so Start New Game opens the page instead of Select Difficulty)
            difficulty: Some(Difficulty::Easy),
            ..Default::default()
        },
        goals: Vec::new(),
    };
    ScenarioEntry { id: ROW_ID.into(), name: tr("STR_MP_LEAGUES").into(), description: sc.scenario.description.clone(), def: Ok(sc) }
}

/// The Start New Game row's line (other rows: None).
pub fn row_line(id: &str) -> Option<String> {
    super::texts();
    (id == ROW_ID).then(|| tr("STR_MP_LEAGUES_ROW").to_string())
}

// ---------------------------------------------------------------- requests

#[derive(Clone, Copy, Debug, PartialEq)]
enum Asked {
    Table,
    Make,
    Join,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
    static ASK: RefCell<Option<(Asked, crate::net::Request)>> = const { RefCell::new(None) };
    static SENT: RefCell<Vec<crate::net::Request>> = const { RefCell::new(Vec::new()) };
    static UPLOADS: RefCell<Vec<crate::net::Request>> = const { RefCell::new(Vec::new()) };
    static VISITING: RefCell<Option<(String, String, crate::net::Request)>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Field {
    Name,
    Code,
}

#[derive(Clone)]
struct Page {
    /// The league whose table is shown (its code).
    shown: Option<String>,
    table: Option<Table>,
    busy: bool,
    /// The neighbour picked in the table (a seat).
    picked: Option<usize>,
    name: String,
    property: usize,
    difficulty: u8,
    measure: Measure,
    code: String,
    editing: Option<Field>,
    note: String,
}

fn page() -> Option<Page> {
    PAGE.with(|p| p.borrow().clone())
}

fn change(f: impl FnOnce(&mut Page)) {
    PAGE.with(|p| {
        if let Some(p) = p.borrow_mut().as_mut() {
            f(p)
        }
    })
}

pub fn is_open() -> bool {
    PAGE.with(|p| p.borrow().is_some())
}

/// Whether a field is being typed in (a phone shows its keyboard: touch.rs).
pub fn editing() -> bool {
    PAGE.with(|p| p.borrow().as_ref().is_some_and(|p| p.editing.is_some()))
}

fn ask(what: Asked, url: String) {
    ASK.with(|a| *a.borrow_mut() = Some((what, crate::net::get(&url, 64 * 1024))));
    change(|p| p.busy = true);
}

fn ask_table(code: &str) {
    let s = settings::get();
    change(|p| {
        p.shown = Some(code.to_string());
        p.picked = None;
    });
    ask(Asked::Table, format!("{}/leagues/{code}?p={}", server(), s.player));
}

/// The JSON in a GET's `d`.
fn d(v: serde_json::Value) -> String {
    super::result::base64url(v.to_string().as_bytes())
}

/// Remembers a league (first in the list).
fn remember(code: &str, name: &str) {
    let mut s = settings::get();
    s.leagues.retain(|c| c.id != code);
    s.leagues.insert(0, settings::MyClub { id: code.into(), name: name.into() });
    s.leagues.truncate(MY_LEAGUES);
    settings::save(s);
}

/// The month's numbers sent, at most one in flight each.
fn poll_sent() {
    SENT.with(|s| s.borrow_mut().retain_mut(|r| r.poll().is_none()));
    UPLOADS.with(|u| u.borrow_mut().retain_mut(|u| u.poll().is_none()));
}

impl App {
    pub fn open_leagues_page(&mut self) -> bool {
        let s = settings::get();
        PAGE.with(|p| {
            *p.borrow_mut() = Some(Page {
                shown: None,
                table: None,
                busy: false,
                picked: None,
                name: String::new(),
                property: 0,
                difficulty: 1,
                measure: Measure::Fun,
                code: String::new(),
                editing: None,
                note: String::new(),
            })
        });
        if let Some(first) = s.leagues.first() {
            ask_table(&first.id);
        }
        self.hover = -1;
        true
    }

    fn close_leagues_page(&mut self) {
        PAGE.with(|p| *p.borrow_mut() = None);
        ASK.with(|a| *a.borrow_mut() = None);
        self.hover = -1;
    }

    fn leagues_turn_on(&self) {
        let mut s = settings::with_player();
        if !s.on {
            s.on = true;
            settings::save(s);
        }
    }

    fn leagues_make(&mut self) {
        let Some(p) = page() else { return };
        let name = p.name.trim().replace("  ", " ");
        if name.chars().count() < 3 {
            self.show_toast(tr("STR_MP_LEAGUE_NAME_FIRST"));
            change(|p| p.editing = Some(Field::Name));
            return;
        }
        self.leagues_turn_on();
        let s = settings::with_player();
        let body = serde_json::json!({
            "protocol": super::version::PROTOCOL, "player": s.player, "nick": settings::nick(&s), "name": name,
            "property": p.property, "difficulty": p.difficulty, "measure": p.measure.id(),
        });
        ask(Asked::Make, format!("{}/leagues/new?d={}", server(), d(body)));
    }

    fn leagues_join(&mut self) {
        let Some(p) = page() else { return };
        let code = p.code.trim().to_uppercase();
        if !is_code(&code) {
            self.show_toast(tr("STR_MP_LEAGUE_CODE_FIRST"));
            change(|p| p.editing = Some(Field::Code));
            return;
        }
        self.leagues_turn_on();
        let s = settings::with_player();
        let body = serde_json::json!({ "protocol": super::version::PROTOCOL, "player": s.player, "nick": settings::nick(&s) });
        ask(Asked::Join, format!("{}/leagues/{code}/join?d={}", server(), d(body)));
    }

    /// Plays the player's own plot: on from the league's own file, or a new game on the plot's land.
    fn leagues_play(&mut self, g: &mut Gfx) {
        let Some(t) = page().and_then(|p| p.table) else { return };
        let Some(seat) = t.my_seat() else { return };
        let path = own_save_path(&t.league.code);
        self.close_leagues_page();
        if sg_core::fsutil::exists(&path) {
            match self.load_game(g, &path) {
                Ok(()) => return,
                Err(e) => println!("league: {} could not be loaded ({e}): a new game", path.display()),
            }
        }
        self.start_league(g, &t.league, seat);
    }

    /// A new game on a seat's plot.
    pub fn start_league(&mut self, g: &mut Gfx, l: &League, seat: usize) {
        self.scen.chosen = None;
        self.course_size = sg_core::mapsize::ORIGINAL;
        self.exe_rng = ExeRng { state: seat_seed(l.seed, seat) };
        self.noise = Noise::new(&mut self.exe_rng);
        self.difficulty = l.difficulty as i32;
        self.course_id = Some(self.exe_rng.below(0xffff));
        self.econ.sandbox = false;
        self.deal_offer(false);
        let sc = scenario(l);
        let prop = sc.property_index();
        self.cheap_slot(prop);
        self.scen.start_money = None;
        println!("league: {} seat {seat} ({}, seed {})", l.code, sg_core::data::property_id(l.property), seat_seed(l.seed, seat));
        self.scen.starting = Some((game_id(&l.code, seat), sc));
        self.start_game(g, prop, false);
        // the same people and stories on every plot, whatever each player's files (as the weekly challenge)
        self.take_shared_tables(super::rival::tables());
    }

    /// Looks round a neighbour's course: their latest save, downloaded, played as a copy nothing is sent from.
    fn leagues_visit(&mut self) {
        let Some(p) = page() else { return };
        let (Some(t), Some(seat)) = (p.table, p.picked) else { return };
        let Some(r) = t.rows.iter().find(|r| r.seat == seat) else { return };
        if !r.save {
            self.show_toast(&trf("STR_MP_LEAGUE_NO_SAVE", &[("NICK", &r.nick)]));
            return;
        }
        let url = format!("{}/leagues/{}/save/{seat}", server(), t.league.code);
        VISITING.with(|v| *v.borrow_mut() = Some((t.league.code.clone(), r.nick.clone(), crate::net::get(&url, 1 << 20))));
        change(|p| p.busy = true);
    }

    /// Each frame on the page: answers in.
    fn leagues_poll(&mut self, g: &mut Gfx) {
        poll_sent();
        if let Some((what, reply)) = ASK.with(|a| {
            let mut a = a.borrow_mut();
            let r = a.as_mut()?.1.poll()?;
            let what = a.take()?.0;
            Some((what, r))
        }) {
            change(|p| p.busy = false);
            match reply.map_err(|e| {
                println!("league: {e}");
                tr("STR_MP_CHECK_CONNECTION").to_string()
            }) {
                Ok(body) => match read_table(&body) {
                    Ok(t) => {
                        if what != Asked::Table || t.my_seat().is_some() {
                            remember(&t.league.code, &t.league.name);
                        }
                        if what == Asked::Make {
                            self.show_toast(&trf("STR_MP_LEAGUE_MADE", &[("CODE", &t.league.code)]));
                        }
                        change(|p| {
                            p.shown = Some(t.league.code.clone());
                            p.table = Some(t);
                            p.code.clear();
                            p.name.clear();
                            p.note.clear();
                        });
                    }
                    Err(e) => change(|p| p.note = trf("STR_MP_LEAGUE_REFUSED", &[("WHY", &e)])),
                },
                Err(e) => change(|p| p.note = e),
            }
        }
        if let Some((code, nick, reply)) = VISITING.with(|v| {
            let mut v = v.borrow_mut();
            let r = v.as_mut()?.2.poll()?;
            let (code, nick, _) = v.take()?;
            Some((code, nick, r))
        }) {
            change(|p| p.busy = false);
            let loaded = reply.and_then(|b| crate::app::read_save_bytes(&b, &nick)).and_then(|s| {
                self.close_leagues_page();
                self.load_save(g, s)
            });
            match loaded {
                Ok(()) => {
                    self.scen.goals.scenario = Some(format!("{VISIT_PREFIX}{code}"));
                    self.show_toast(&trf("STR_MP_LEAGUE_VISITING", &[("NICK", &nick)]));
                }
                Err(e) => self.show_toast(&trf("STR_MP_LEAGUE_REFUSED", &[("WHY", &e)])),
            }
        }
    }

    /// Each game month: a league game's numbers to its table, and now and then its save (kept in the league's own file
    /// too). Nothing from a game with cheats, a neighbour's course being looked round, or after the season.
    pub fn league_month(&mut self) {
        poll_sent();
        let Some((code, _seat)) = league_of(self.scen.goals.scenario.as_deref()) else { return };
        if self.club.cheated {
            return;
        }
        let f = crate::scenario_ui::facts(self);
        let month = self.econ.month_index();
        if f.year > END_YEAR + 1 || (f.year == END_YEAR + 1 && month % 8 != 0) {
            return;
        }
        let s = settings::with_player();
        let body = serde_json::json!({
            "player": s.player, "cheats": false,
            "facts": { "year": f.year, "month": month % 8, "cash": f.cash, "holes": f.holes, "fun": f.fun, "skill": f.skill,
                       "members": f.members, "rank": f.rank },
        });
        let url = format!("{}/leagues/{code}/report?d={}", server(), d(body));
        SENT.with(|r| r.borrow_mut().push(crate::net::get(&url, 4096)));
        if month % SAVE_EVERY_MONTHS == 0 || f.year == END_YEAR + 1 {
            let path = own_save_path(&code);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = self.save_game(&path) {
                println!("league: {} not kept: {e}", path.display());
            }
            if let Ok(bytes) = self.save_bytes(6) {
                let url = format!("{}/leagues/{code}/save?p={}", server(), s.player);
                UPLOADS.with(|u| u.borrow_mut().push(crate::net::post(&url, bytes)));
            }
        }
        if f.year == END_YEAR + 1 && month % 8 == 0 {
            self.show_toast(tr("STR_MP_LEAGUE_OVER"));
        }
    }

    pub fn leagues_click(&mut self, g: &mut Gfx, x: f32, y: f32) -> bool {
        let Some(p) = page() else { return false };
        change(|p| {
            p.editing = if NAME.has(x, y) {
                Some(Field::Name)
            } else if CODE.has(x, y) {
                Some(Field::Code)
            } else {
                None
            }
        });
        let mine = settings::get().leagues;
        if MAKE.has(x, y) {
            self.leagues_make();
        } else if JOIN.has(x, y) {
            self.leagues_join();
        } else if PROPERTY.has(x, y) {
            change(|p| p.property = (p.property + 1) % 16);
        } else if DIFFICULTY.has(x, y) {
            change(|p| p.difficulty = (p.difficulty + 1) % 4);
        } else if MEASURE.has(x, y) {
            change(|p| p.measure = Measure::ALL[(Measure::ALL.iter().position(|m| *m == p.measure).unwrap_or(0) + 1) % 4]);
        } else if PLAY.has(x, y) {
            self.leagues_play(g);
        } else if VISIT.has(x, y) {
            self.leagues_visit();
        } else if BACK.has(x, y) || !BOX.has(x, y) {
            self.close_leagues_page();
        } else if y >= MINE_TOP && y < MINE_TOP + ROW_H * 4.0 {
            if let Some(l) = mine.get(((y - MINE_TOP) / ROW_H) as usize) {
                change(|p| p.table = None);
                ask_table(&l.id);
            }
        } else if let Some(t) = &p.table {
            if y >= TABLE_TOP && y < TABLE_TOP + ROW_H * SEATS as f32 {
                let k = ((y - TABLE_TOP) / ROW_H) as usize;
                if let Some(r) = t.ranked().get(k) {
                    let seat = r.seat;
                    change(|p| p.picked = Some(seat));
                }
            }
        }
        true
    }

    pub fn leagues_key(&mut self, g: &mut Gfx, k: miniquad::KeyCode) -> bool {
        use miniquad::KeyCode;
        let Some(p) = page() else { return false };
        match (p.editing, k) {
            (Some(f), KeyCode::Backspace) => change(|p| {
                if f == Field::Name {
                    p.name.pop();
                } else {
                    p.code.pop();
                }
            }),
            (Some(Field::Name), KeyCode::Enter | KeyCode::KpEnter) => self.leagues_make(),
            (Some(Field::Code), KeyCode::Enter | KeyCode::KpEnter) => self.leagues_join(),
            (Some(_), KeyCode::Escape | KeyCode::Tab) => change(|p| p.editing = None),
            (Some(_), _) => {}
            (None, KeyCode::Escape) => self.close_leagues_page(),
            (None, KeyCode::Enter | KeyCode::KpEnter) => self.leagues_play(g),
            _ => {}
        }
        true
    }

    pub fn leagues_char(&mut self, c: char) -> bool {
        if !is_open() {
            return false;
        }
        change(|p| match p.editing {
            Some(Field::Name) if super::clubs_ui::name_allowed(&p.name, c) => p.name.push(c),
            Some(Field::Code) if p.code.len() < 6 && c.is_ascii_alphanumeric() => p.code.push(c.to_ascii_uppercase()),
            _ => {}
        });
        true
    }

    pub fn leagues_draw(&mut self, g: &mut Gfx) -> bool {
        if !is_open() {
            return false;
        }
        self.leagues_poll(g);
        let Some(p) = page() else { return true };
        let s = Ui::new(self.draw_w, self.draw_h);
        self.view = s.view;
        crate::info_ui::dim(g, &s);
        self.draw_info_frame(g, &s, BOX.x, BOX.y, BOX.w, BOX.h);
        let (white, yellow, navy, grey) = (c15(0x7fff), c15(0x7ff0), c15(0x0018), c15(0x6318));
        let (px, py) = self.info.pointer;
        let f = crate::textsize::big(F_INFO14);
        let x = BOX.x + 24.0;
        s.put_shadowed(g, F_MANUAL20, 400.0, BOX.y + 14.0, tr("STR_MP_LEAGUES"), yellow, true);
        s.put_fit(g, f, x, BOX.y + 46.0, BOX.w - 48.0, tr("STR_MP_LEAGUES_INTRO"), white);
        s.put_fit(g, F_ARIAL10, x, BOX.y + 66.0, BOX.w - 48.0, &trf("STR_MP_LEAGUES_INTRO2", &[("YEAR", &END_YEAR)]), navy);
        // the player's leagues
        s.put(g, F_ARIAL10, x, MINE_TOP - 16.0, tr("STR_MP_LEAGUES_MINE"), navy);
        let mine = settings::get().leagues;
        if mine.is_empty() {
            s.put(g, f, x, MINE_TOP, tr("STR_MP_LEAGUES_NONE"), white);
        }
        for (i, l) in mine.iter().take(4).enumerate() {
            let y = MINE_TOP + i as f32 * ROW_H;
            let on = p.shown.as_deref() == Some(l.id.as_str());
            if on {
                s.fill(g, x - 4.0, y - 2.0, BOX.w - 40.0, ROW_H, crate::ui::rgb(0.47, 0.47, 0.70));
            } else if py >= y - 2.0 && py < y - 2.0 + ROW_H && px >= x && px < BOX.x + BOX.w - 24.0 {
                s.fill(g, x - 4.0, y - 2.0, BOX.w - 40.0, ROW_H, crate::ui::rgb(0.66, 0.66, 0.84));
            }
            s.put(g, f, x, y, &l.name, if on { yellow } else { white });
            s.put(g, f, x + 360.0, y, &trf("STR_MP_LEAGUE_CODE", &[("CODE", &l.id)]), if on { yellow } else { white });
        }
        // the shown league's table
        match &p.table {
            None if p.busy => s.put(g, f, x, TABLE_TOP, tr("STR_MP_LEAGUE_LOADING"), white),
            None => {}
            Some(t) => {
                let place = sg_core::data::properties().get(t.league.property).map(|p| p.name().to_string()).unwrap_or_default();
                let diff = tr(crate::render::DIFFICULTY_KEYS[t.league.difficulty.min(3) as usize]);
                s.put_fit(
                    g,
                    F_ARIAL10,
                    x,
                    TABLE_TOP - 34.0,
                    BOX.w - 48.0,
                    &trf(
                        "STR_MP_LEAGUE_ABOUT",
                        &[
                            ("LEAGUE", &t.league.name),
                            ("PLACE", &place),
                            ("DIFFICULTY", &diff),
                            ("MEASURE", &super::rival::measure_text(t.league.measure)),
                        ],
                    ),
                    navy,
                );
                let cols = [0.0, 190.0, 290.0, 400.0, 470.0, 560.0];
                for (c, k) in cols.iter().zip([
                    "STR_MP_LEAGUE_WHO",
                    "STR_MP_LEAGUE_DATE",
                    "STR_MP_MEASURE_CASH",
                    "STR_MP_MEASURE_FUN",
                    "STR_MP_MEASURE_MEMBERS",
                    "STR_MP_MEASURE_SKILL",
                ]) {
                    s.put(g, F_ARIAL10, x + c, TABLE_TOP - 16.0, tr(k), navy);
                }
                for (i, r) in t.ranked().into_iter().enumerate() {
                    let y = TABLE_TOP + i as f32 * ROW_H;
                    if p.picked == Some(r.seat) {
                        s.fill(g, x - 4.0, y - 2.0, BOX.w - 40.0, ROW_H, crate::ui::rgb(0.47, 0.47, 0.70));
                    }
                    let col = if r.mine { yellow } else { white };
                    let who = if r.mine { trf("STR_MP_LEAGUE_YOU", &[("NICK", &r.nick)]) } else { r.nick.clone() };
                    s.put_fit(g, f, x, y, 180.0, &format!("{}. {who}", i + 1), col);
                    match &r.facts {
                        None => s.put(g, f, x + cols[1], y, tr("STR_MP_LEAGUE_NOT_YET"), grey),
                        Some(fa) => {
                            let date = date_text(r.month, fa.year);
                            s.put(g, f, x + cols[1], y, &date, col);
                            s.put(g, f, x + cols[2], y, &crate::ui::money(fa.cash), col);
                            s.put(g, f, x + cols[3], y, &fa.fun.to_string(), col);
                            s.put(g, f, x + cols[4], y, &fa.members.to_string(), col);
                            s.put(g, f, x + cols[5], y, &format!("{:.2}", fa.skill as f64 / 100.0), col);
                        }
                    }
                }
                let mine_seat = t.my_seat();
                draw_button(g, &s, PLAY, tr("STR_MP_LEAGUE_PLAY"), mine_seat.is_some(), mine_seat.is_some() && PLAY.has(px, py));
                let can_visit = p.picked.is_some_and(|k| Some(k) != mine_seat);
                draw_button(g, &s, VISIT, tr("STR_MP_LEAGUE_VISIT"), can_visit, can_visit && VISIT.has(px, py));
            }
        }
        if !p.note.is_empty() {
            s.put_fit(g, F_ARIAL10, x, PLAY.y + 30.0, BOX.w - 48.0, &p.note, navy);
        }
        // a new league
        s.put(g, f, x, NAME.y + 2.0, tr("STR_MP_LEAGUE_NEW"), white);
        let field = |g: &mut Gfx, r: Rect, text: &str, hint: &str, on: bool| {
            s.fill(g, r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, c15(if on { 0x7ff0 } else { 0x2d6b }));
            s.fill(g, r.x, r.y, r.w, r.h, c15(0x7fff));
            let caret = if on && (miniquad::date::now() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
            if text.is_empty() && !on {
                s.put(g, f, r.x + 6.0, r.y + 2.0, hint, grey);
            } else {
                s.put(g, f, r.x + 6.0, r.y + 2.0, &format!("{text}{caret}"), navy);
            }
        };
        field(g, NAME, &p.name, tr("STR_MP_CLUBS_NEW_NAME"), p.editing == Some(Field::Name));
        let prop = sg_core::data::properties().get(p.property).map(|p| p.name().to_string()).unwrap_or_default();
        draw_button(g, &s, PROPERTY, &trf("STR_MP_LEAGUE_PROPERTY", &[("PLACE", &prop)]), true, PROPERTY.has(px, py));
        draw_button(g, &s, DIFFICULTY, tr(crate::render::DIFFICULTY_KEYS[p.difficulty as usize]), true, DIFFICULTY.has(px, py));
        draw_button(
            g,
            &s,
            MEASURE,
            &trf("STR_MP_LEAGUE_SCORES", &[("MEASURE", &super::lobby::measure_name(p.measure))]),
            true,
            MEASURE.has(px, py),
        );
        draw_button(g, &s, MAKE, tr("STR_MP_LEAGUE_MAKE"), !p.busy, !p.busy && MAKE.has(px, py));
        // joining by code
        s.put(g, f, x, CODE.y + 2.0, tr("STR_MP_LEAGUE_JOIN_CODE"), white);
        field(g, CODE, &p.code, "ABC234", p.editing == Some(Field::Code));
        draw_button(g, &s, JOIN, tr("STR_MP_LEAGUE_JOIN"), !p.busy, !p.busy && JOIN.has(px, py));
        draw_button(g, &s, BACK, tr("STR_MP_BACK"), true, BACK.has(px, py));
        g.flush();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_league_table_reads_and_ranks() {
        let body = serde_json::json!({ "ok": true,
        "league": { "code": "ABC234", "name": "Valley", "property": 3, "difficulty": 1, "measure": "cash", "seed": 1234, "endYear": 2002 },
        "players": [
            { "seat": 0, "nick": "Mike", "facts": null, "at": "", "save": false, "mine": true },
            { "seat": 1, "nick": "Ann", "facts": { "year": 2001, "month": 5, "cash": 9000, "holes": 3, "fun": 1, "skill": 2, "members": 3, "rank": 0 }, "at": "", "save": true, "mine": false },
            { "seat": 2, "nick": "Bo", "facts": { "year": 2001, "month": 2, "cash": 12000, "holes": 3, "fun": 1, "skill": 2, "members": 3, "rank": 0 }, "at": "", "save": false, "mine": false },
        ]});
        let t = read_table(body.to_string().as_bytes()).unwrap();
        assert_eq!(t.league.measure, Measure::Cash);
        assert_eq!(t.my_seat(), Some(0));
        assert_eq!(t.ranked().iter().map(|r| r.nick.as_str()).collect::<Vec<_>>(), ["Bo", "Ann", "Mike"]);
        assert_eq!(t.rows[1].month, 5);
        assert!(read_table(br#"{"ok":false,"error":"no league has that code"}"#).is_err());
    }

    #[test]
    fn each_seat_has_its_own_land_and_id() {
        let seeds: std::collections::HashSet<u32> = (0..SEATS).map(|k| seat_seed(0x00ab_cdef, k)).collect();
        assert_eq!(seeds.len(), SEATS);
        assert!(seeds.iter().all(|s| *s <= 0x00ff_ffff));
        assert_eq!(league_of(Some(&game_id("ABC234", 2))), Some(("ABC234".into(), 2)));
        assert_eq!(league_of(Some("*rival/2026-W48")), None);
        assert!(is_code("ABC234") && !is_code("ABC0EF") && !is_code("abc234"));
    }
}
