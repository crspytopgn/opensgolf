//! Undo for building and terrain (the port's own, docs/PLAYING.md "Undo"). At a click on the course the parts of the game
//! that the building and terrain tools change are kept: the tiles and their corners, the objects, the holes' tees and cups and
//! the money booked to building. When the button comes up and the click changed the course (its land, terrain or holes), what
//! it changed goes on the undo list, each tile, corner and object as it was before. Ctrl+Z (Cmd+Z on a Mac) puts back only
//! that, with the money the click took or gave: the clock, the golfers and the rest of the game go on as they are. The
//! course editor's heightmap and land changes are kept as a whole saved game instead, as no time passes there. The list is
//! not saved and starts empty with each game.

use crate::app::App;
use crate::gfx::Gfx;
use sg_core::economy;
use sg_core::land::{self, Land, Object};
use std::hash::{Hash, Hasher};

/// The most clicks the list keeps (the oldest go first).
pub const MAX_STEPS: usize = 30;

/// The ledger columns building books to: its costs and refunds, and a home site's lot sold or bought back. The others are
/// the club's takings and bills, which go on while a drag lasts.
const BUILD_COLUMNS: [usize; 3] = [economy::LEDGER_HOME_SITES, economy::LEDGER_BUILD_COURSE, economy::LEDGER_FACILITIES];

#[derive(Default)]
pub struct Undo {
    /// What each click that changed the course changed, oldest first.
    steps: Vec<Step>,
    /// The course as it was at the button's press, until it comes up.
    pending: Option<Box<Before>>,
}

impl Undo {
    pub fn clear(&mut self) {
        self.steps.clear();
        self.pending = None;
    }

    /// Whether there is a click to undo (the touch Undo button shows only then, touch.rs).
    pub fn can_undo(&self) -> bool {
        !self.steps.is_empty()
    }

    /// An amount floated over a tile by the click under way (App::float_change), to float again given back if it is undone.
    pub fn note_float(&mut self, units: i32, at: (i32, i32)) {
        if let Some(b) = self.pending.as_mut() {
            b.floats.push((units, at));
        }
    }

    fn push(&mut self, s: Step) {
        self.steps.push(s);
        if self.steps.len() > MAX_STEPS {
            self.steps.remove(0);
        }
    }
}

enum Step {
    /// A click on the course: what it changed, as it was before.
    Click(Box<Click>),
    /// The whole game as a save, loaded back as a saved game is (the course editor's heightmap and land, editor.rs).
    Save(Vec<u8>),
}

/// What a vector held before a click where the click changed it, and its length then (a new object makes the list longer).
struct Was<T> {
    len: usize,
    at: Vec<(u32, T)>,
}

impl<T: Copy + PartialEq + Default> Was<T> {
    fn of(before: &[T], after: &[T]) -> Self {
        let at = before.iter().enumerate().filter(|&(i, v)| after.get(i) != Some(v)).map(|(i, &v)| (i as u32, v)).collect();
        Was { len: before.len(), at }
    }

    fn put_back(&self, v: &mut Vec<T>) {
        v.resize(self.len, T::default());
        for &(i, x) in &self.at {
            v[i as usize] = x;
        }
    }
}

/// The land's tiles, corners and objects a click changed (not the growth counters, which are the simulation's).
struct LandWas {
    ty: Was<u8>,
    flags: Was<u16>,
    layer: Was<u8>,
    var: Was<u8>,
    height: Was<u8>,
    undo: Was<u8>,
    refund: Was<i8>,
    objects: Was<Object>,
}

impl LandWas {
    fn of(b: &Land, a: &Land) -> Self {
        LandWas {
            ty: Was::of(&b.ty, &a.ty),
            flags: Was::of(&b.flags, &a.flags),
            layer: Was::of(&b.layer, &a.layer),
            var: Was::of(&b.var, &a.var),
            height: Was::of(&b.height, &a.height),
            undo: Was::of(&b.undo, &a.undo),
            refund: Was::of(&b.refund, &a.refund),
            objects: Was::of(&b.objects, &a.objects),
        }
    }

    fn put_back(&self, l: &mut Land) {
        self.ty.put_back(&mut l.ty);
        self.flags.put_back(&mut l.flags);
        self.layer.put_back(&mut l.layer);
        self.var.put_back(&mut l.var);
        self.height.put_back(&mut l.height);
        self.undo.put_back(&mut l.undo);
        self.refund.put_back(&mut l.refund);
        self.objects.put_back(&mut l.objects);
    }
}

/// The hole tool's part of a hole record: its tees, its cup and its par (the hole's statistics are the golfers').
#[derive(Clone, Copy, PartialEq)]
struct HoleWas {
    par: i32,
    game_par: i32,
    back: (i32, i32),
    fwd: (i32, i32),
    pin: (i32, i32),
}

impl HoleWas {
    fn of(h: &sg_core::golfer::Hole) -> Self {
        HoleWas { par: h.par, game_par: h.game_par, back: h.back, fwd: h.fwd, pin: h.pin }
    }
}

/// The course as it is at a press, to tell afterwards what the click changed.
struct Before {
    fingerprint: u64,
    terrain: [Vec<u8>; 4],
    corner: Vec<i8>,
    land: Land,
    holes: Vec<(HoleWas, i32)>,
    next_hole: i32,
    free_landmarks: u32,
    money: [f64; 3],
    floats: Vec<(i32, (i32, i32))>,
}

/// What one click changed, as it was before.
struct Click {
    /// The terrain's tile types, variants, texture sets and paths, and its corners.
    terrain: [Was<u8>; 4],
    corner: Was<i8>,
    land: LandWas,
    /// The holes whose tees, cup or par it changed, and what it added to each hole's building cost.
    holes: Vec<(usize, HoleWas)>,
    build_cost: Vec<(usize, i32)>,
    next_hole: Option<i32>,
    free_landmarks: Option<u32>,
    /// What it booked to each of the building columns (negative for money paid), and the amounts it floated over its tiles.
    money: [f64; 3],
    floats: Vec<(i32, (i32, i32))>,
}

impl App {
    /// A fingerprint of the course: what building and terrain tools change (not what the simulation changes as time goes
    /// by: growth, weeds, the undo records).
    fn course_fingerprint(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.terrain.ty.hash(&mut h);
        self.terrain.corner.hash(&mut h);
        self.terrain.path_kind.hash(&mut h);
        if let Some(l) = &self.land {
            l.ty.hash(&mut h);
            l.height.hash(&mut h);
            if let Ok(v) = serde_json::to_vec(&l.objects) {
                v.hash(&mut h);
            }
        }
        h.finish()
    }

    /// The building columns' totals over every year (a drag may cross into a new one).
    fn build_money(&self) -> [f64; 3] {
        BUILD_COLUMNS.map(|c| self.econ.ledger.iter().map(|y| y[c]).sum())
    }

    /// A press on the course: keeps the course as it is now, in case the click changes it.
    pub fn undo_press(&mut self) {
        let Some(land) = self.land.clone() else { return };
        let t = &self.terrain;
        self.undo.pending = Some(Box::new(Before {
            fingerprint: self.course_fingerprint(),
            terrain: [t.ty.clone(), t.variation.clone(), t.set.clone(), t.path_kind.clone()],
            corner: t.corner.clone(),
            land,
            holes: self.club.holes.iter().map(|h| (HoleWas::of(h), h.build_cost)).collect(),
            next_hole: self.club.next_hole,
            free_landmarks: self.club.free_landmarks,
            money: self.build_money(),
            floats: Vec::new(),
        }));
    }

    /// The button came up: what the click changed goes on the undo list when it changed the course.
    pub fn undo_release(&mut self) {
        let Some(b) = self.undo.pending.take() else { return };
        let Some(land) = self.land.as_ref() else { return };
        if self.course_fingerprint() == b.fingerprint {
            return;
        }
        let t = &self.terrain;
        let now = [&t.ty, &t.variation, &t.set, &t.path_kind];
        let mut holes = Vec::new();
        let mut build_cost = Vec::new();
        for (i, (h, (was, cost))) in self.club.holes.iter().zip(&b.holes).enumerate() {
            if HoleWas::of(h) != *was {
                holes.push((i, *was));
            }
            if h.build_cost != *cost {
                build_cost.push((i, h.build_cost - cost));
            }
        }
        let money = self.build_money();
        let click = Click {
            terrain: std::array::from_fn(|k| Was::of(&b.terrain[k], now[k])),
            corner: Was::of(&b.corner, &t.corner),
            land: LandWas::of(&b.land, land),
            holes,
            build_cost,
            next_hole: (self.club.next_hole != b.next_hole).then_some(b.next_hole),
            free_landmarks: (self.club.free_landmarks != b.free_landmarks).then_some(b.free_landmarks),
            money: std::array::from_fn(|k| money[k] - b.money[k]),
            floats: b.floats,
        };
        self.undo.push(Step::Click(Box::new(click)));
    }

    /// A step on the undo list made by something other than a click on the course (the course editor's heightmap and
    /// land, editor.rs): the game as it is now.
    pub fn undo_checkpoint(&mut self) {
        if self.land.is_none() {
            return;
        }
        if let Ok(b) = self.save_bytes(1) {
            self.undo.push(Step::Save(b));
        }
    }

    /// Ctrl+Z: the course as it was before the last click that changed it, and the money that click took or gave. Returns
    /// false with nothing to undo.
    pub fn undo_step(&mut self, g: &mut Gfx) -> bool {
        match self.undo.steps.pop() {
            Some(Step::Click(c)) => self.undo_click(&c),
            Some(Step::Save(b)) => self.undo_save(g, b),
            None => false,
        }
    }

    fn undo_click(&mut self, c: &Click) -> bool {
        let Some(land) = self.land.as_mut() else { return false };
        let homes =
            |l: &Land| -> Vec<(i32, i32)> { l.objects.iter().filter(|o| o.kind == land::K_HOME_SITE).map(|o| (o.a, o.b)).collect() };
        let homes_now = homes(land);
        let t = &mut self.terrain;
        for (w, v) in c.terrain.iter().zip([&mut t.ty, &mut t.variation, &mut t.set, &mut t.path_kind]) {
            w.put_back(v);
        }
        c.corner.put_back(&mut t.corner);
        c.land.put_back(land);
        land.green_looks_to(&mut self.terrain);
        // a home site the undo takes away loses its residents and its record, as when it is demolished (App::undo_tile)
        let homes_then = homes(land);
        for (a, b) in homes_now.into_iter().filter(|h| !homes_then.contains(h)) {
            self.club.evict(a, b);
            self.home_removed(a, b);
        }
        for &(i, w) in &c.holes {
            let h = &mut self.club.holes[i];
            (h.par, h.game_par, h.back, h.fwd, h.pin) = (w.par, w.game_par, w.back, w.fwd, w.pin);
        }
        for &(i, d) in &c.build_cost {
            self.club.holes[i].build_cost -= d;
        }
        if let Some(n) = c.next_hole {
            self.club.next_hole = n;
        }
        if let Some(f) = c.free_landmarks {
            self.club.free_landmarks = f;
        }
        // the money back, booked against the columns the click booked to
        let mut back = 0.0;
        for (k, &col) in BUILD_COLUMNS.iter().enumerate() {
            if c.money[k] != 0.0 {
                self.econ.book(col, -c.money[k]);
                back -= c.money[k];
            }
        }
        if back > 0.0 {
            self.econ.earn(back);
        } else if back < 0.0 {
            self.econ.spend(-back);
        }
        // each tile's amount floats again over it, given back
        for &(units, at) in &c.floats {
            self.float_money(-units, at);
        }
        self.after_object_change();
        true
    }

    /// Undoes the last click as Ctrl+Z does, without the graphics a saved game's step needs (tests).
    #[cfg(test)]
    pub(crate) fn undo_click_step(&mut self) -> bool {
        match self.undo.steps.pop() {
            Some(Step::Click(c)) => self.undo_click(&c),
            _ => false,
        }
    }

    /// The course editor's step: the whole game as it was, loaded as a saved game is.
    fn undo_save(&mut self, g: &mut Gfx, b: Vec<u8>) -> bool {
        let Ok(s) = crate::app::read_save_bytes(&b, "undo") else { return false };
        // what the player was doing stays as it was: the open panel, its tool and the view
        let steps = std::mem::take(&mut self.undo.steps);
        let (panel, alt, edit, tool) = (self.panel, self.pstate.alt, self.edit, self.tool);
        if self.load_save_with(g, s, true).is_err() {
            self.undo.steps = steps;
            return false;
        }
        self.undo.steps = steps;
        self.panel = panel;
        self.pstate.alt = alt;
        self.edit = edit;
        self.tool = tool;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn was_puts_back_changed_places_and_length() {
        let before = vec![1u8, 2, 3];
        let mut v = vec![1u8, 9, 3, 4];
        let w = Was::of(&before, &v);
        assert_eq!(w.at, vec![(1, 2)]);
        // a later change elsewhere stays
        v[0] = 7;
        w.put_back(&mut v);
        assert_eq!(v, vec![7, 2, 3]);
    }
}
