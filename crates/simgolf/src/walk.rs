//! The walk camera: a hidden test view of the course from a golfer's eye height, with a perspective projection, reached only
//! with the `--walk` command-line switch on the desktop (it is in no menu, setting or help text). F12 turns it on and off on
//! the course screen, Esc turns it off; W A S D or the arrows walk (Shift faster), a drag with a mouse button held turns
//! and looks up and down, and the eye follows the ground.
//!
//! The ground is the same terrain meshes the overhead view draws (App::batches, lit as the overhead camera lights them),
//! seen through `Mat4::perspective` and `Mat4::look_at` with depth testing. The objects, golfers and trees are the overhead
//! view's sprites, stood upright on their ground point as billboards facing the camera, in the view of the game's quarter
//! turn nearest to the walk's heading. Nothing here runs without the switch, and nothing in the game's state changes:
//! leaving puts the overhead camera back where it was.
//!
//! With the switch on a scripted still (`--png`), the still is taken from the walk camera; `SG_WALK_AT=TX,TY,YAW[,PITCH]`
//! (tiles and degrees) places it.
use crate::app::{App, Screen};
use crate::gfx::{Gfx, Mat4, Mode, Uniforms, Vert};
use crate::ui::{rgba, Screen as Ui, F_ARIAL10};
use crate::Stage;
use miniquad::{KeyCode, MouseButton};
use sg_core::terrain::{Terrain, TILE_SIZE};
use std::sync::atomic::{AtomicBool, Ordering};

/// World units per metre: a tile's edge is 100 units and 25 yards (docs/DECODE_PLAYCORE.md).
pub const METRE: f32 = TILE_SIZE / (25.0 * 0.9144);
/// The eye's height over the ground.
pub const EYE: f32 = 1.7 * METRE;
/// Walking speed (metres a second), and how many times that with Shift.
const SPEED: f32 = 5.0;
const FAST: f32 = 5.0;
/// Degrees turned per drawable pixel of a drag (divided by the window's dpi scale).
const LOOK: f32 = 0.25;
/// The steepest look up or down, degrees.
const MAX_PITCH: f32 = 85.0;
/// Vertical field of view, degrees.
const FOV: f32 = 60.0;
/// The near plane (world units).
const NEAR: f32 = 0.25 * METRE;
/// The sprites' size against the overhead view's: the art is drawn far larger than the ground it stands on (a golfer is
/// most of a tile tall), so at eye height it is shrunk towards its real size.
const SPRITE_SCALE: f32 = 0.3;
/// The key that turns the walk camera on and off (one the game leaves unused).
pub const KEY: KeyCode = KeyCode::F12;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// The `--walk` switch was given.
#[cfg_attr(any(target_arch = "wasm32", target_os = "android"), allow(dead_code))]
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Where the walk camera stands and looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    /// The ground point under the eye (world units).
    pub x: f32,
    pub z: f32,
    /// Heading in degrees: 0 looks towards world -Z, 90 towards +X (the overhead camera's heading is 45 plus its turn).
    pub yaw: f32,
    /// Degrees above level.
    pub pitch: f32,
}

impl Cam {
    /// Level ahead and to the right, on the ground plane.
    pub fn axes(&self) -> ([f32; 2], [f32; 2]) {
        let (s, c) = self.yaw.to_radians().sin_cos();
        ([s, -c], [c, s])
    }
    /// The way the eye looks.
    pub fn forward(&self) -> [f32; 3] {
        let ([fx, fz], _) = self.axes();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        [fx * cp, sp, fz * cp]
    }
    /// The eye, at its height over the ground here.
    pub fn eye(&self, t: &Terrain) -> [f32; 3] {
        [self.x, eye_height(t, self.x, self.z), self.z]
    }
    pub fn view(&self, t: &Terrain) -> Mat4 {
        let e = self.eye(t);
        let f = self.forward();
        Mat4::look_at(e, [e[0] + f[0], e[1] + f[1], e[2] + f[2]], [0.0, 1.0, 0.0])
    }
    /// The quarter turn of the overhead camera nearest to this heading: its sprites' views are drawn.
    pub fn quarter(&self) -> i32 {
        (((self.yaw - 45.0) / 90.0).round() as i32).rem_euclid(4)
    }
    /// Moves `ahead` and `right` world units along the ground, kept on the map.
    pub fn walk(&mut self, t: &Terrain, ahead: f32, right: f32) {
        let ([fx, fz], [rx, rz]) = self.axes();
        let (hw, hh) = (t.w as f32 * TILE_SIZE * 0.5, t.h as f32 * TILE_SIZE * 0.5);
        self.x = (self.x + fx * ahead + rx * right).clamp(-hw + 1.0, hw - 1.0);
        self.z = (self.z + fz * ahead + rz * right).clamp(-hh + 1.0, hh - 1.0);
    }
    /// Turns by a drag of (dx, dy) degrees.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw = (self.yaw + dx).rem_euclid(360.0);
        self.pitch = (self.pitch - dy).clamp(-MAX_PITCH, MAX_PITCH);
    }
}

/// The eye's height at a ground point: the terrain's there plus `EYE`.
pub fn eye_height(t: &Terrain, x: f32, z: f32) -> f32 {
    t.height_at(x, z) + EYE
}

/// The heading (Cam::yaw) from one ground point to another.
pub fn heading(from: (f32, f32), to: (f32, f32)) -> f32 {
    (to.0 - from.0).atan2(-(to.1 - from.1)).to_degrees().rem_euclid(360.0)
}

/// The world point at a tile's centre.
fn tile_centre(t: &Terrain, tx: i32, ty: i32) -> (f32, f32) {
    ((tx as f32 + 0.5) * TILE_SIZE - t.w as f32 * TILE_SIZE * 0.5, (ty as f32 + 0.5) * TILE_SIZE - t.h as f32 * TILE_SIZE * 0.5)
}

/// Where a walk starts: the first hole's tee looking at its green, else the map's centre with the overhead camera's heading.
pub fn start(app: &App) -> Cam {
    let t = &app.terrain;
    if let Ok(v) = std::env::var("SG_WALK_AT") {
        let v: Vec<f32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() >= 3 {
            let (x, z) = tile_centre(t, v[0] as i32, v[1] as i32);
            return Cam { x, z, yaw: v[2], pitch: v.get(3).copied().unwrap_or(0.0) };
        }
    }
    if let Some(h) = app.club.holes.get(1).filter(|h| h.par > 0) {
        let tee = if h.back != (0, 0) { h.back } else { h.fwd };
        if tee != (0, 0) {
            let (x, z) = tile_centre(t, tee.0, tee.1);
            let pin = tile_centre(t, h.pin.0, h.pin.1);
            let yaw = if pin != (x, z) { heading((x, z), pin) } else { 45.0 + app.rot };
            return Cam { x, z, yaw, pitch: 0.0 };
        }
    }
    Cam { x: 0.0, z: 0.0, yaw: (45.0 + app.rot).rem_euclid(360.0), pitch: 0.0 }
}

/// The walk camera's state in the main loop.
#[derive(Default)]
pub struct Walking {
    /// On, and where it is; None in the overhead view.
    pub cam: Option<Cam>,
    /// The overhead camera's place, zoom and turn when the walk began, given back when it ends.
    saved: (f32, f32, f32, f32),
    /// Walk keys held: ahead, back, left, right.
    held: [bool; 4],
    /// A mouse button is down: moving turns the camera.
    looking: bool,
    /// The clock of the last step.
    last: f64,
    /// A scripted still's walk has begun.
    still_begun: bool,
}

fn held_index(k: KeyCode) -> Option<usize> {
    match k {
        KeyCode::W | KeyCode::Up => Some(0),
        KeyCode::S | KeyCode::Down => Some(1),
        KeyCode::A | KeyCode::Left => Some(2),
        KeyCode::D | KeyCode::Right => Some(3),
        _ => None,
    }
}

impl Stage {
    /// Whether the walk camera is drawn this frame.
    pub(crate) fn walk_shown(&self) -> bool {
        self.walk.cam.is_some() && walk_screen(&self.app)
    }

    fn walk_begin(&mut self) {
        let a = &self.app;
        self.walk.saved = (a.cam_x, a.cam_z, a.zoom, a.rot);
        self.walk.cam = Some(start(a));
        self.walk.held = [false; 4];
        self.walk.looking = false;
        self.walk.last = self.app.clock;
    }

    fn walk_end(&mut self) {
        self.walk.cam = None;
        let a = &mut self.app;
        (a.cam_x, a.cam_z, a.zoom, a.rot) = self.walk.saved;
    }

    /// A key went down: true when the walk camera took it.
    pub(crate) fn walk_key(&mut self, k: KeyCode) -> bool {
        if !enabled() {
            return false;
        }
        if self.walk_shown() {
            if k == KEY || k == KeyCode::Escape {
                self.walk_end();
            } else if let Some(i) = held_index(k) {
                self.walk.held[i] = true;
            }
            return true;
        }
        if k == KEY && walk_screen(&self.app) {
            self.walk_begin();
            return true;
        }
        false
    }

    pub(crate) fn walk_key_up(&mut self, k: KeyCode) {
        if let Some(i) = held_index(k) {
            self.walk.held[i] = false;
        }
    }

    /// A mouse button: true when the walk camera took it.
    pub(crate) fn walk_button(&mut self, _button: MouseButton, down: bool) -> bool {
        if !self.walk_shown() {
            // the release of a button held when the walk ended is the walk's too
            return std::mem::take(&mut self.walk.looking) && !down;
        }
        self.walk.looking = down;
        true
    }

    /// The pointer moved by (dx, dy) drawable pixels: true when the walk camera took it.
    pub(crate) fn walk_motion(&mut self, dx: f32, dy: f32) -> bool {
        if !self.walk_shown() {
            return false;
        }
        if self.walk.looking {
            let k = LOOK / self.app.dpi.max(1.0);
            if let Some(c) = self.walk.cam.as_mut() {
                c.look(dx * k, dy * k);
            }
        }
        true
    }

    /// Once a frame: the walk's steps, and a scripted still's start in the walk camera.
    pub(crate) fn walk_update(&mut self, t: f64) {
        if enabled() && self.png_out.is_some() && !self.walk.still_begun && walk_screen(&self.app) {
            self.walk.still_begun = true;
            self.walk_begin();
        }
        let dt = (t - self.walk.last).clamp(0.0, 0.1) as f32;
        self.walk.last = t;
        if !self.walk_shown() {
            return;
        }
        let h = self.walk.held;
        let ahead = (h[0] as i32 - h[1] as i32) as f32;
        let right = (h[3] as i32 - h[2] as i32) as f32;
        if ahead == 0.0 && right == 0.0 {
            return;
        }
        let len = (ahead * ahead + right * right).sqrt();
        let step = SPEED * METRE * if self.shift { FAST } else { 1.0 } * dt / len;
        let terrain = &self.app.terrain;
        if let Some(c) = self.walk.cam.as_mut() {
            c.walk(terrain, ahead * step, right * step);
        }
    }
}

/// The screens the walk camera shows on: the course, with no photo mode, fly-over or notice.
pub(crate) fn walk_screen(app: &App) -> bool {
    app.screen == Screen::Play && app.photo.is_none() && app.showcase.is_none() && app.editor.is_none() && app.notice.is_none()
}

impl App {
    /// Draws the course from the walk camera, and the line of help.
    pub(crate) fn render_walk(&mut self, g: &mut Gfx, cam: &Cam) {
        let aspect = self.draw_w / self.draw_h.max(1.0);
        let far = (self.terrain.w.max(self.terrain.h) as f32 * TILE_SIZE * 1.5).max(8000.0);
        let proj = Mat4::perspective(FOV, aspect, NEAR, far);
        let view = cam.view(&self.terrain);
        self.walk_sky(g, &proj, cam);
        // the ground lit as the overhead camera at the nearest quarter turn lights it: the light is fixed in its eye space, so
        // the normals go through that camera's turn (the uniform `mv` is used for nothing else)
        let pitch = crate::app::pitch_for(self.draw_w, self.draw_h);
        let overhead = Mat4::rotate(pitch, 1.0, 0.0, 0.0).mul(&Mat4::rotate((45.0 + cam.quarter() as f32 * 90.0) as f64, 0.0, 1.0, 0.0));
        let mut lit = Uniforms::flat(&proj, &view);
        lit.mv = overhead.0;
        lit.lit = 1.0;
        lit.light_amb = self.light.ambient.map(|v| v * crate::render::BRIGHTNESS);
        lit.light_dif = self.light.diffuse.map(|v| v * crate::render::BRIGHTNESS);
        lit.light_spec = self.light.specular;
        lit.light_dir = sg_core::terrain::light_direction();
        let mut overlay = lit;
        overlay.alpha_ref = 0.04;
        for b in self.batches.iter().chain(&self.wall_batches) {
            g.draw_mesh(Mode::Solid, &b.mesh, b.tex, &lit);
        }
        for b in self.path_batches.iter().chain(&self.mud_batches) {
            g.draw_mesh(Mode::Overlay, &b.mesh, b.tex, &overlay);
        }
        let mut u = Uniforms::flat(&proj, &view);
        u.alpha_ref = 0.04;
        self.walk_sprites(g, &u, cam);
        g.flush();
        let s = Ui::new(self.draw_w, self.draw_h);
        let t = "Walk camera: W A S D or arrows walk, Shift faster, drag to look, F12 or Esc back";
        let w = F_ARIAL10.width(t) + 12.0;
        s.fill(g, 6.0, 580.0, w, 14.0, rgba(0.0, 0.0, 0.0, 0.45));
        s.put(g, F_ARIAL10, 12.0, 582.0, t, rgba(1.0, 1.0, 1.0, 0.85));
        g.flush();
    }

    /// A sky behind the ground: pale at the horizon, deeper overhead.
    fn walk_sky(&self, g: &mut Gfx, proj: &Mat4, cam: &Cam) {
        // where the horizon is on the screen (y in -1..1): level ahead, seen through the camera's pitch
        let k = proj.0[5];
        let horizon = (-(cam.pitch.to_radians().tan()) * k).clamp(-4.0, 4.0);
        let top = [0.36, 0.56, 0.86, 1.0];
        let low = [0.78, 0.86, 0.94, 1.0];
        let below = [0.62, 0.70, 0.62, 1.0];
        let u = Uniforms::flat(&Mat4::identity(), &Mat4::identity());
        let v = |x: f32, y: f32, c: [f32; 4]| Vert::new(x, y, 0.0, 0.5, 0.5).col(c);
        let h = horizon;
        g.quad(
            Mode::Flat,
            None,
            &u,
            [v(-1.0, 1.0f32.max(h + 1.0), top), v(1.0, 1.0f32.max(h + 1.0), top), v(1.0, h, low), v(-1.0, h, low)],
        );
        g.quad(
            Mode::Flat,
            None,
            &u,
            [v(-1.0, h, below), v(1.0, h, below), v(1.0, (-1.0f32).min(h - 1.0), below), v(-1.0, (-1.0f32).min(h - 1.0), below)],
        );
        g.flush();
    }

    /// The props' sprites as upright billboards on their ground points, the farthest first, hidden behind the ground.
    fn walk_sprites(&mut self, g: &mut Gfx, u: &Uniforms, cam: &Cam) {
        if !self.show_props || self.props.is_empty() {
            return;
        }
        self.update_props();
        let ([fx, fz], [rx, rz]) = cam.axes();
        let quarter = cam.quarter();
        let mut items: Vec<(f32, usize)> = Vec::new();
        for (i, p) in self.props.iter().enumerate() {
            if p.hidden || p.flat || p.body.is_none() {
                continue;
            }
            let ahead = (p.x - cam.x) * fx + (p.z - cam.z) * fz;
            if ahead > NEAR {
                items.push((ahead, i));
            }
        }
        items.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for &(_, i) in &items {
            let p = self.props[i].clone();
            let Some(si) = p.body else { continue };
            let view = self.prop_view(&p, si, quarter);
            let sp = &self.sprites[si].s;
            let (w, h, ax, ay) = (sp.w as f32, sp.h as f32, sp.anchor_x as f32, sp.anchor_y as f32);
            let st = match p.outfit {
                Some(o) => self.outfit_texture(g, si, view, p.frame, o).unwrap_or_else(|| self.sprite_texture(g, si, view, p.frame)),
                None => self.sprite_texture(g, si, view, p.frame),
            };
            let s = sg_core::sprites::SPRITE_UNITS_PER_PIXEL * p.scale * SPRITE_SCALE;
            let (l, r) = (-ax * s, (w - ax) * s);
            let (t, b) = (ay * s, -(h - ay) * s);
            let flip = if p.mirror { -1.0 } else { 1.0 };
            let y = self.terrain.height_at(p.x, p.z) + p.lift * SPRITE_SCALE;
            let c = |cx: f32, cy: f32, tu: f32, tv: f32| {
                let cx = cx * flip;
                let (tu, tv) = st.at(tu, tv);
                Vert::new(p.x + rx * cx, y + cy, p.z + rz * cx, tu, tv)
            };
            g.quad(Mode::Overlay, Some(st.id), u, [c(l, t, 0.0, 0.0), c(r, t, 1.0, 0.0), c(r, b, 1.0, 1.0), c(l, b, 0.0, 1.0)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hill() -> Terrain {
        let mut t = Terrain::demo_course(8, 8, 1);
        t.corner.iter_mut().for_each(|c| *c = 0);
        t.tile_corner.clear();
        // one corner raised four levels in the middle of the map
        t.corner[4 * 9 + 4] = 4;
        t
    }

    #[test]
    fn the_eye_follows_the_ground() {
        let t = hill();
        // the corner (4, 4) is the map's centre: 4 levels up there, flat ground far from it
        assert!((eye_height(&t, 0.0, 0.0) - (4.0 * sg_core::terrain::HEIGHT_STEP + EYE)).abs() < 1e-3);
        assert!((eye_height(&t, -350.0, -350.0) - EYE).abs() < 1e-3);
        // half way up the slope, half the height
        let mid = eye_height(&t, 50.0, 0.0);
        assert!((mid - (2.0 * sg_core::terrain::HEIGHT_STEP + EYE)).abs() < 1e-3);
        // a step towards the hill climbs it, and the eye stays 1.7 m over the ground
        let mut c = Cam { x: 100.0, z: 0.0, yaw: 270.0, pitch: 0.0 };
        let before = c.eye(&t)[1];
        c.walk(&t, 50.0, 0.0);
        assert!((c.x - 50.0).abs() < 1e-3 && c.z.abs() < 1e-3);
        assert!(c.eye(&t)[1] > before);
        assert!((c.eye(&t)[1] - t.height_at(c.x, c.z) - 1.7 * METRE).abs() < 1e-3);
        // and the map's edge stops the walk
        c.walk(&t, 10_000.0, 0.0);
        assert!(c.x > -400.0 && c.x < -398.0);
    }

    #[test]
    fn eye_height_is_a_golfers() {
        // 25 yards a tile: 1.7 m is about 7.4 units
        assert!((EYE - 7.437).abs() < 0.01);
    }

    #[test]
    fn the_point_looked_at_is_at_the_screen_centre() {
        let t = hill();
        for (yaw, pitch) in [(0.0, 0.0), (45.0, 10.0), (200.0, -20.0)] {
            let c = Cam { x: -200.0, z: 150.0, yaw, pitch };
            let e = c.eye(&t);
            let f = c.forward();
            let p = [e[0] + f[0] * 300.0, e[1] + f[1] * 300.0, e[2] + f[2] * 300.0];
            let m = Mat4::perspective(FOV, 1.6, NEAR, 8000.0).mul(&c.view(&t));
            let q = m.apply(p);
            assert!((q[0] / q[3]).abs() < 1e-4 && (q[1] / q[3]).abs() < 1e-4, "{yaw} {pitch}");
            // the walk's right is on the screen's right
            let ([_, _], [rx, rz]) = c.axes();
            let q = m.apply([p[0] + rx * 50.0, p[1], p[2] + rz * 50.0]);
            assert!(q[0] / q[3] > 0.1);
        }
    }

    #[test]
    fn headings_and_quarters() {
        // world -Z is heading 0, +X 90
        assert!((heading((0.0, 0.0), (0.0, -10.0)) - 0.0).abs() < 1e-4);
        assert!((heading((0.0, 0.0), (10.0, 0.0)) - 90.0).abs() < 1e-4);
        assert!((heading((0.0, 0.0), (0.0, 10.0)) - 180.0).abs() < 1e-4);
        // the overhead camera's own headings give its quarter turns
        for q in 0..4 {
            let c = Cam { x: 0.0, z: 0.0, yaw: 45.0 + q as f32 * 90.0, pitch: 0.0 };
            assert_eq!(c.quarter(), q);
        }
        assert_eq!(Cam { x: 0.0, z: 0.0, yaw: 10.0, pitch: 0.0 }.quarter(), 0);
        assert_eq!(Cam { x: 0.0, z: 0.0, yaw: 350.0, pitch: 0.0 }.quarter(), 3);
        // the overhead camera's heading at turn 0 matches the eye's view direction of the overhead matrix
        let m = Mat4::rotate(40.0, 1.0, 0.0, 0.0).mul(&Mat4::rotate(45.0, 0.0, 1.0, 0.0)).0;
        let (vx, vz) = (-m[2], -m[10]);
        let ([fx, fz], _) = Cam { x: 0.0, z: 0.0, yaw: 45.0, pitch: 0.0 }.axes();
        let l = (vx * vx + vz * vz).sqrt();
        assert!((vx / l - fx).abs() < 1e-4 && (vz / l - fz).abs() < 1e-4);
    }

    #[test]
    fn looking_is_held_short_of_straight_up() {
        let mut c = Cam { x: 0.0, z: 0.0, yaw: 350.0, pitch: 0.0 };
        c.look(20.0, -200.0);
        assert!((c.yaw - 10.0).abs() < 1e-4);
        assert_eq!(c.pitch, MAX_PITCH);
    }
}
