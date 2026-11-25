//! The player's multiplayer settings: their own key in settings.json ("multiplayer"), so nothing in the shared settings
//! code changes and a public merge never touches it. Off until the player turns it on.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// The settings.json key.
pub const SETTINGS_KEY: &str = "multiplayer";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Multiplayer is on (the weekly challenge is offered, results may be sent).
    pub on: bool,
    /// The random id the game made for this player (16 hex digits), made the first time a result is sent.
    pub player: String,
    /// The name shown on the boards (none: one made from the id).
    pub nick: String,
    /// A friends' group code (6 of A-Z 0-9), or empty for the public board.
    pub group: String,
    /// The private clubs this player made or joined, newest first (the shared clubs' page lists them with their codes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clubs: Vec<MyClub>,
    /// The neighbours leagues this player made or joined (id: the league's code), newest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leagues: Vec<MyClub>,
}

/// A private club the player knows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MyClub {
    pub id: String,
    pub name: String,
}

/// Private clubs remembered.
pub const MY_CLUBS: usize = 10;

/// Remembers a private club (first in the list).
pub fn remember_club(id: &str, name: &str) {
    let mut s = get();
    s.clubs.retain(|c| c.id != id);
    s.clubs.insert(0, MyClub { id: id.into(), name: name.into() });
    s.clubs.truncate(MY_CLUBS);
    save(s);
}

thread_local! {
    static SETTINGS: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

fn path() -> std::path::PathBuf {
    crate::app::save_dir().join(crate::hd::SETTINGS_FILE)
}

fn settings_json() -> serde_json::Value {
    sg_core::fsutil::read_file(path())
        .and_then(|d| serde_json::from_slice::<serde_json::Value>(&d).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// The settings in use (read from settings.json the first time).
pub fn get() -> Settings {
    SETTINGS.with(|c| {
        c.borrow_mut().get_or_insert_with(|| serde_json::from_value(settings_json()[SETTINGS_KEY].clone()).unwrap_or_default()).clone()
    })
}

/// Saves the settings into settings.json, keeping its other keys.
pub fn save(s: Settings) {
    let mut v = settings_json();
    if let Ok(c) = serde_json::to_value(&s) {
        v[SETTINGS_KEY] = c;
    }
    if let Ok(t) = serde_json::to_string_pretty(&v) {
        sg_core::fsutil::write_file(path(), t.as_bytes());
    }
    SETTINGS.with(|c| *c.borrow_mut() = Some(s));
}

/// Some entropy for a new player id: the clock, and addresses that differ run to run.
#[cfg(target_arch = "wasm32")]
fn entropy() -> u128 {
    #[link(wasm_import_module = "env")]
    extern "C" {
        /// 32 random bits from the browser (web/opensgolf.js).
        fn sg_random() -> u32;
    }
    // SAFETY: no arguments
    let r = |s: u32| u128::from(unsafe { sg_random() }) << s;
    (r(0) | r(32) | r(64) | r(96)) ^ (miniquad::date::now() * 1e6) as u128
}

#[cfg(not(target_arch = "wasm32"))]
fn entropy() -> u128 {
    let t = (miniquad::date::now() * 1e6) as u128;
    let local = 0u8;
    let a = &local as *const u8 as usize as u128;
    let b = Box::new(0u8);
    let c = &*b as *const u8 as usize as u128;
    t ^ a << 64 ^ c << 32 ^ (std::process::id() as u128) << 96
}

/// The settings with a player id (made and saved when there is none yet).
pub fn with_player() -> Settings {
    let mut s = get();
    if !is_player(&s.player) {
        s.player = super::result::new_player_id(entropy());
        save(s.clone());
    }
    s
}

pub fn is_player(p: &str) -> bool {
    (16..=32).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The nickname sent: the player's own, or "Golfer" and three digits of their id.
pub fn nick(s: &Settings) -> String {
    let n = s.nick.trim();
    if (3..=16).contains(&n.chars().count()) {
        return n.to_string();
    }
    let k = u32::from_str_radix(s.player.get(..6).unwrap_or("0"), 16).unwrap_or(0) % 1000;
    format!("Golfer {k:03}")
}

/// The group code sent (anything that isn't one is the public board).
pub fn group(s: &Settings) -> String {
    let g = s.group.trim().to_ascii_uppercase();
    if g.len() == 6 && g.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        g
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nick_and_group_fall_back() {
        let s = Settings { player: "0123456789abcdef".into(), nick: " x ".into(), group: "friend".into(), ..Default::default() };
        assert_eq!(nick(&s), format!("Golfer {:03}", 0x012345 % 1000));
        assert_eq!(group(&s), "FRIEND");
        let s = Settings { nick: "Big Hitter".into(), group: "FRIENDS".into(), ..s };
        assert_eq!(nick(&s), "Big Hitter");
        assert_eq!(group(&s), "");
        assert!(is_player("0123456789abcdef") && !is_player("0123456789ABCDEF") && !is_player("abc"));
    }

    #[test]
    fn unknown_keys_and_missing_fields_read() {
        let s: Settings = serde_json::from_value(serde_json::json!({"on": true, "later": 1})).unwrap();
        assert!(s.on && s.player.is_empty());
    }
}
