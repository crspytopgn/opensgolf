//! The welcome boxes (the port's own, none of it in the original; docs/PLAYING.md "How would you like to play?"):
//!
//! * "How would you like to play?", once, at the first title menu of a new player (no settings.json yet, `decide`). Its
//!   four choices are the four presets of the OpenSGolf extras (extras_ui.rs `Preset::ALL`: the original game, Original +
//!   helpers, OpenSGolf, Custom), put in use as choosing them in Preferences > Game options does; Custom, which has no mix
//!   of the player's own yet, opens Game options to tick one. Esc, a right-click or a click outside the box keeps the
//!   settings as they are (the original game). Nothing is marked as the current choice.
//! * "What's new in OpenSGolf 0.6", once after an update (a player with a settings.json from before, compared with its
//!   "last_version_seen", which holds the minor version: 0.6.1 does not show 0.6's box again to a player who saw it). A
//!   new player never gets it (the first box records the version as seen). A highlight that is an option has a Try it
//!   pill that puts it in use (On once it is), or opens the HD maker's question; See everything that's new opens the
//!   changelog in the browser.
//!
//! Neither shows in scripted runs (--png, --time, --screen: the stills and the tests stay as they were) nor over another
//! box; both wait for the title menu to settle as the support box does (support.rs).
//!
//! Test hooks: SG_WELCOME=1 and SG_WHATSNEW=1 show the box at the first title menu whatever settings.json says (for
//! stills; nothing is written then).
use crate::app::{App, Screen};
use crate::extras_ui::Preset;
use crate::popup_ui::PopupKind;
use sg_core::extras::Extras;
use sg_core::lang::{tr, trf};
use std::cell::RefCell;

/// The changelog (box B's See everything that's new, Preferences > More > What's new...).
pub const CHANGELOG_URL: &str = "https://github.com/crspytopgn/opensgolf/blob/main/CHANGELOG.md";
/// settings.json's keys: "welcome_shown" is false from a new player's first start until the first box has shown (a
/// player who quits before it is still new next time), true after; "last_version_seen" the minor version last shown.
pub const WELCOME_KEY: &str = "welcome_shown";
pub const VERSION_KEY: &str = "last_version_seen";
/// How long the title menu shows before a box comes up there (as the support box, support.rs).
const TITLE_SETTLE: f64 = 1.5;
/// The width the boxes' notes wrap at: the original's font, the free set's (whose face is wider).
const NOTE_W: [f32; 2] = [440.0, 540.0];

/// What a highlight's Try it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Try {
    /// Opens Preferences > Graphics' Make HD graphics... question (hd_make.rs; only with the original game's files and
    /// not in a browser: elsewhere the line shows without a pill).
    MakeHd,
    /// The Mouse aim and swing extra on (extras.rs).
    MouseSwing,
    /// The Right click puts the tool away extra on (extras.rs).
    RightDrop,
    /// Sandbox: no new dandelions on for the next games (sg_core::extras).
    NoWeeds,
}

/// One of a version's highlights: its line, the note under it, and what its Try it does (none: no option to try).
#[derive(Clone, Copy, Debug)]
pub struct Highlight {
    pub line: &'static str,
    pub note: &'static str,
    pub action: Option<Try>,
}

const fn hl(line: &'static str, note: &'static str, action: Option<Try>) -> Highlight {
    Highlight { line, note, action }
}

/// Each version's highlights, oldest first: a release adds its entry here (and its lines to lang/en.txt). A version is
/// its minor version ("0.6"), the title's.
pub const HIGHLIGHTS: &[(&str, &[Highlight])] = &[(
    "0.6",
    &[
        hl("STR_WHATSNEW_SMOOTH", "STR_WHATSNEW_SMOOTH_NOTE", None),
        hl("STR_WHATSNEW_HD", "STR_WHATSNEW_HD_NOTE", Some(Try::MakeHd)),
        hl("STR_WHATSNEW_SWING", "STR_WHATSNEW_SWING_NOTE", Some(Try::MouseSwing)),
        hl("STR_WHATSNEW_RIGHT", "STR_WHATSNEW_RIGHT_NOTE", Some(Try::RightDrop)),
        hl("STR_WHATSNEW_WEEDS", "STR_WHATSNEW_WEEDS_NOTE", Some(Try::NoWeeds)),
        hl("STR_WHATSNEW_MORE", "STR_WHATSNEW_MORE_NOTE", None),
    ],
)];

/// A version as numbers ("0.6" and "0.6.0" alike); none for a version that does not read.
pub fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.trim().split('.').map(|p| p.parse::<u32>().ok());
    let a = it.next()??;
    let b = it.next().unwrap_or(Some(0))?;
    let c = it.next().unwrap_or(Some(0))?;
    Some((a, b, c))
}

/// The seen marker of version `v`: its minor version ("0.6.1" is "0.6"), so a fix release does not show its minor
/// version's box again.
pub fn marker(v: &str) -> String {
    match parse(v) {
        Some((a, b, _)) => format!("{a}.{b}"),
        None => v.to_string(),
    }
}

/// The newest version's highlights after `seen` (none: a player from before the key) up to `now` (its minor version).
pub fn news(seen: Option<&str>, now: &str) -> Option<(&'static str, &'static [Highlight])> {
    let now = parse(&marker(now))?;
    let seen = seen.and_then(|s| parse(&marker(s))).unwrap_or((0, 0, 0));
    HIGHLIGHTS.iter().rev().find(|(v, _)| parse(v).is_some_and(|v| v > seen && v <= now)).copied()
}

/// Which box is due for the settings.json `v` read at the very start (before anything writes it) at version `now`: a new
/// player (no settings, or a first start that quit before the first box) gets "How would you like to play?" and never
/// What's new; a player updating (settings from before) gets only What's new, when there is something new.
pub fn decide(v: &serde_json::Value, now: &str) -> (bool, Option<(&'static str, &'static [Highlight])>) {
    let empty = v.as_object().is_none_or(|o| o.is_empty());
    if empty || v[WELCOME_KEY] == serde_json::Value::Bool(false) {
        return (true, None);
    }
    (false, news(v[VERSION_KEY].as_str(), now))
}

thread_local! {
    /// settings.json as it was when the game started (read before anything else writes it).
    static AT_START: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
}

fn settings_json() -> serde_json::Value {
    sg_core::fsutil::read_file(crate::app::save_dir().join(crate::hd::SETTINGS_FILE))
        .and_then(|d| serde_json::from_slice::<serde_json::Value>(&d).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// At the very start, before anything writes settings.json: what it holds (a new player has none).
pub fn note_first_start() {
    let v = settings_json();
    AT_START.with(|c| *c.borrow_mut() = Some(v));
}

/// The boxes' state (`App::welcome`).
#[derive(Default)]
pub struct State {
    /// "How would you like to play?" is due.
    pub welcome_due: bool,
    /// "What's new" is due.
    pub news_due: bool,
    /// The version and highlights it shows.
    pub news: Option<(&'static str, &'static [Highlight])>,
    /// SG_WELCOME / SG_WHATSNEW: the box shown once, nothing written.
    forced: bool,
    /// The screen of the last frame and when the title menu came up.
    last: Option<Screen>,
    title_since: f64,
}

/// At start up (`scripted`: a still or test run, where nothing is due unless forced).
pub fn init(scripted: bool) -> State {
    let now = env!("CARGO_PKG_VERSION");
    let flag = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty() && v != "0");
    let (fw, fn_) = (flag("SG_WELCOME"), flag("SG_WHATSNEW"));
    if fw || fn_ {
        return State { welcome_due: fw, news_due: fn_, news: news(None, now), forced: true, ..Default::default() };
    }
    if scripted {
        return State::default();
    }
    let v = AT_START.with(|c| c.borrow().clone()).unwrap_or_else(settings_json);
    let (welcome_due, news) = decide(&v, now);
    if welcome_due {
        // still a new player if the game is closed before the box shows
        save(WELCOME_KEY, serde_json::Value::Bool(false));
    }
    State { welcome_due, news_due: news.is_some(), news, ..Default::default() }
}

/// Writes `key` into settings.json, keeping its other keys.
fn save(key: &str, val: serde_json::Value) {
    let mut v = settings_json();
    v[key] = val;
    if let Ok(t) = serde_json::to_string_pretty(&v) {
        sg_core::fsutil::write_file(crate::app::save_dir().join(crate::hd::SETTINGS_FILE), t.as_bytes());
    }
}

/// The first box's choices, in order: the presets (`Preset::ALL`), each with its line and the note under it.
pub const CHOICES: [(Preset, &str, &str); 4] = [
    (Preset::Original, "STR_WELCOME_ORIGINAL", "STR_WELCOME_ORIGINAL_NOTE"),
    (Preset::Helpers, "STR_WELCOME_HELPERS", "STR_WELCOME_HELPERS_NOTE"),
    (Preset::OpenSGolf, "STR_WELCOME_OPENSGOLF", "STR_WELCOME_OPENSGOLF_NOTE"),
    (Preset::Custom, "STR_WELCOME_CUSTOM", "STR_WELCOME_CUSTOM_NOTE"),
];

/// What a choice of the first box does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    /// These extras (gameplay, interface) put in use as Game options' OK does (main.rs apply_extras).
    Put(Extras, crate::extras::Settings),
    /// Custom without a mix of the player's own: Game options opens to tick one.
    OwnMix,
}

/// The first box's choice `k` from the extras `e` and `ui` in use (`custom`: the player's own mix kept, if any), as the
/// preset line of Game options puts it (`Preset::apply`); none for Esc or a click outside (nothing changes).
pub fn pick(
    k: Option<usize>,
    mut e: Extras,
    mut ui: crate::extras::Settings,
    custom: Option<(Extras, crate::extras::Settings)>,
) -> Option<Pick> {
    let (p, _, _) = *CHOICES.get(k?)?;
    Some(if p.apply(&mut e, &mut ui, custom) { Pick::Put(e, ui) } else { Pick::OwnMix })
}

/// `t` wrapped as the boxes' notes (`free`: on the free set).
fn notes(t: &str, free: bool) -> impl Iterator<Item = String> {
    crate::extras::wrap(crate::popup_ui::NOTE_FONT, t, NOTE_W[free as usize]).into_iter().map(|l| format!("\t{l}"))
}

/// The first box's lines: the title, a gap (clear of the free set's header band), each choice with its note, and the
/// line that says where to change it.
pub fn welcome_lines(free: bool) -> Vec<String> {
    let mut v = vec![tr("STR_WELCOME_TITLE").to_string(), "\t".to_string()];
    for (_, line, note) in CHOICES {
        v.push(tr(line).to_string());
        v.extend(notes(tr(note), free));
    }
    v.push("\t".to_string());
    v.extend(notes(tr("STR_WELCOME_CHANGE"), free));
    v
}

/// The second box's lines: the title, a gap, each highlight with its note, a gap, See everything that's new and OK.
pub fn whatsnew_lines(version: &str, h: &[Highlight], free: bool) -> Vec<String> {
    let mut v = vec![trf("STR_WHATSNEW_TITLE", &[("VERSION", &version)]), "\t".to_string()];
    for x in h {
        v.push(tr(x.line).to_string());
        v.extend(notes(tr(x.note), free));
    }
    v.push("\t".to_string());
    v.extend([tr("STR_WHATSNEW_ALL").to_string(), tr("STR_WHATSNEW_OK").to_string()]);
    v
}

/// The second box's options that only tell (no Try it here, `can` says which can be tried): drawn with a bullet, never
/// lit.
pub fn whatsnew_plain(h: &[Highlight], can: impl Fn(Try) -> bool) -> u32 {
    h.iter().enumerate().filter(|(_, x)| !x.action.is_some_and(&can)).fold(0, |m, (k, _)| m | 1 << k)
}

/// The Try it pill's width (its widest label).
pub fn pill_w() -> f32 {
    let f = crate::extras_ui::PILL_FONT;
    f.width(tr("STR_WHATSNEW_TRY")).max(f.width(tr("STR_WHATSNEW_ON"))) + 16.0
}

impl App {
    /// Once a frame: the box due at the title menu once it has settled, never over another box.
    pub fn welcome_frame(&mut self) {
        let now = self.screen;
        let w = &mut self.welcome;
        if now == Screen::Menu && w.last != Some(Screen::Menu) {
            w.title_since = self.clock;
        }
        w.last = Some(now);
        if !(w.welcome_due || w.news_due) || now != Screen::Menu || self.clock - w.title_since < TITLE_SETTLE && !w.forced {
            return;
        }
        if self.other_box_up() {
            return;
        }
        let w = &mut self.welcome;
        let write = !w.forced;
        let seen = || marker(env!("CARGO_PKG_VERSION"));
        if std::mem::take(&mut w.welcome_due) {
            if write {
                // a new player: this version's news are no news to them
                save(WELCOME_KEY, serde_json::Value::Bool(true));
                save(VERSION_KEY, seen().into());
                w.news_due = false;
            }
            self.hover = -1;
            self.open_popup(PopupKind::Welcome);
            return;
        }
        if std::mem::take(&mut w.news_due) && w.news.is_some() {
            if write {
                save(VERSION_KEY, seen().into());
            }
            self.hover = -1;
            self.open_popup(PopupKind::WhatsNew);
        }
    }

    /// The changelog in the browser (a new tab on the web); without one its address goes on the clipboard.
    pub fn open_changelog(&mut self) {
        if !crate::bugreport::open_link(CHANGELOG_URL) {
            #[cfg(not(target_arch = "wasm32"))]
            miniquad::window::clipboard_set(CHANGELOG_URL);
            self.show_toast(&trf("STR_SUPPORT_NO_BROWSER", &[("URL", &CHANGELOG_URL)]));
        }
    }

    /// The highlights the What's new box shows (none once it is answered).
    pub fn whatsnew_shown(&self) -> &'static [Highlight] {
        self.welcome.news.map_or(&[], |n| n.1)
    }

    /// Whether a highlight's Try it can be offered here (the HD maker only with the original game's files, not in a
    /// browser).
    pub fn can_try(&self, t: Try) -> bool {
        t != Try::MakeHd || self.hd_make_offer().is_some()
    }

    /// The What's new box's options that only tell (`whatsnew_plain`).
    pub fn whatsnew_tell(&self) -> u32 {
        whatsnew_plain(self.whatsnew_shown(), |t| self.can_try(t))
    }

    /// Whether a highlight's option is already in use (its pill says On).
    pub fn tried(&self, t: Try) -> bool {
        match t {
            Try::MakeHd => false,
            Try::MouseSwing => crate::extras::mouse_swing(),
            Try::RightDrop => crate::extras::settings().right_drop,
            Try::NoWeeds => self.extras_next.no_weeds,
        }
    }

    /// The first box's answer (`k` its option, none for Esc, a right-click or a click outside it: nothing changes), from
    /// the extras in use now.
    pub fn welcome_pick(&self, k: Option<usize>) -> Option<Pick> {
        pick(k, self.extras_next, crate::extras::settings(), crate::extras_ui::custom_mix())
    }

    /// The second box's option `k`: a highlight's Try it (true: the box stays up), See everything that's new (stays up),
    /// OK; none (Esc, outside) closes it.
    pub fn whatsnew_pick(&mut self, k: Option<usize>) -> bool {
        let h = self.whatsnew_shown();
        let Some(k) = k else { return false };
        if k == h.len() {
            self.open_changelog();
            return true;
        }
        let Some(x) = h.get(k) else { return false };
        match x.action {
            Some(t) if !self.can_try(t) || self.tried(t) => true,
            None => true,
            Some(Try::MakeHd) => {
                // the box makes way for the HD maker's question (Preferences > Graphics' Make HD graphics...)
                self.close_popup();
                self.prefs_stack.clear();
                self.hd_make_open();
                false
            }
            Some(Try::MouseSwing) | Some(Try::RightDrop) => {
                let mut ui = crate::extras::settings();
                if x.action == Some(Try::MouseSwing) {
                    ui.mouse_swing = true;
                } else {
                    ui.right_drop = true;
                }
                crate::extras::set(ui);
                crate::extras::save(&ui);
                true
            }
            Some(Try::NoWeeds) => {
                let mut e = self.extras_next;
                e.no_weeds = true;
                self.extras_next = e;
                crate::extras_ui::save_next(&e);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extras::Settings;
    use serde_json::json;

    #[test]
    fn versions_read_and_compare() {
        assert_eq!(parse("0.6"), Some((0, 6, 0)));
        assert_eq!(parse("0.6.0"), parse("0.6"));
        assert_eq!(parse("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse("x"), None);
        assert!(parse("0.5.12") < parse("0.6"));
    }

    #[test]
    fn the_seen_marker_is_the_minor_version() {
        assert_eq!(marker("0.6.1"), "0.6");
        assert_eq!(marker("0.6.0"), "0.6");
        assert_eq!(marker("1.0"), "1.0");
        // 0.6.0 players (no marker) see 0.6's box once in 0.6.1, and not again in 0.6.2
        assert_eq!(news(None, "0.6.1").map(|n| n.0), Some("0.6"));
        assert!(news(Some(&marker("0.6.1")), "0.6.1").is_none());
        assert!(news(Some(&marker("0.6.1")), "0.6.2").is_none());
        assert!(news(Some("0.6.0"), "0.6.1").is_none(), "an older marker of the same minor version");
    }

    #[test]
    fn whats_new_shows_once_after_an_update_to_a_version_with_highlights() {
        assert_eq!(news(None, "0.6.0").map(|n| n.0), Some("0.6"), "a player from before the key");
        assert_eq!(news(Some("0.5.12"), "0.6.1").map(|n| n.0), Some("0.6"));
        assert!(news(Some("0.6"), "0.6.1").is_none(), "seen");
        assert!(news(Some("0.5.12"), "0.5.13").is_none(), "an older game");
    }

    #[test]
    fn new_players_get_the_first_box_and_updating_players_only_whats_new() {
        let now = "0.6.1";
        // a new player: no settings.json (or an empty one), or a first start closed before the box showed
        for v in [json!({}), json!(null), json!({ WELCOME_KEY: false, "ui_extras": {} })] {
            let (welcome, news) = decide(&v, now);
            assert!(welcome && news.is_none(), "{v}");
        }
        // a player of 0.5.x or 0.6.0 (settings from before, neither key): only What's new
        let old = json!({ "ui_extras": { "people": true }, "hd": { "enabled": false } });
        let (welcome, news) = decide(&old, now);
        assert!(!welcome);
        assert_eq!(news.map(|n| n.0), Some("0.6"));
        // a player who has had the first box in 0.6.1 (it records the version as seen): nothing
        let (welcome, news) = decide(&json!({ WELCOME_KEY: true, VERSION_KEY: "0.6" }), now);
        assert!(!welcome && news.is_none());
        // a player who saw What's new: nothing
        let (welcome, news) = decide(&json!({ "ui_extras": {}, VERSION_KEY: "0.6" }), now);
        assert!(!welcome && news.is_none());
    }

    #[test]
    fn the_four_choices_put_their_preset_in_use() {
        assert_eq!(CHOICES.map(|c| c.0), Preset::ALL);
        // kept ticks (the pauses, the update check, the controls presets') stay as they are
        let ui = Settings { update_check: true, wide_ui: true, mouse_swing: true, ..Settings::ALL_OFF };
        let e = Extras::default();
        for (k, (p, _, _)) in CHOICES.iter().enumerate().take(3) {
            let Some(Pick::Put(pe, pu)) = pick(Some(k), e, ui, None) else { panic!("{p:?}") };
            assert_eq!(Preset::of(&pe, &pu), *p);
            let (mut we, mut wu) = (e, ui);
            assert!(p.apply(&mut we, &mut wu, None));
            assert_eq!((pe, pu), (we, wu), "as Game options' preset line puts it");
            assert!(pu.update_check && pu.wide_ui && pu.mouse_swing);
        }
        // Custom: Game options opens to tick a mix (a new player has none), or the mix kept comes back
        assert_eq!(pick(Some(3), e, ui, None), Some(Pick::OwnMix));
        let mix = (Extras { no_weeds: true, ..Extras::default() }, Settings { people: true, ..Settings::ALL_OFF });
        assert_eq!(pick(Some(3), e, ui, Some(mix)), Some(Pick::Put(mix.0, mix.1.with_kept(&ui))));
        // Esc, a right-click or a click outside: nothing changes
        assert_eq!(pick(None, e, ui, None), None);
        assert_eq!(pick(Some(9), e, ui, None), None);
    }

    #[test]
    fn the_welcome_box_has_four_choices_each_with_a_note() {
        for free in [false, true] {
            let l = welcome_lines(free);
            let opts: Vec<usize> = l.iter().enumerate().filter(|(_, s)| s.starts_with(' ')).map(|(i, _)| i).collect();
            assert_eq!(opts.len(), 4);
            for i in opts {
                assert!(l[i + 1].starts_with('\t') && l[i + 1].len() > 1, "{}", l[i]);
            }
            assert_eq!(l.last().unwrap(), "\tYou can change this any time in Preferences > Game options.");
        }
    }

    #[test]
    fn whats_new_lists_each_highlight_then_see_everything_and_ok() {
        let (v, h) = news(None, "0.6.1").unwrap();
        let l = whatsnew_lines(v, h, false);
        assert_eq!(l[0], "What's new in OpenSGolf 0.6");
        let opts: Vec<&String> = l.iter().filter(|s| s.starts_with(' ')).collect();
        assert_eq!(opts.len(), h.len() + 2);
        assert_eq!(opts[h.len() + 1], tr("STR_WHATSNEW_OK"));
        for x in h {
            // a missing key reads as the key itself
            assert!(tr(x.line) != x.line && tr(x.note) != x.note, "{}", x.line);
        }
        // the HD maker's pill only where it can be made: elsewhere its line only tells
        let hd = h.iter().position(|x| x.action == Some(Try::MakeHd)).unwrap();
        assert_eq!(whatsnew_plain(h, |_| true) & 1 << hd, 0);
        assert_ne!(whatsnew_plain(h, |t| t != Try::MakeHd) & 1 << hd, 0);
        assert!(h.iter().all(|x| x.action != Some(Try::MakeHd) || tr(x.note).contains("your own SimGolf files")));
    }
}
