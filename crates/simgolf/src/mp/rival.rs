//! Rival courses in the game: this week's challenge as a row of Start New Game (once Multiplayer is on), the same land
//! and start for everyone, and its result sent to the weekly board when 2001 ends.
//!
//! The game is an ordinary scenario game whose scenario id is `*rival/<week>` (never a pack id: packs can't start with
//! `*`), so the save already remembers which week it is and nothing new goes into the save format.
//!
//! The hooks outside this folder, each one line behind the `multiplayer` feature: scenario_ui.rs (the row, picking it,
//! and the page's draw, click and key), app.rs (the year end) and main.rs (polling each frame, typed characters).

use super::challenge::{self, Challenge, Kind, Measure};
use super::{result, settings};
use crate::app::App;
use crate::gfx::Gfx;
use crate::scenario_ui::ScenarioEntry;
use sg_core::land::{ExeRng, Noise};
use sg_core::lang::{tr, trf};
use sg_core::scenario::{GoalDef, Scenario, ScenarioDef};
use std::cell::RefCell;

/// The scenario id of a rival-courses game is this and the week id.
pub const ID_PREFIX: &str = "*rival/";

thread_local! {
    /// A result being sent.
    static SENDING: RefCell<Option<crate::net::Request>> = const { RefCell::new(None) };
}

/// The board's address: OPENSGOLF_BOARD_URL when set (a local Worker, for tests), else the real one.
pub fn board_url() -> String {
    std::env::var("OPENSGOLF_BOARD_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| result::BOARD_URL.to_string())
}

fn now_s() -> i64 {
    miniquad::date::now() as i64
}

/// The challenge a game is playing, from its scenario id.
pub fn challenge_of(scenario: Option<&str>) -> Option<Challenge> {
    let week = scenario?.strip_prefix(ID_PREFIX)?;
    challenge::is_week_id(week).then(|| challenge::challenge(week))
}

pub fn measure_text(m: Measure) -> &'static str {
    match m {
        Measure::Cash => tr("STR_MP_GOAL_CASH"),
        Measure::Fun => tr("STR_MP_GOAL_FUN"),
        Measure::Members => tr("STR_MP_GOAL_MEMBERS"),
        Measure::Skill => tr("STR_MP_GOAL_SKILL"),
    }
}

/// What the week's kind adds to its description (empty for the usual start).
pub fn kind_text(k: Kind) -> String {
    super::texts();
    match k {
        Kind::Standard => String::new(),
        Kind::Budget => tr("STR_MP_KIND_BUDGET").into(),
        Kind::Nine => trf("STR_MP_KIND_NINE", &[("N", &challenge::NINE_HOLES)]),
    }
}

/// The challenge as a scenario. Its one goal only says what the week is about: it has no conditions or deadline, so it
/// never ends the game (the board decides who did best).
pub fn scenario(c: &Challenge) -> Scenario {
    super::texts();
    let goal = measure_text(c.measure);
    Scenario {
        scenario: ScenarioDef {
            name: Some(trf("STR_MP_WEEKLY_NAME", &[("WEEK", &c.id)])),
            description: format!("{} {}", trf("STR_MP_WEEKLY_DESCRIPTION", &[("GOAL", &goal), ("YEAR", &c.end_year)]), kind_text(c.kind))
                .trim_end()
                .to_string(),
            theme: Some(c.theme),
            property: Some(sg_core::data::property_id(challenge::property(c)).to_string()),
            difficulty: Some(c.difficulty),
            intro: format!(
                "{} {}",
                trf("STR_MP_WEEKLY_INTRO", &[("WEEK", &c.id), ("GOAL", &goal), ("YEAR", &c.end_year)]),
                kind_text(c.kind)
            )
            .trim_end()
            .to_string(),
            ..Default::default()
        },
        goals: vec![GoalDef {
            id: Some("weekly".into()),
            text: trf("STR_MP_WEEKLY_GOAL", &[("GOAL", &goal), ("YEAR", &c.end_year)]),
            ..Default::default()
        }],
    }
}

/// The rows Start New Game adds: this week's challenge (its page is where Multiplayer is turned on, lobby.rs) and the
/// shared clubs (clubs_ui.rs).
pub fn entries() -> Vec<ScenarioEntry> {
    let c = challenge::this_week(now_s());
    let sc = scenario(&c);
    vec![
        ScenarioEntry {
            id: format!("{ID_PREFIX}{}", c.id),
            name: sc.scenario.name.clone().unwrap_or_default(),
            description: sc.scenario.description.clone(),
            def: Ok(sc),
        },
        super::clubs_ui::entry(),
        super::league::entry(),
    ]
}

/// The week's people, stories, celebrities and pros: the free set's Standard theme for every player.
pub fn tables() -> super::club::Tables {
    let files = sg_core::freeset::theme_files("Standard");
    let mut stories: Vec<(String, String)> = files
        .iter()
        .filter(|(n, _)| n.to_lowercase().ends_with(".txt") && !n.to_lowercase().contains("shadow"))
        .map(|(n, b)| (n.clone(), sg_core::formats::latin1(b)))
        .collect();
    stories.sort_by_key(|(n, _)| n.to_lowercase());
    let mut stories = sg_core::stories::Stories::build(&stories, 0);
    stories.fill_shape();
    let text = |name: &str| files.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, b)| sg_core::formats::latin1(b));
    super::club::Tables {
        stories,
        celebrities: text("celebrities.dta").map(|t| sg_core::vips::parse_celebrities(&t)).unwrap_or_default(),
        pros: text("progolfers.dta").map(|t| sg_core::vips::parse_pros(&t)).unwrap_or_default(),
        roster: Some(sg_core::roster::from_theme_files(&files)),
    }
}

impl App {
    /// Start New Game's row picked: its page (lobby.rs) on Start New Game, else (the `--scenario` test hook) the game at
    /// once. False when `id` isn't a week's challenge.
    pub fn pick_rival(&mut self, g: &mut Gfx, id: &str) -> bool {
        if id == super::clubs_ui::ROW_ID {
            return self.screen == crate::app::Screen::Scenarios && self.ui_ok && self.open_clubs_page();
        }
        if id == super::league::ROW_ID {
            return self.screen == crate::app::Screen::Scenarios && self.ui_ok && self.open_leagues_page();
        }
        if self.screen == crate::app::Screen::Scenarios && self.ui_ok {
            self.open_rival_lobby(id)
        } else {
            self.start_rival(g, id)
        }
    }

    /// Starts a week's challenge (`id` is the row's scenario id); false when it isn't one.
    pub fn start_rival(&mut self, g: &mut Gfx, id: &str) -> bool {
        let Some(c) = challenge_of(Some(id)) else { return false };
        self.scen.chosen = None;
        self.course_size = sg_core::mapsize::ORIGINAL;
        // the week's seed in place of the clock: the noise, the offer, the land and the club's first draws
        self.exe_rng = ExeRng { state: c.seed };
        self.noise = Noise::new(&mut self.exe_rng);
        self.difficulty = c.difficulty.level();
        self.course_id = Some(self.exe_rng.below(0xffff));
        self.econ.sandbox = false;
        self.deal_offer(false);
        let sc = scenario(&c);
        let prop = sc.property_index();
        self.cheap_slot(prop);
        self.scen.start_money = None;
        println!("rival: weekly challenge {} ({:?}, {:?}, {}, seed {})", c.id, c.theme, c.difficulty, c.measure.id(), c.seed);
        self.scen.starting = Some((id.to_string(), sc));
        // a budget week: start_game halves the money it starts with
        self.mp.halve_money = c.kind == Kind::Budget;
        self.start_game(g, prop, false);
        self.mp.halve_money = false;
        // everyone plays the week with the same people and stories, whatever their files and theme pack (each seeing their
        // own words where they have them: club.rs take_shared_tables)
        let t = tables();
        self.take_shared_tables(t);
        true
    }

    /// Whether this week's rule keeps another hole from opening (a nine-hole week with its nine open), saying so.
    pub fn rival_refuses_hole(&mut self) -> bool {
        let Some(c) = challenge_of(self.scen.goals.scenario.as_deref()) else { return false };
        if c.kind != Kind::Nine || (crate::scenario_ui::facts(self).holes as usize) < challenge::NINE_HOLES {
            return false;
        }
        self.show_toast(&trf("STR_MP_WEEKLY_NINE_FULL", &[("N", &challenge::NINE_HOLES)]));
        true
    }

    /// At a year end: when the challenge's year is over, its result is sent.
    pub fn rival_year_end(&mut self) {
        let Some(c) = challenge_of(self.scen.goals.scenario.as_deref()) else { return };
        let f = crate::scenario_ui::facts(self);
        if f.year != c.end_year + 1 {
            return;
        }
        let s = settings::with_player();
        let (nick, group) = (settings::nick(&s), settings::group(&s));
        let platform = crate::playcount::platform_here();
        let who = result::Sender { player: &s.player, nick: &nick, group: &group, platform: &platform, version: env!("CARGO_PKG_VERSION") };
        let score = c.measure.score(&f);
        match result::result_url(&board_url(), &c, &who, self.club.cheated, &f) {
            Ok(url) => {
                println!("rival: sending {} = {score}", c.measure.id());
                SENDING.with(|r| *r.borrow_mut() = Some(crate::net::get(&url, 4096)));
                self.show_toast(&trf("STR_MP_WEEKLY_OVER_SENDING", &[("SCORE", &score)]));
            }
            Err(result::NotSent::Cheated) => {
                self.show_toast(&trf("STR_MP_WEEKLY_OVER_CHEATS", &[("SCORE", &score)]));
            }
            Err(result::NotSent::YearNotOver) => {}
        }
    }

    /// Each frame: the board's answer to a result sent.
    pub fn rival_poll(&mut self) {
        if let Some(note) = super::lobby::poll(now_s()) {
            println!("rival: {note}");
            self.show_toast(&note);
            self.toast_until = self.clock + 8.0;
        }
        let Some(reply) = SENDING.with(|r| {
            let mut r = r.borrow_mut();
            let reply = r.as_mut()?.poll()?;
            *r = None;
            Some(reply)
        }) else {
            return;
        };
        let msg = match reply {
            Ok(body) => answer_text(&body),
            Err(e) => trf("STR_MP_BOARD_UNREACHABLE", &[("ERROR", &e)]),
        };
        println!("rival: {msg}");
        self.show_toast(&msg);
    }
}

/// What the player is told of the Worker's answer.
fn answer_text(body: &[u8]) -> String {
    super::texts();
    let v: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    match (v["ok"].as_bool(), v["better"].as_bool(), v["error"].as_str()) {
        (Some(true), Some(true), _) => tr("STR_MP_RESULT_ON_BOARD").into(),
        (Some(true), _, _) => tr("STR_MP_RESULT_NOT_BETTER").into(),
        (_, _, Some("update the game")) => tr("STR_MP_RESULT_UPDATE").into(),
        (_, _, Some(e)) => trf("STR_MP_RESULT_REFUSED", &[("ERROR", &e)]),
        _ => tr("STR_MP_RESULT_UNREADABLE").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_rival_ids_are_challenges() {
        assert_eq!(challenge_of(Some("*rival/2026-W47")).map(|c| c.id), Some("2026-W47".to_string()));
        for id in [None, Some("example-desert"), Some("*rival/nope"), Some("*challenges")] {
            assert!(challenge_of(id).is_none(), "{id:?}");
        }
    }

    #[test]
    fn the_scenario_names_the_week_and_sets_the_start() {
        let c = challenge::challenge("2026-W47");
        let sc = scenario(&c);
        assert_eq!(sc.scenario.theme, Some(c.theme));
        assert_eq!(sc.scenario.difficulty, Some(c.difficulty));
        assert!(sc.scenario.name.as_deref().unwrap().contains("2026-W47"));
        // the goal never decides the game: no conditions, no deadline
        let mut goals = sc.start_goals("*rival/2026-W47", "x");
        let f = sg_core::scenario::Facts { year: 2003, cash: 1 << 40, ..Default::default() };
        assert!(goals.check(&f, Some(2002)).is_empty() && goals.outcome.is_none());
    }

    #[test]
    fn the_weeks_tables_are_the_free_sets() {
        let t = super::tables();
        assert_eq!(t.celebrities.len(), 22);
        assert_eq!(t.pros.len(), 20);
        assert!(t.stories.slots.iter().filter(|s| !s.is_empty()).count() >= 20);
        // (the tutorial id is the OpeningDay file's list index, as the original has it: stories.rs)
        assert!(t.stories.name(2 * t.stories.tutorial).contains("OpeningDay"));
        assert_eq!(t.stories.shape.len(), t.stories.slots.len());
        assert_eq!(t.roster.as_ref().map(|r| r.len()), Some(sg_core::roster::PEOPLE));
    }

    #[test]
    fn a_weeks_game_keeps_its_tables_when_loaded() {
        // (a test game reads no theme files: without the week's own tables a load would leave none)
        let mut a = crate::mp::lockstep::tests::shared_game(0);
        a.mp.session = None;
        a.scen.goals.scenario = Some(format!("{ID_PREFIX}2026-W48"));
        a.take_shared_tables(super::tables());
        let b = a.save_bytes(6).unwrap();
        let s = crate::app::read_save_bytes(&b, "week").unwrap();
        a.load_save_into(None, s, false).unwrap();
        assert_eq!(a.club.celebrities.len(), 22);
        assert_eq!(a.club.stories.shape, super::tables().stories.shape);
        // (the people are the week's; their names this player's own)
        assert_eq!(a.club.roster[76].traits, super::tables().roster.unwrap()[76].traits);
    }

    #[test]
    fn answers() {
        assert_eq!(answer_text(br#"{"ok":true,"better":true}"#), "Your result is on the weekly board.");
        assert!(answer_text(br#"{"ok":true,"better":false}"#).contains("still"));
        assert!(answer_text(br#"{"ok":false,"error":"update the game"}"#).starts_with("Update"));
        assert!(answer_text(b"<html>").contains("couldn't"));
    }
}
