//! Hole boundaries (an OpenSGolf extra, `Extras::bounds`; docs/PLAYING.md, "Hole boundaries"): a hole may have a playing
//! area, the tiles the player marks for it. A Course A golfer playing that hole aims inside it (the shot planner rates a
//! landing outside it as out of bounds), and a ball that comes to rest outside it is out of bounds as land the club does
//! not own is in the original: back to where it was hit, with a stroke added (`Club::rest`). A hole without an area, a
//! Course B hole (the Second course extra) and every hole with the extra off play as before. None of this is the
//! original's.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Each hole's playing area: its tiles (`a * n + b` on the map of edge n), sorted, by hole number.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub areas: BTreeMap<u8, Vec<u32>>,
}

impl Bounds {
    pub fn is_empty(&self) -> bool {
        self.areas.values().all(|v| v.is_empty())
    }

    /// Hole `h`'s area, if it has one.
    pub fn area(&self, h: i32) -> Option<&[u32]> {
        u8::try_from(h).ok().and_then(|h| self.areas.get(&h)).map(|v| v.as_slice()).filter(|v| !v.is_empty())
    }

    /// Whether tile (a, b) is outside hole `h`'s area (false when the hole has none, or off the map).
    pub fn outside(&self, h: i32, a: i32, b: i32) -> bool {
        let n = crate::course::n();
        if !(0..n).contains(&a) || !(0..n).contains(&b) {
            return false;
        }
        self.area(h).is_some_and(|t| t.binary_search(&((a * n + b) as u32)).is_err())
    }

    /// Whether tile (a, b) is in hole `h`'s area.
    pub fn inside(&self, h: i32, a: i32, b: i32) -> bool {
        let n = crate::course::n();
        (0..n).contains(&a) && (0..n).contains(&b) && self.area(h).is_some_and(|t| t.binary_search(&((a * n + b) as u32)).is_ok())
    }

    /// Adds tile (a, b) to hole `h`'s area, or takes it out (`on` false). Returns whether it changed.
    pub fn set(&mut self, h: i32, a: i32, b: i32, on: bool) -> bool {
        let n = crate::course::n();
        let Ok(hk) = u8::try_from(h) else { return false };
        if !(0..n).contains(&a) || !(0..n).contains(&b) {
            return false;
        }
        let t = (a * n + b) as u32;
        let v = self.areas.entry(hk).or_default();
        let changed = match (v.binary_search(&t), on) {
            (Err(i), true) => {
                v.insert(i, t);
                true
            }
            (Ok(i), false) => {
                v.remove(i);
                true
            }
            _ => false,
        };
        if v.is_empty() {
            self.areas.remove(&hk);
        }
        changed
    }

    /// Hole `h`'s area taken away.
    pub fn clear(&mut self, h: i32) {
        if let Ok(h) = u8::try_from(h) {
            self.areas.remove(&h);
        }
    }
}

impl crate::golfer::Club {
    /// Whether golfer g's ball at tile (a, b) is out of bounds by its hole's area (Hole boundaries): only with the extra on,
    /// on Course A, for a hole that has an area. The hole's tees and the tiles round its pin are never outside, so a hole
    /// whose area leaves them out can still be teed off and finished.
    pub fn outside_area(&self, g: usize, a: i32, b: i32) -> bool {
        let gg = &self.g[g];
        if !self.extras.bounds || gg.course_b || !(1..=18).contains(&gg.hole) || !self.bounds.outside(gg.hole, a, b) {
            return false;
        }
        let hr = &self.holes[gg.hole as usize];
        let tees = [Some(hr.back), (hr.fwd != (0, 0)).then_some(hr.fwd), hr.front_tee, hr.middle_tee];
        !(tees.contains(&Some((a, b))) || ((a - hr.pin.0).abs() <= 1 && (b - hr.pin.1).abs() <= 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hole_without_an_area_has_no_outside() {
        let mut b = Bounds::default();
        assert!(!b.outside(3, 10, 10));
        assert!(b.set(3, 10, 10, true));
        assert!(!b.set(3, 10, 10, true), "already in");
        assert!(!b.outside(3, 10, 10) && b.inside(3, 10, 10));
        assert!(b.outside(3, 10, 11));
        assert!(!b.outside(4, 10, 11), "another hole has none");
        assert!(b.set(3, 10, 10, false));
        assert!(b.is_empty() && !b.outside(3, 10, 11));
        // only saved when there is one
        assert!(serde_json::to_string(&Bounds::default()).is_ok());
    }

    #[test]
    fn the_tees_and_the_green_are_never_outside() {
        let mut cl = crate::golfer::Club::default();
        if cl.g.is_empty() {
            cl.g.push(Default::default());
        }
        cl.g[0].hole = 2;
        cl.holes[2].par = 4;
        cl.holes[2].back = (5, 5);
        cl.holes[2].pin = (20, 5);
        cl.bounds.set(2, 10, 5, true);
        assert!(!cl.outside_area(0, 30, 30), "the extra is off");
        cl.extras.bounds = true;
        assert!(cl.outside_area(0, 30, 30));
        assert!(!cl.outside_area(0, 10, 5), "in the area");
        assert!(!cl.outside_area(0, 5, 5), "the back tee");
        assert!(!cl.outside_area(0, 21, 4), "round the pin");
        assert!(cl.outside_area(0, 22, 5));
    }
}
