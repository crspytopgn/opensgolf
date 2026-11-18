//! Practise a hole (an OpenSGolf extra on the Info page, off by default; docs/PLAYING.md, "Practise a hole"): Hole Stats'
//! Practise button sends the player's pro out to play that one hole, as the pro panel's practice round sends him out to
//! play the course (sg_core::pro `request_practice_hole`): he starts on the hole's tee with a partner, the player aims
//! his shots as usual, and his round ends when he holes out. The ticker then gives his score against par, beside the
//! hole's own stroke average and its Fun rating, and Hole Stats can send him out again. A practice hole is not a round:
//! nothing goes in the round records.
//!
//! While he practises, the Mulligan button (and Backspace) takes his last shot back: he and his ball go back to where he
//! hit it from and the stroke is taken off, to play it again. Only the last shot, and only while he waits for the next.
use crate::app::App;
use sg_core::lang::{tr, trf};

impl App {
    /// Hole Stats' Practise: the pro sent out to play hole `h` alone. Says why not when he cannot go (he is already out,
    /// his skills are still to hand out, a round cannot start now).
    pub fn practice_hole(&mut self, h: usize) -> bool {
        // the interface's call is an action (replay.rs): logged, then applied by App::apply
        if self.rec.depth_zero() {
            return self.act(sg_core::action::Action::Practice { hole: h as i32 });
        }
        if self.club.gary != -1 {
            self.show_toast(&trf("STR_TOAST_PRO_ALREADY_PLAYING", &[("PRO", &self.pro_name())]));
            return false;
        }
        if self.club.pro_skill[..12].iter().all(|&v| v == 0) {
            self.show_toast(tr("STR_TOAST_PRACTICE_SKILLS_FIRST"));
            return false;
        }
        if !self.club.request_practice_hole(h as i32) {
            self.show_toast(tr("STR_TOAST_ROUND_CANT_START"));
            return false;
        }
        self.show_toast(&trf("STR_TOAST_PRACTICE_HOLE", &[("PRO", &self.pro_name()), ("HOLE", &h)]));
        true
    }

    /// The Mulligan button and Backspace while the pro practises a hole: his last shot taken back.
    pub fn mulligan(&mut self) -> bool {
        if self.rec.depth_zero() {
            if !self.club.can_mulligan() {
                return false;
            }
            return self.act(sg_core::action::Action::Mulligan);
        }
        if !self.club.mulligan() {
            return false;
        }
        self.touch_target.target = None;
        self.show_toast(tr("STR_TOAST_MULLIGAN"));
        self.ui_sound(0x18);
        true
    }

    /// The pro has holed out on the hole he practised: his score against par, the hole's stroke average and its Fun.
    pub fn practice_done(&mut self, hole: i32, strokes: i32, par: i32) {
        let d = strokes - par;
        let against = match d {
            0 => tr("STR_PRACTICE_LEVEL").to_string(),
            d if d < 0 => trf("STR_PRACTICE_UNDER", &[("N", &-d)]),
            d => trf("STR_PRACTICE_OVER", &[("N", &d)]),
        };
        let h = self.club.holes.get(hole as usize);
        let played: i32 = h.map(|h| h.hist.iter().sum()).unwrap_or(0);
        let total: i32 = h.map(|h| h.hist.iter().enumerate().map(|(i, &n)| (i % 11) as i32 * n).sum()).unwrap_or(0);
        let avg = if played > 0 { format!("{:.2}", total as f64 / played as f64) } else { "-".into() };
        let text = trf(
            "STR_PRACTICE_DONE",
            &[("PRO", &self.pro_name()), ("HOLE", &hole), ("STROKES", &strokes), ("PAR", &par), ("AGAINST", &against), ("AVERAGE", &avg)],
        );
        println!("[{:6.1}s] {text}", self.sim_time);
        self.post_message(&text, 1, -1);
        self.show_toast(&text);
    }
}

#[cfg(test)]
mod tests {
    use sg_core::golfer::Club;

    #[test]
    fn a_practice_hole_needs_an_open_hole_and_no_round_under_way() {
        let mut cl = Club::default();
        assert!(!cl.request_practice_hole(1), "no hole open");
        cl.next_hole = 3;
        cl.holes[2].par = 4;
        assert!(!cl.request_practice_hole(5), "not open");
        assert!(cl.request_practice_hole(2));
        assert_eq!(cl.practice_hole, 2);
        assert!(!cl.can_mulligan(), "nothing to take back yet");
        assert!(!cl.mulligan());
    }
}
