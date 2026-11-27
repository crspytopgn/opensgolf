//! Multiplayer: the weekly challenge, neighbours leagues and shared clubs. Built with the `multiplayer`
//! feature, and even then off until the player turns the Multiplayer extra on.
//!
//! * [`challenge`]: the weekly rival-courses challenge, the same for everyone each ISO week.
//! * [`club`]: a shared club in the game: making, joining, playing it through the club's room, leaving.
//! * [`clubs_ui`]: the shared clubs' page (the list, joining, making a club).
//! * [`conn`]: a shared club's WebSocket, on its own thread.
//! * [`done`]: "Challenge complete", over the course when the weekly challenge's year ends.
//! * [`fair`]: the same rules for everyone in the weekly challenge and the leagues (extras, cheats).
//! * [`hud`]: the reminder on the course of what the multiplayer game being played is about.
//! * [`league`]: neighbours leagues: each player's own plot of one property, compared in a table.
//! * [`lobby`]: the weekly challenge's page (nickname, group, the board, turning Multiplayer on).
//! * [`lockstep`]: a shared game's simulation, with nothing the screen, the art or the drawing's clock sees reaching it.
//! * [`result`]: a finished challenge's result, as the leaderboard Worker (multiplayer/worker) reads it.
//! * [`rival`]: the weekly challenge in the game (a Start New Game row) and its result sent when 2001 ends.
//! * [`session`]: a shared game's clock and the order of its actions, the game's side and the server's.
//! * [`settings`]: the player's own settings.json key (on, id, nickname, group).
//! * [`title`]: the title menu's "This week's challenge" button.
//! * [`version`]: what two games, or a game and the server, must agree on before they play together.
// parts the game does not call yet (the shared course comes next)
#![allow(dead_code)]

pub mod challenge;
pub mod club;
pub mod clubs_ui;
pub mod conn;
pub mod done;
pub mod fair;
pub mod hud;
pub mod league;
pub mod lobby;
pub mod lockstep;
pub mod names;
pub mod result;
pub mod rival;
pub mod session;
pub mod settings;
pub mod title;
pub mod version;

/// The multiplayer text (multiplayer/lang/en.txt) joins the English table once.
pub fn texts() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| sg_core::lang::add_english(include_str!("../../../../multiplayer/lang/en.txt")));
}

/// The App's multiplayer part (its one field outside this folder). Making it adds the multiplayer text.
#[derive(Debug)]
pub struct State {
    /// The shared game being played.
    pub session: Option<session::Session>,
    /// The shared club being played (its connection and where it is).
    pub live: Option<club::Live>,
    /// The player's id in a club, when not settings.json's (the tests play two players in one process).
    pub player: Option<String>,
    /// A club to make from the new game being started (clubs_ui.rs), by its name.
    pub make_next: Option<String>,
    /// The next new game starts with half its usual money (a budget week: rival.rs).
    pub halve_money: bool,
    /// A private club being made: told its code and remembered once the server answers (club.rs).
    pub making_private: bool,
    /// The club to make next is private (clubs_ui.rs).
    pub make_private: bool,
}

impl Default for State {
    fn default() -> State {
        texts();
        State { session: None, live: None, player: None, make_next: None, halve_money: false, making_private: false, make_private: false }
    }
}

#[cfg(test)]
mod tests {
    /// Every key the multiplayer code uses is in multiplayer/lang/en.txt once, and every key there is used (lang/en.txt's
    /// own test leaves this folder out).
    #[test]
    fn the_multiplayer_text_has_every_key_once() {
        let text = include_str!("../../../../multiplayer/lang/en.txt");
        let keys: Vec<&str> = sg_core::lang::parse(text).into_iter().map(|(k, _)| k).collect();
        let mut seen = std::collections::HashSet::new();
        for k in &keys {
            assert!(seen.insert(*k), "{k} twice");
            assert!(!sg_core::lang::parse(sg_core::lang::EN).iter().any(|(e, _)| e == k), "{k} is in lang/en.txt too");
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mp");
        let mut used = std::collections::HashSet::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let src = std::fs::read_to_string(e.path()).unwrap();
            let mut rest = src.as_str();
            while let Some(i) = rest.find("\"STR_") {
                let tail = &rest[i + 1..];
                let n = tail.find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')).unwrap_or(tail.len());
                if n > 4 && tail[n..].starts_with('"') {
                    assert!(seen.contains(&tail[..n]), "{} (in {}) is not in multiplayer/lang/en.txt", &tail[..n], e.path().display());
                    used.insert(tail[..n].to_string());
                }
                rest = &tail[n..];
            }
        }
        for k in &keys {
            assert!(used.contains(*k), "{k} is in multiplayer/lang/en.txt but no code uses it");
        }
        super::texts();
        assert_eq!(sg_core::lang::tr("STR_MP_CLUBS"), "Shared clubs");
    }
}
