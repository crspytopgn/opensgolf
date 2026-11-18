//! Show a hole (an OpenSGolf extra on the Info page, off by default; docs/PLAYING.md, "Show a hole"): Hole Stats' Show hole
//! button, and the digit keys that jump to a hole, frame the whole hole on the screen: its tees, the bends of its route and
//! its green, as close as they all fit over the dock's panel. The original's digit keys only centre the camera between the
//! back tee and the pin, at the zoom it has. With Classic graphics the zoom is the closest of the original's three levels
//! that holds the hole; with Enhanced graphics' smooth camera it is the zoom that just holds it, eased to.
use crate::app::{far_zoom, App, ZOOM_UNIT};

/// How much of the screen the hole may take: a margin round it, more at the top and the foot for the gauges and the dock.
const FILL_X: f32 = 0.8;
const FILL_Y: f32 = 0.62;
/// The part of the 800 x 600 screen over the dock's panel (y 0 to 452), whose middle the hole is framed in.
const CLEAR_H: f32 = 452.0 / 600.0;

/// The ground point (x, z) whose place on the screen, through the view's rotation `m` (fx::view_rotation), is (ex, ey)
/// in world units about the screen's middle: the camera that puts that place there.
fn camera_for(m: &[f32; 16], ex: f64, ey: f64) -> (f64, f64) {
    let (a, b, c, d) = (m[0] as f64, m[8] as f64, m[1] as f64, m[9] as f64);
    let det = a * d - b * c;
    if det.abs() < 1e-9 {
        return (0.0, 0.0);
    }
    ((ex * d - b * ey) / det, (a * ey - c * ex) / det)
}

/// The zoom that holds a span of `w` x `h` world units on a `dw` x `dh` drawable at view scale `vs`, between the farthest
/// zoom and the closest of the original's levels; with `levels` (Classic) the closest whole level that still holds it.
pub fn zoom_for(w: f32, h: f32, dw: f32, dh: f32, vs: f32, far: f32, levels: bool) -> f32 {
    let upp = (w / (dw * FILL_X)).max(h / (dh * FILL_Y)).max(1e-6);
    let z = (2.0 / (upp * vs.max(1e-6))).clamp(far, ZOOM_UNIT * 4.0);
    if !levels {
        return z;
    }
    let mut l = ZOOM_UNIT * 4.0;
    while l > far * 1.001 && l > z * 1.001 {
        l /= 2.0;
    }
    l.max(far)
}

impl App {
    /// The points that make up hole `h` as the player sees it, in map units: the middles of its tees and pin tiles, and
    /// its yardage markers (the bends of its route).
    fn hole_points(&self, h: usize) -> Vec<(i32, i32)> {
        let Some(hr) = self.club.holes.get(h) else { return Vec::new() };
        let mid = |(a, b): (i32, i32)| (a * 1024 + 512, b * 1024 + 512);
        let mut t = vec![mid(hr.back), mid(hr.pin)];
        if hr.fwd != (0, 0) {
            t.push(mid(hr.fwd));
        }
        t.extend(hr.front_tee.map(mid));
        t.extend(hr.middle_tee.map(mid));
        t.extend(hr.markers.iter().copied().filter(|&(x, y)| x >= 0 && y >= 0));
        t
    }

    /// Frames open hole `h` on the screen (Show hole). Returns false for a hole not open.
    pub fn fit_hole(&mut self, h: usize) -> bool {
        if !(1..=18).contains(&h) || self.club.holes[h].par == 0 {
            return false;
        }
        // the points' places on the screen in world units with the camera at the origin, as render_world draws them
        let m = crate::fx::view_rotation(self);
        let sp: Vec<(f64, f64)> = self
            .hole_points(h)
            .iter()
            .map(|&(x, y)| {
                let (wx, wz) = self.units_to_world(x, y);
                let ht = self.terrain.height_at(wx, wz);
                ((m[0] * wx + m[4] * ht + m[8] * wz) as f64, (m[1] * wx + m[5] * ht + m[9] * wz) as f64)
            })
            .collect();
        let (x0, x1) = sp.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (y0, y1) = sp.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        // a tile's own size round the points, so a tee and a green are not cut at the edge
        let pad = sg_core::terrain::TILE_SIZE as f64 * 0.75;
        let (w, hh) = ((x1 - x0 + 2.0 * pad) as f32, (y1 - y0 + 2.0 * pad) as f32);
        let vs = crate::ui::view_scale(self.draw_w, self.draw_h);
        let zoom = zoom_for(w, hh, self.draw_w, self.draw_h * CLEAR_H, vs, far_zoom(), !crate::enhanced::smooth_camera());
        // the middle of the hole in the middle of the screen over the dock: the screen's middle a little below it
        let upp = 2.0 / (zoom * vs) as f64;
        let below = (1.0 - CLEAR_H as f64) * 0.5 * self.draw_h as f64 * upp;
        let (cx, cz) = camera_for(&m, (x0 + x1) * 0.5, (y0 + y1) * 0.5 - below);
        self.cam_x = cx as f32;
        self.cam_z = cz as f32;
        self.zoom = zoom;
        self.xs.follow = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_puts_the_point_where_asked() {
        let m = crate::gfx::Mat4::rotate(40.5, 1.0, 0.0, 0.0).mul(&crate::gfx::Mat4::rotate(135.0, 0.0, 1.0, 0.0)).0;
        for &(x, z) in &[(0.0f32, 0.0f32), (100.0, -40.0), (-7.5, 333.0)] {
            let (ex, ey) = ((m[0] * x + m[8] * z) as f64, (m[1] * x + m[9] * z) as f64);
            let (cx, cz) = camera_for(&m, ex, ey);
            assert!((cx - x as f64).abs() < 1e-3 && (cz - z as f64).abs() < 1e-3);
        }
    }

    #[test]
    fn a_longer_hole_gets_a_farther_zoom() {
        let far = ZOOM_UNIT;
        let near = zoom_for(200.0, 100.0, 800.0, 452.0, 1.0, far, false);
        let long = zoom_for(2000.0, 800.0, 800.0, 452.0, 1.0, far, false);
        assert!(long < near);
        // Classic: one of the original's levels, and one that still holds the hole
        let z = zoom_for(900.0, 300.0, 800.0, 452.0, 1.0, far, true);
        assert!([ZOOM_UNIT, ZOOM_UNIT * 2.0, ZOOM_UNIT * 4.0].contains(&z));
        assert!(z <= zoom_for(900.0, 300.0, 800.0, 452.0, 1.0, far, false) * 1.001);
        // never past the farthest zoom
        assert_eq!(zoom_for(1e7, 1e7, 800.0, 452.0, 1.0, far, true), far);
    }
}
