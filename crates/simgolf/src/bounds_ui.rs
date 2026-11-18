//! Hole boundaries' brush (the Hole boundaries extra, sg_core::bounds; docs/PLAYING.md, "Hole boundaries"): Hole Stats'
//! Boundary button frames the hole and puts a brush in hand that paints its playing area. A click or a drag adds tiles,
//! with Shift (or the lowering long press on a touch screen) it takes them out, Delete takes the whole area away, and Space,
//! Esc or a right click puts the brush down as any tool. While the brush is in hand the area is tinted green over the
//! course, its hole's tees and green ringed, and the tiles outside it dimmed. The brush only changes the area: nothing is
//! built and nothing is charged.
use crate::app::*;
use crate::gfx::Gfx;
use crate::ui::Screen as Ui;
use sg_core::lang::{tr, trf};

/// The brush's tool number (`App::tool`): after the original's 0 paint, 1 raise, 2 path, 4 building and 5 undo.
pub const TOOL: i32 = 7;

impl App {
    /// Hole Stats' Boundary: the brush for hole `h`'s playing area in hand, the hole framed.
    pub fn open_bounds(&mut self, h: usize) {
        if !self.club.extras.bounds || !(1..=18).contains(&h) || self.club.holes[h].par == 0 {
            return;
        }
        self.xs.bounds_hole = Some(h);
        self.tool = TOOL;
        self.edit = true;
        self.fit_hole(h);
        self.show_toast(&trf("STR_TOAST_BOUNDS_BRUSH", &[("HOLE", &h)]));
    }

    /// The hole the brush paints, while it is in hand.
    pub fn bounds_brush(&self) -> Option<usize> {
        self.xs.bounds_hole.filter(|_| self.edit && self.tool == TOOL && self.club.extras.bounds)
    }

    /// A click or a drag of the brush on tile (a, b): into the area, or out of it with `remove`.
    pub fn paint_bound(&mut self, a: i32, b: i32, remove: bool) {
        let Some(h) = self.bounds_brush() else { return };
        self.act(sg_core::action::Action::Bound { hole: h as i32, x: a, y: b, on: !remove });
    }

    /// The Bound action (replay.rs).
    pub fn set_bound(&mut self, hole: i32, a: i32, b: i32, on: bool) -> bool {
        if !self.club.extras.bounds {
            return false;
        }
        self.club.bounds.set(hole, a, b, on)
    }

    /// Delete with the brush in hand: the hole's area taken away (the BoundClear action).
    pub fn clear_bound(&mut self, hole: i32) -> bool {
        if self.rec.depth_zero() {
            return self.act(sg_core::action::Action::BoundClear { hole });
        }
        self.club.bounds.clear(hole);
        self.show_toast(&trf("STR_TOAST_BOUNDS_CLEARED", &[("HOLE", &hole)]));
        true
    }

    /// The area over the course while the brush is in hand (draw_hud, under the golfers' names).
    pub fn draw_bounds_view(&mut self, g: &mut Gfx, s: &Ui) {
        let Some(h) = self.bounds_brush() else { return };
        let has = self.club.bounds.area(h as i32).is_some();
        let corner = |x: i32, y: i32| {
            let (wx, wz) = self.units_to_world(x, y);
            self.screen_of_world(wx, wz)
        };
        let n = sg_core::course::n();
        let marks = [self.club.holes[h].back, self.club.holes[h].pin];
        for a in 0..n {
            for b in 0..n {
                let inside = self.club.bounds.inside(h as i32, a, b);
                let mark = marks.contains(&(a, b));
                // with no area yet only the tees and green are marked; once there is one, the outside is dimmed
                let col = if inside {
                    [0.2, 0.85, 0.3, 0.38]
                } else if mark {
                    [1.0, 0.9, 0.2, 0.5]
                } else if has {
                    [0.0, 0.0, 0.0, 0.28]
                } else {
                    continue;
                };
                if self.screen_of(a * 1024 + 512, b * 1024 + 512).is_none() {
                    continue;
                }
                let (x0, y0, x1, y1) = (a * 1024, b * 1024, a * 1024 + 1024, b * 1024 + 1024);
                if let (Some(p0), Some(p1), Some(p2), Some(p3)) = (corner(x0, y0), corner(x1, y0), corner(x1, y1), corner(x0, y1)) {
                    s.quad(g, [p0, p1, p2, p3], col);
                }
            }
        }
        let t = trf("STR_BOUNDS_LEGEND", &[("HOLE", &h), ("TILES", &self.club.bounds.area(h as i32).map_or(0, |v| v.len()))]);
        let (_, y) = (0.0, 34.0);
        s.text_centered(g, 400.0, y, &t, 14.0, [1.0, 1.0, 1.0, 1.0]);
        s.text_centered(g, 400.0, y + 18.0, tr("STR_BOUNDS_HINT"), 12.0, [1.0, 1.0, 0.85, 1.0]);
    }
}
