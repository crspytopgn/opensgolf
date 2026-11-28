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
//!   the original's three levels, out to half its farthest and in to four times its closest (photo mode's range), and the
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
pub const TILT_RANGE: (f32, f32) = (-15.0, 30.0);
/// Degrees one press of a tilt key turns the camera; a wheel notch with Ctrl turns half of it.
pub const TILT_STEP: f32 = 5.0;

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
    /// zooms further out, app::far_level), or with More zoom and tilt and in photo mode half the farthest to four times
    /// the closest.
    pub fn zoom_range(&self) -> (f32, f32) {
        if free(self) {
            (far_zoom() * 0.5, ZOOM_UNIT * 16.0)
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
        // on: two more levels in, one more out, and the tilt within its range
        with_camera_more(true);
        a.zoom_step(true);
        a.zoom_step(true);
        a.zoom_step(true);
        assert_eq!(a.zoom, ZOOM_UNIT * 16.0);
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
