//! The port's own camera helpers (docs/PLAYING.md, "Camera bookmarks"): none of them in the original, and nothing changes
//! until the player uses them.
//!
//! * Camera bookmarks: four views of the course (its centre, zoom and turn) kept with the game. Shift+F1 to Shift+F4 (keys
//!   the original leaves unused; remappable in Preferences > Keyboard controls) go back to a view, Ctrl with the same key
//!   keeps the view there. They are saved with the game (the course's own places), and only once one has been kept: a
//!   game without any saves exactly as before.
//! * Zoom towards the pointer (Preferences > OpenSGolf extras, off by default): the mouse wheel and a two-finger pinch keep
//!   the ground under the pointer (or between the fingers) where it is on the screen, at the original's three levels and
//!   with Enhanced graphics' smooth zoom alike (the original zooms about the middle of the screen).
//! * More zoom and tilt (Preferences > OpenSGolf extras, off by default; on with Original + helpers): the zoom goes on past
//!   the original's three levels, out to half its farthest and in to eight times its closest, and the
//!   camera tilts from [`TILT_RANGE`]'s low angle to nearly straight down (End and PageDown, Ctrl with the wheel). The
//!   camera starts at the original's view each run and Shift+Z (`reset_camera`) puts it back; with the extra off the
//!   original's range and angle come back at once. Only the camera moves: the rules keyed to the zoom read the nearest of
//!   the original's levels (`App::exe_zoom`), and the art is the original's, drawn at the angle it was made for at 0.
use crate::app::*;
use serde::{Deserialize, Serialize};
use sg_core::lang::trf;

/// The camera bookmarks.
pub const SLOTS: usize = 4;
/// Their actions in controls.rs, in slot order.
pub const ACTION_IDS: [&str; SLOTS] = ["view_1", "view_2", "view_3", "view_4"];

/// A kept view: the world point in the middle of the screen, the zoom and the turn (degrees, a multiple of 90).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub x: f32,
    pub z: f32,
    pub zoom: f32,
    pub rot: f32,
    /// The tilt (degrees added to the original's angle, More zoom and tilt); 0 in views kept before it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tilt: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// The tilt More zoom and tilt allows, in degrees added to the original's angle of about 40 degrees down (app::pitch_for):
/// from a lower, flatter look across the course to nearly straight down.
pub const TILT_RANGE: (f32, f32) = (-25.0, 30.0);
/// Degrees one press of a tilt key turns the camera; a wheel notch with Ctrl turns half of it.
pub const TILT_STEP: f32 = 5.0;

/// How a sprite drawn for the original's angle is reshaped for the tilt (More zoom and tilt). Tilting from the original's
/// angle `p0` to `p` stretches the course's ground up the screen by sin p / sin p0 ([`Warp::ground`]) and heights by
/// cos p / cos p0 ([`Warp::height`]). A picture is flat, so how much of each of its parts is ground and how much height is
/// read back from its outline ([`shape`]): in each column, its lowest pixel is the front of its footprint on the ground,
/// the footprint's back edge follows from its corners (a footprint is a parallelogram on the screen, so its back corner is
/// its left and right corners less its front one), and its highest pixel is the top of what stands over that back edge.
/// A column is then redrawn from its front base, which moves with the course, through its footprint's depth (stretched as
/// ground) to its top, whose height above the back edge is stretched as a height. So a building keeps to its tiles, its
/// roof spreads out seen from above and its walls stand tall seen from low down. A tree or a person stands on its anchor,
/// and a flat base or a shadow is all ground. At no tilt both stretches are 1 and the picture is the original's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    /// The ground's and the heights' stretch up the screen.
    pub ground: f32,
    pub height: f32,
}

/// One column of a picture's shape ([`shape`]), up the screen from the anchor at the original's angle: where its front
/// base, its footprint's back edge and its top are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Column {
    pub x: f32,
    pub base: f32,
    pub back: f32,
    pub top: f32,
}

/// Where a tree's crown starts, up its picture from its foot.
pub const CROWN_LOW: f32 = 0.35;

/// Columns a building's picture is read and drawn in.
pub const BASE_COLUMNS: usize = 32;

impl Warp {
    pub const NONE: Warp = Warp { ground: 1.0, height: 1.0 };

    /// A point drawn `cy` above the anchor (world units on the screen, up positive) at the original's angle, in a column of
    /// shape `c` (all zero: standing on the anchor), tilted (`ground_only`: a flat base or a shadow).
    pub fn y(&self, cy: f32, c: Column, ground_only: bool) -> f32 {
        if ground_only {
            return cy * self.ground;
        }
        let span = c.top - c.base;
        let k = if span > 1e-3 { ((c.back - c.base) * self.ground + (c.top - c.back) * self.height) / span } else { self.height };
        c.base * self.ground + (cy - c.base) * k
    }

    /// A tree's point `cy` above its foot, its picture `top` high: the trunk is a height, but the crown is round and
    /// looks the same size from any angle, so it keeps its size and rides up or down on the trunk.
    pub fn y_tree(&self, cy: f32, top: f32) -> f32 {
        if cy <= 0.0 || top <= 1e-3 {
            return cy * self.height;
        }
        let low = top * CROWN_LOW;
        let mid = (low + top) * 0.5;
        // the crown's new foot: its middle moved like a height, its size kept (never under the tree's foot)
        let low2 = (mid * self.height - (top - low) * 0.5).max(0.0);
        if cy >= low {
            low2 + (cy - low)
        } else {
            cy * low2 / low
        }
    }

    /// A person's point `cy` above their feet: people are drawn upright from the original's angle, so they shrink
    /// with the tilt only half as much as a wall would (fully, they read as flat cut-outs from above).
    pub fn y_figure(&self, cy: f32) -> f32 {
        cy * self.height.sqrt()
    }

    pub fn is_none(&self) -> bool {
        self.ground == 1.0 && self.height == 1.0
    }
}

/// The lower convex hull of points sorted by x, then read back at every point's x: the straight-sided base under them.
pub fn lower_hull(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if pts.is_empty() {
        return Vec::new();
    }
    let mut hull: Vec<(f32, f32)> = Vec::new();
    for &p in pts {
        while hull.len() >= 2 {
            let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
            // b is above the line from a to p: not on the lower hull
            if (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) <= 0.0 {
                hull.pop();
            } else {
                break;
            }
        }
        hull.push(p);
    }
    pts.iter().map(|&(x, _)| (x, along(&hull, x))).collect()
}

/// A polyline sorted by x read at `x` (its ends held beyond them).
fn along(line: &[(f32, f32)], x: f32) -> f32 {
    let k = line.iter().position(|h| h.0 >= x).unwrap_or(line.len() - 1);
    if k == 0 {
        return line[0].1;
    }
    let (a, b) = (line[k - 1], line[k]);
    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0).max(1e-6)
}

/// The lowest and highest opaque rows (pixels from the top: the row under the lowest, the highest's own) at each of
/// `BASE_COLUMNS + 1` points across a picture, from its RGBA pixels; `None` for an empty column. Each point looks a little
/// either side, so a thin gap between columns does not count as empty.
pub fn outline_rows(w: usize, h: usize, px: &[u8]) -> Vec<Option<(f32, f32)>> {
    let reach = (w / BASE_COLUMNS / 2).max(1);
    let opaque = |x0: usize, x1: usize, y: usize| (x0..=x1).any(|x| px.get((y * w + x) * 4 + 3).is_some_and(|&a| a > 32));
    (0..=BASE_COLUMNS)
        .map(|k| {
            let cx = (k * w.saturating_sub(1)) / BASE_COLUMNS;
            let (x0, x1) = (cx.saturating_sub(reach), (cx + reach).min(w.saturating_sub(1)));
            let low = (0..h).rev().find(|&y| opaque(x0, x1, y))?;
            let high = (0..h).find(|&y| opaque(x0, x1, y))?;
            Some(((low + 1) as f32, high as f32))
        })
        .collect()
}

/// A picture's shape from its outline (`outline_rows`, with the anchor at `ax`, `ay` pixels): per column, its base (the
/// lower hull of the lowest pixels: straight walls meeting at corners, so a porch or a bush does not bend it), its
/// footprint's back edge (from the base's left, front and right corners) and its top (the upper hull of the highest
/// pixels), in pixels up from the anchor. Empty columns past the picture's sides take its last ones.
pub fn shape(rows: &[Option<(f32, f32)>], w: usize, ax: f32, ay: f32) -> Vec<Column> {
    let xs: Vec<f32> = (0..rows.len()).map(|k| (k * w.saturating_sub(1) / BASE_COLUMNS) as f32 - ax).collect();
    let full: Vec<usize> = (0..rows.len()).filter(|&k| rows[k].is_some()).collect();
    if full.is_empty() {
        return Vec::new();
    }
    let lows: Vec<(f32, f32)> = full.iter().map(|&k| (xs[k], ay - rows[k].unwrap().0)).collect();
    let highs: Vec<(f32, f32)> = full.iter().map(|&k| (xs[k], -(ay - rows[k].unwrap().1))).collect();
    let base = lower_hull(&lows);
    let top: Vec<(f32, f32)> = lower_hull(&highs).into_iter().map(|(x, y)| (x, -y)).collect();
    // the footprint's corners: left and right where the base ends, front where it is lowest; the back corner opposite
    let (l, r) = (base[0], base[base.len() - 1]);
    let f = base.iter().copied().fold(l, |m, p| if p.1 < m.1 { p } else { m });
    let bk = (l.0 + r.0 - f.0, l.1 + r.1 - f.1);
    let back_line = [l, bk, r];
    xs.iter()
        .map(|&x| {
            let (b, t) = (along(&base, x), along(&top, x));
            let k = along(&back_line, x).clamp(b, t.max(b));
            Column { x, base: b, back: k, top: t.max(b) }
        })
        .collect()
}

/// Whether the camera may go past the original's: the More zoom and tilt extra, or photo mode (its own free camera).
fn free(app: &App) -> bool {
    crate::extras::camera_more() || app.photo.is_some()
}

/// The camera to the ground point `g` kept at its place on the screen as the zoom goes from `from` to `to`: the screen is
/// scaled about that point, so the middle moves along the line to it by the zoom's ratio.
pub fn anchored(cam: (f32, f32), g: (f32, f32), from: f32, to: f32) -> (f32, f32) {
    if to <= 0.0 || from <= 0.0 {
        return cam;
    }
    let k = from / to;
    (g.0 + (cam.0 - g.0) * k, g.1 + (cam.1 - g.1) * k)
}

impl App {
    /// The zoom the camera may take (lowest, highest): the original's farthest and closest levels (a larger course size
    /// zooms further out, app::far_level), or with More zoom and tilt and in photo mode half the farthest to eight times
    /// the closest.
    pub fn zoom_range(&self) -> (f32, f32) {
        if free(self) {
            (far_zoom() * 0.5, ZOOM_UNIT * 32.0)
        } else {
            (far_zoom(), ZOOM_UNIT * 4.0)
        }
    }

    /// The camera's tilt as drawn (degrees added to the original's angle): 0 unless the camera may tilt, eased with the
    /// smooth camera (enhanced.rs camera_frame).
    pub fn view_tilt(&self) -> f32 {
        if !free(self) {
            return 0.0;
        }
        if crate::enhanced::smooth_camera() || self.photo.is_some() {
            self.enh.view_tilt
        } else {
            self.tilt
        }
    }

    /// The camera's angle down from level (degrees): the original's for the window's size, tilted by More zoom and tilt.
    pub fn pitch(&self) -> f64 {
        pitch_for(self.draw_w, self.draw_h) + self.view_tilt() as f64
    }

    /// How sprites are reshaped for the tilt as drawn ([`Warp`]); none at the original's angle.
    pub fn sprite_warp(&self) -> Warp {
        let t = self.view_tilt();
        if t == 0.0 {
            return Warp::NONE;
        }
        let p0 = (pitch_for(self.draw_w, self.draw_h) as f32).to_radians();
        let p = p0 + t.to_radians();
        Warp { ground: p.sin() / p0.sin(), height: p.cos() / p0.cos() }
    }

    /// A building's shape across its picture, for the tilt ([`Warp`], [`shape`]), in sprite pixels from its anchor. Kept
    /// per frame.
    pub fn base_edges(&mut self, si: usize, view: i32, frame: i32) -> Vec<Column> {
        let sp = &self.sprites[si].s;
        let i = sp.frame_index(view, frame);
        if let Some(e) = self.enh.base_edges.get(&(si, i)) {
            return e.clone();
        }
        let (ax, ay) = (sp.anchor_x as f32, sp.anchor_y as f32);
        let img = sp.frame(i);
        let (w, h) = (img.w as usize, img.h as usize);
        let e = shape(&outline_rows(w, h, &img.px), w, ax, ay);
        self.enh.base_edges.insert((si, i), e.clone());
        e
    }

    /// The course's light in eye space (Terrain.dll's light 0, fixed in eye space at the original's angle), turned with the
    /// tilt so that it stays where it was over the course: tilting the camera moves the eye, not the sun.
    pub fn light_eye(&self) -> [f32; 3] {
        let l = sg_core::terrain::light_direction();
        let t = self.view_tilt();
        if t == 0.0 {
            return l;
        }
        let (s, c) = t.to_radians().sin_cos();
        [l[0], c * l[1] - s * l[2], s * l[1] + c * l[2]]
    }

    /// The tilt keys and Ctrl with the wheel (More zoom and tilt, photo mode): the camera tilts by `degrees`, positive
    /// towards straight down, within [`TILT_RANGE`]. Returns whether it may tilt at all.
    pub fn tilt_by(&mut self, degrees: f32) -> bool {
        if !free(self) {
            return false;
        }
        self.tilt = (self.tilt + degrees).clamp(TILT_RANGE.0, TILT_RANGE.1);
        true
    }

    /// The camera back to the original's view (Shift+Z with More zoom and tilt): no tilt and a zoom inside the original's
    /// range, at the nearest of its levels.
    pub fn reset_camera(&mut self) {
        self.tilt = 0.0;
        let far = far_level() as f32;
        self.zoom = ZOOM_UNIT * ((self.zoom / ZOOM_UNIT).log2().round().clamp(far, 2.0)).exp2();
    }

    /// Once a frame: a camera past the original's while it may not be (the extra just turned off, photo mode closed) comes
    /// back inside it.
    pub fn keep_camera_in_range(&mut self) {
        if free(self) {
            return;
        }
        self.tilt = 0.0;
        let (lo, hi) = self.zoom_range();
        if self.zoom < lo * 0.999 || self.zoom > hi * 1.001 {
            self.zoom = self.zoom.clamp(lo, hi);
        }
    }

    /// The point of the ground plane (height 0) under a drawable pixel, with the camera as last drawn: the course under
    /// the pixel lies on the same line of sight, so keeping this point keeps it.
    pub fn plane_point(&self, mx: f32, my: f32) -> Option<(f32, f32)> {
        let m: Vec<f64> = self.mv.iter().map(|&v| v as f64).collect();
        let upp = self.upp as f64;
        if upp <= 0.0 {
            return None;
        }
        let ex = (mx as f64 - self.draw_w as f64 * 0.5) * upp;
        let ey = (self.draw_h as f64 * 0.5 - my as f64) * upp;
        let v = [ex - m[12], ey - m[13], -m[14]];
        let mut o = [0f64; 3];
        let mut d = [0f64; 3];
        for j in 0..3 {
            o[j] = m[4 * j] * v[0] + m[4 * j + 1] * v[1] + m[4 * j + 2] * v[2];
            d[j] = -m[4 * j + 2];
        }
        if d[1].abs() < 1e-6 {
            return None;
        }
        let s = -o[1] / d[1];
        Some(((o[0] + d[0] * s) as f32, (o[2] + d[2] * s) as f32))
    }

    /// A zoom made by `change` (the wheel, a pinch) about drawable pixel `at`: with Zoom towards the pointer on, the ground
    /// under it stays there (with the smooth camera frame by frame as the zoom eases, enhanced.rs camera_frame); else the
    /// change alone, as the original.
    pub fn zoom_about(&mut self, at: (f32, f32), change: impl FnOnce(&mut App)) {
        if !crate::extras::zoom_pointer() {
            return change(self);
        }
        if crate::enhanced::smooth_camera() || self.photo.is_some() {
            self.enh.zoom_anchor = Some(at);
            return change(self);
        }
        let before = self.zoom;
        let g = self.plane_point(at.0, at.1);
        change(self);
        if let Some(g) = g.filter(|_| self.zoom != before) {
            (self.cam_x, self.cam_z) = anchored((self.cam_x, self.cam_z), g, before, self.zoom);
        }
    }

    /// The smooth camera's shown zoom went from `from` to `to` this frame: the anchor's ground keeps its place.
    pub fn zoom_anchor_frame(&mut self, from: f32, to: f32) {
        let Some(at) = self.enh.zoom_anchor else { return };
        if from != to {
            if let Some(g) = self.plane_point(at.0, at.1) {
                (self.cam_x, self.cam_z) = anchored((self.cam_x, self.cam_z), g, from, to);
            }
        }
        if to == self.zoom {
            self.enh.zoom_anchor = None;
        }
    }

    /// Ctrl with a bookmark's key: the view kept in slot `i`.
    pub fn keep_view(&mut self, i: usize) {
        let v = View { x: self.cam_x, z: self.cam_z, zoom: self.zoom, rot: self.rot.rem_euclid(360.0), tilt: self.tilt };
        let views = &mut self.xs.kept.views;
        if views.len() < SLOTS {
            views.resize(SLOTS, None);
        }
        views[i] = Some(v);
        let key = crate::controls::key_label(ACTION_IDS[i]);
        self.show_toast(&trf("STR_TOAST_VIEW_KEPT", &[("SLOT", &(i + 1)), ("KEY", &key)]));
    }

    /// A bookmark's key: back to the view kept in slot `i` (its zoom at the nearest of the original's levels without the
    /// smooth camera).
    pub fn recall_view(&mut self, i: usize) {
        let Some(v) = self.xs.kept.views.get(i).copied().flatten() else {
            let key = crate::controls::key_label(ACTION_IDS[i]);
            self.show_toast(&trf("STR_TOAST_VIEW_NONE", &[("SLOT", &(i + 1)), ("KEY", &key)]));
            return;
        };
        self.xs.follow = None;
        self.cam_x = v.x;
        self.cam_z = v.z;
        let (lo, hi) = self.zoom_range();
        self.zoom = if crate::enhanced::smooth_camera() {
            v.zoom.clamp(lo, hi)
        } else {
            ZOOM_UNIT * ((v.zoom / ZOOM_UNIT).log2().round().clamp((lo / ZOOM_UNIT).log2(), (hi / ZOOM_UNIT).log2())).exp2()
        };
        self.tilt = 0.0;
        self.tilt_by(v.tilt);
        let quarters = (((v.rot - self.rot) / 90.0).round() as i32).rem_euclid(4);
        if quarters != 0 {
            self.rotate_view(if quarters == 3 { -1 } else { quarters });
        }
        self.show_toast(&trf("STR_TOAST_VIEW", &[("SLOT", &(i + 1))]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sets the interface extras for a test and gives back the ones before.
    fn with_camera_more(on: bool) -> crate::extras::Settings {
        let was = crate::extras::settings();
        crate::extras::set(crate::extras::Settings { camera_more: on, ..was });
        was
    }

    #[test]
    fn more_zoom_and_tilt_is_off_by_default_and_on_with_the_helpers() {
        assert!(!crate::extras::Settings::ALL_OFF.camera_more && crate::extras::Settings::HELPERS.camera_more);
        let mut a = App::with_clock(std::path::PathBuf::new(), 4242);
        let was = with_camera_more(false);
        // off: the original's three levels and one angle
        assert_eq!(a.zoom_range(), (far_zoom(), ZOOM_UNIT * 4.0));
        assert!(!a.tilt_by(TILT_STEP));
        assert_eq!((a.tilt, a.view_tilt()), (0.0, 0.0));
        a.zoom = ZOOM_UNIT * 4.0;
        a.zoom_step(true);
        assert_eq!(a.zoom, ZOOM_UNIT * 4.0, "no closer than the original's closest");
        // on: three more levels in, one more out, and the tilt within its range
        with_camera_more(true);
        for _ in 0..4 {
            a.zoom_step(true);
        }
        assert_eq!(a.zoom, ZOOM_UNIT * 32.0);
        for _ in 0..20 {
            a.tilt_by(TILT_STEP);
        }
        assert_eq!(a.tilt, TILT_RANGE.1);
        assert!(a.exe_zoom() <= 4.0, "the rules keyed to the zoom read the original's closest at most");
        let p = a.pitch();
        assert!((p - pitch_for(a.draw_w, a.draw_h) - TILT_RANGE.1 as f64).abs() < 1e-4 || crate::enhanced::smooth_camera());
        // the reset puts the original's view back
        a.reset_camera();
        assert_eq!((a.tilt, a.zoom), (0.0, ZOOM_UNIT * 4.0));
        // turned off with the camera past the original's: it comes back inside
        a.zoom = ZOOM_UNIT * 16.0;
        a.tilt = 20.0;
        with_camera_more(false);
        a.keep_camera_in_range();
        assert_eq!((a.tilt, a.zoom), (0.0, ZOOM_UNIT * 4.0));
        crate::extras::set(was);
    }

    #[test]
    fn the_light_stays_over_the_course_as_the_camera_tilts() {
        // the world's light is the same at any tilt: eye light turned back by the view's tilt
        let mut a = App::with_clock(std::path::PathBuf::new(), 4242);
        let was = with_camera_more(true);
        a.tilt = 25.0;
        a.enh.view_tilt = 25.0;
        let w0 = crate::fx::to_world(
            &crate::gfx::Mat4::rotate(pitch_for(a.draw_w, a.draw_h), 1.0, 0.0, 0.0).0,
            sg_core::terrain::light_direction(),
        );
        let w1 = crate::fx::to_world(&crate::gfx::Mat4::rotate(a.pitch(), 1.0, 0.0, 0.0).0, a.light_eye());
        for k in 0..3 {
            assert!((w0[k] - w1[k]).abs() < 1e-4, "{w0:?} {w1:?}");
        }
        crate::extras::set(was);
    }

    #[test]
    fn a_tree_keeps_its_round_crown_when_tilted() {
        let w = Warp { ground: 1.4, height: 0.5 };
        let (low, top) = (CROWN_LOW * 100.0, 100.0);
        // the crown keeps its size, lower down; the trunk takes the squash; the foot stays put
        assert!(((w.y_tree(top, top) - w.y_tree(low, top)) - (top - low)).abs() < 1e-3);
        assert!(w.y_tree(top, top) < top);
        assert_eq!(w.y_tree(0.0, top), 0.0);
        // from low down the crown rides up on a longer trunk
        let up = Warp { ground: 0.6, height: 1.2 };
        assert!(up.y_tree(low, top) > low);
        assert!(((up.y_tree(top, top) - up.y_tree(low, top)) - (top - low)).abs() < 1e-3);
        assert_eq!(Warp::NONE.y_tree(50.0, top), 50.0);
    }

    #[test]
    fn a_picture_is_read_as_a_box_on_its_footprint() {
        // a V-shaped base with a bump up in it (a porch drawn higher): the hull keeps the V
        let pts = [(0.0, -10.0), (10.0, -20.0), (20.0, -5.0), (30.0, -40.0), (40.0, -20.0), (50.0, -10.0)];
        let h = lower_hull(&pts);
        assert_eq!(h[3], (30.0, -40.0));
        assert!(h[2].1 < -5.0 && h[1].1 <= -20.0, "{h:?}");
        // a box drawn as a diamond footprint 64 wide and 32 deep with walls 20 high: its outline read back gives the
        // footprint's back corner over its front one
        let (w, ht, ax, ay) = (65usize, 60usize, 32.0f32, 40.0f32);
        let mut px = vec![0u8; w * ht * 4];
        for x in 0..w {
            let d = 16.0 * (1.0 - (x as f32 - ax).abs() / 32.0); // half the footprint's depth at this column
            let (lo, hi) = (ay + d, ay - d - 20.0);
            for y in hi.max(0.0) as usize..(lo as usize).min(ht) {
                px[(y * w + x) * 4 + 3] = 255;
            }
        }
        let c = shape(&outline_rows(w, ht, &px), w, ax, ay);
        let mid = c[BASE_COLUMNS / 2];
        assert!(mid.x.abs() < 1.0 && (mid.base + 16.0).abs() < 1.5 && (mid.back - 16.0).abs() < 2.0, "{mid:?}");
        assert!((mid.top - 36.0).abs() < 1.5, "{mid:?}");
        // tilted, the base stays on the ground, the footprint stretches as ground and the walls as heights
        let warp = Warp { ground: 1.5, height: 0.5 };
        assert!((warp.y(mid.base, mid, false) - mid.base * 1.5).abs() < 1e-3);
        let top = warp.y(mid.top, mid, false);
        let want = mid.base * 1.5 + (mid.back - mid.base) * 1.5 + (mid.top - mid.back) * 0.5;
        assert!((top - want).abs() < 1e-3);
        assert_eq!(warp.y(10.0, Column::default(), false), 5.0);
        assert_eq!(warp.y(10.0, mid, true), 15.0);
    }

    #[test]
    fn a_view_kept_without_a_tilt_saves_as_before() {
        let v = View { x: 1.0, z: 2.0, zoom: 0.45, rot: 0.0, tilt: 0.0 };
        assert!(!serde_json::to_string(&v).unwrap().contains("tilt"));
        let back: View = serde_json::from_str(r#"{"x":1.0,"z":2.0,"zoom":0.45,"rot":0.0}"#).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn the_anchor_keeps_its_place() {
        // zooming in twice about a point: the point stays, the middle closes in on it by half
        let c = anchored((0.0, 0.0), (100.0, 40.0), 1.0, 2.0);
        assert_eq!(c, (50.0, 20.0));
        // the screen offset of the point (its distance from the middle times the zoom) is the same before and after
        let off = |c: (f32, f32), z: f32| ((100.0 - c.0) * z, (40.0 - c.1) * z);
        assert_eq!(off((0.0, 0.0), 1.0), off(c, 2.0));
        // and back out again
        assert_eq!(anchored(c, (100.0, 40.0), 2.0, 1.0), (0.0, 0.0));
        // nothing for a zoom of nothing
        assert_eq!(anchored((3.0, 4.0), (1.0, 1.0), 0.0, 1.0), (3.0, 4.0));
    }

    #[test]
    fn views_round_trip_in_the_save() {
        let mut k = crate::extras::Kept::default();
        assert!(!serde_json::to_string(&k).unwrap().contains("views"));
        k.views = vec![None, Some(View { x: 1.5, z: -2.0, zoom: 0.45, rot: 90.0, tilt: 0.0 }), None, None];
        let back: crate::extras::Kept = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
        assert_eq!(back, k);
    }
}
