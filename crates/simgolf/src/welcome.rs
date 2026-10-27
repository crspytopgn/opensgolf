//! The welcome boxes (the port's own, none of it in the original; docs/PLAYING.md "How would you like to play?"):
//!
//! * "How would you like to play?", once: at the first title menu of a new player, and once for a player of an older
//!   version (settings.json without "welcome_shown"). Its three choices put one of the presets of the OpenSGolf extras in
//!   use (extras_ui.rs Preset: the original game, Original + helpers, the full OpenSGolf); Esc, a right-click or a click
//!   outside the box keeps the settings as they are (for a new player, the original game).
//! * "What's new in OpenSGolf {VERSION}", once after the game is updated to a version with highlights (`HIGHLIGHTS`,
//!   compared with settings.json's "last_version_seen"; a new player, who gets the first box, does not get this one). A
//!   highlight that is an option has a Try it pill that puts it in use (or opens its Preferences page); See everything
//!   that's new opens the changelog in the browser.
//!
//! Neither shows in scripted runs (--png, --time, --screen: the stills and the tests stay as they were) nor over another
//! box; both wait for the title menu to settle as the support box does (support.rs).
//!
//! Test hooks: SG_WELCOME=1 and SG_WHATSNEW=1 show the box at the first title menu whatever settings.json says (for
//! stills; nothing is written then).
use crate::app::{App, Screen};
use crate::extras_ui::Preset;
use crate::popup_ui::PopupKind;
use sg_core::lang::{tr, trf};
use std::cell::Cell;

/// The changelog (box B's See everything that's new, Preferences > More > What's new...).
pub const CHANGELOG_URL: &str = "https://github.com/crspytopgn/opensgolf/blob/main/CHANGELOG.md";
/// settings.json's keys.
pub const WELCOME_KEY: &str = "welcome_shown";
pub const VERSION_KEY: &str = "last_version_seen";
/// How long the title menu shows before a box comes up there (as the support box, support.rs).
const TITLE_SETTLE: f64 = 1.5;
/// The width the boxes' notes wrap at: the original's font, the free set's (whose face is wider).
const NOTE_W: [f32; 2] = [440.0, 540.0];

/// What a highlight's Try it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Try {
    /// Enhanced graphics on.
    Enhanced,
    /// The Mouse aim and swing extra on (extras.rs).
    MouseSwing,
    /// Sandbox: no new dandelions on for the next games (sg_core::extras).
    NoWeeds,
    /// Opens a Preferences page.
    Page(PopupKind),
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

/// Each version's highlights, oldest first: a release adds its entry here (and its lines to lang/en.txt).
pub const HIGHLIGHTS: &[(&str, &[Highlight])] = &[(
    "0.6",
    &[
        hl("STR_WHATSNEW_SMOOTH", "STR_WHATSNEW_SMOOTH_NOTE", Some(Try::Enhanced)),
        hl("STR_WHATSNEW_HD", "STR_WHATSNEW_HD_NOTE", Some(Try::Page(PopupKind::PrefsGraphics))),
        hl("STR_WHATSNEW_FACES", "STR_WHATSNEW_FACES_NOTE", None),
        hl("STR_WHATSNEW_SWING", "STR_WHATSNEW_SWING_NOTE", Some(Try::MouseSwing)),
        hl("STR_WHATSNEW_WEEDS", "STR_WHATSNEW_WEEDS_NOTE", Some(Try::NoWeeds)),
        hl("STR_WHATSNEW_UPDATES", "STR_WHATSNEW_UPDATES_NOTE", None),
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

/// The newest version's highlights between `seen` (none: a player from before the key) and `now`, both included at `now`
/// and left out at `seen`.
pub fn news(seen: Option<&str>, now: &str) -> Option<(&'static str, &'static [Highlight])> {
    let now = parse(now)?;
    let seen = seen.and_then(parse).unwrap_or((0, 0, 0));
    HIGHLIGHTS.iter().rev().find(|(v, _)| parse(v).is_some_and(|v| v > seen && v <= now)).copied()
}

thread_local! {
    /// Whether settings.json had nothing in it when the game started (read before anything else writes it).
    static NEW_PLAYER: Cell<Option<bool>> = const { Cell::new(None) };
}

fn settings_json() -> serde_json::Value {
    sg_core::fsutil::read_file(crate::app::save_dir().join(crate::hd::SETTINGS_FILE))
        .and_then(|d| serde_json::from_slice::<serde_json::Value>(&d).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// At the very start, before anything writes settings.json: whether this is a new player (no settings saved yet).
pub fn note_first_start() {
    let new = settings_json().as_object().is_none_or(|o| o.is_empty());
    NEW_PLAYER.with(|c| c.set(Some(new)));
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
    let v = settings_json();
    let new = NEW_PLAYER.with(|c| c.get()).unwrap_or(false);
    let now = env!("CARGO_PKG_VERSION");
    let flag = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty() && v != "0");
    let (fw, fn_) = (flag("SG_WELCOME"), flag("SG_WHATSNEW"));
    if fw || fn_ {
        return State { welcome_due: fw, news_due: fn_, news: news(None, now), forced: true, ..Default::default() };
    }
    if scripted {
        return State::default();
    }
    let welcome_due = v[WELCOME_KEY].as_bool() != Some(true);
    let news = if new { None } else { news(v[VERSION_KEY].as_str(), now) };
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

/// The preset of each choice of the first box, in order.
pub const CHOICES: [Preset; 3] = [Preset::Original, Preset::Helpers, Preset::OpenSGolf];

/// `t` wrapped as the boxes' notes (`free`: on the free set).
fn notes(t: &str, free: bool) -> impl Iterator<Item = String> {
    crate::extras::wrap(crate::popup_ui::NOTE_FONT, t, NOTE_W[free as usize]).into_iter().map(|l| format!("\t{l}"))
}

/// The first box's lines: the title, a gap (clear of the free set's header band), each choice with its note, and the
/// line that says where to change it.
pub fn welcome_lines(free: bool) -> Vec<String> {
    let mut v = vec![tr("STR_WELCOME_TITLE").to_string(), "\t".to_string()];
    for (line, note) in [
        ("STR_WELCOME_ORIGINAL", "STR_WELCOME_ORIGINAL_NOTE"),
        ("STR_WELCOME_HELPERS", "STR_WELCOME_HELPERS_NOTE"),
        ("STR_WELCOME_OPENSGOLF", "STR_WELCOME_OPENSGOLF_NOTE"),
    ] {
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

/// The second box's options that only tell (no Try it): drawn with a bullet, never lit.
pub fn whatsnew_plain(h: &[Highlight]) -> u32 {
    h.iter().enumerate().filter(|(_, x)| x.action.is_none()).fold(0, |m, (k, _)| m | 1 << k)
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
        if std::mem::take(&mut w.welcome_due) {
            if write {
                save(WELCOME_KEY, serde_json::Value::Bool(true));
                if NEW_PLAYER.with(|c| c.get()).unwrap_or(false) {
                    save(VERSION_KEY, env!("CARGO_PKG_VERSION").into());
                }
            }
            self.hover = -1;
            self.open_popup(PopupKind::Welcome);
            return;
        }
        if std::mem::take(&mut w.news_due) && w.news.is_some() {
            if write {
                save(VERSION_KEY, env!("CARGO_PKG_VERSION").into());
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

    /// Whether a highlight's option is already in use (its pill says On).
    pub fn tried(&self, t: Try) -> bool {
        match t {
            Try::Enhanced => crate::enhanced::settings().enabled,
            Try::MouseSwing => crate::extras::mouse_swing(),
            Try::NoWeeds => self.extras_next.no_weeds,
            Try::Page(_) => false,
        }
    }

    /// The first box's answer (`k` its option, none for Esc, a right-click or a click outside it: nothing changes).
    pub fn welcome_answer(&mut self, g: &mut crate::gfx::Gfx, k: Option<usize>) {
        if let Some(p) = k.and_then(|k| CHOICES.get(k)) {
            self.put_game_style(g, *p);
        }
    }

    /// The second box's option `k`: a highlight's Try it (true: the box stays up), See everything that's new (stays up),
    /// OK; none (Esc, outside) closes it.
    pub fn whatsnew_pick(&mut self, g: &mut crate::gfx::Gfx, k: Option<usize>) -> bool {
        let h = self.whatsnew_shown();
        let Some(k) = k else { return false };
        if k == h.len() {
            self.open_changelog();
            return true;
        }
        let Some(x) = h.get(k) else { return false };
        match x.action {
            None => true,
            Some(Try::Page(kind)) => {
                self.close_popup();
                self.open_popup(kind);
                false
            }
            Some(t) if self.tried(t) => true,
            Some(Try::Enhanced) => {
                let mut enh = crate::enhanced::settings();
                enh.enabled = true;
                if self.apply_enhanced(g, enh) {
                    self.ui_ok &= crate::load_art(self, g);
                }
                true
            }
            Some(Try::MouseSwing) => {
                let mut ui = crate::extras::settings();
                ui.mouse_swing = true;
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

    #[test]
    fn versions_read_and_compare() {
        assert_eq!(parse("0.6"), Some((0, 6, 0)));
        assert_eq!(parse("0.6.0"), parse("0.6"));
        assert_eq!(parse("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse("x"), None);
        assert!(parse("0.5.12") < parse("0.6"));
    }

    #[test]
    fn whats_new_shows_once_after_an_update_to_a_version_with_highlights() {
        assert_eq!(news(None, "0.6.0").map(|n| n.0), Some("0.6"), "a player from before the key");
        assert_eq!(news(Some("0.5.12"), "0.6.0").map(|n| n.0), Some("0.6"));
        assert!(news(Some("0.6.0"), "0.6.0").is_none(), "seen");
        assert!(news(Some("0.6.0"), "0.6.1").is_none(), "no highlights for 0.6.1");
        assert!(news(Some("0.5.12"), "0.5.13").is_none(), "an older game");
    }

    #[test]
    fn the_welcome_box_has_three_choices_each_with_a_note() {
        let l = welcome_lines(false);
        let opts: Vec<usize> = l.iter().enumerate().filter(|(_, s)| s.starts_with(' ')).map(|(i, _)| i).collect();
        assert_eq!(opts.len(), CHOICES.len());
        for i in opts {
            assert!(l[i + 1].starts_with('\t') && l[i + 1].len() > 1, "{}", l[i]);
        }
        assert!(l.last().unwrap().contains("Game options"));
    }

    #[test]
    fn whats_new_lists_each_highlight_then_see_everything_and_ok() {
        let (v, h) = news(None, "0.6.0").unwrap();
        let l = whatsnew_lines(v, h, false);
        assert_eq!(l[0], "What's new in OpenSGolf 0.6");
        let opts: Vec<&String> = l.iter().filter(|s| s.starts_with(' ')).collect();
        assert_eq!(opts.len(), h.len() + 2);
        assert_eq!(opts[h.len() + 1], tr("STR_WHATSNEW_OK"));
        for x in h {
            // a missing key reads as the key itself
            assert!(tr(x.line) != x.line && tr(x.note) != x.note, "{}", x.line);
        }
    }
}
