//! Enhanced graphics' tournament gallery (docs/ENHANCED.md, "Tournament atmosphere"), off by default: during a tournament
//! or a championship a gallery of spectators stands behind a rope round the last green, two rows deep round the green the
//! leading group is playing to and one row round the other greens in play. The rope runs a tile or so outside the green
//! and leaves the front of the green open towards the fairway, so nobody stands on the line of play, on the fairway or on
//! the green itself. The spectators watch a ball in the air on their hole (looking up), otherwise the green; some at the
//! front of the last green's gallery sit on the grass, and every so often a hole's gallery cheers.
//!
//! They are the game's own golfer sprites in the usual random colours and the rope is drawn here (stakes and a sagging
//! line), so nothing new is taken from the original. Drawing only: nothing in the simulation knows about them, golfers walk
//! through them as through scenery, and what varies comes from hashes of the hole, never from the game's random generator.
use crate::app::{App, Prop, GOLFER_BODIES};
use crate::gfx::Vert;
use miniquad::TextureId;
use sg_core::terrain::*;
use std::f32::consts::{PI, TAU};

/// The golfer clips spectators use (bases of app::GOLFER_CLIPS): standing, cheering, sitting on the grass, looking up.
const SQ: i32 = 0x0a;
const HAPPY: i32 = 0x1e;
const SIT: i32 = 0x46;
const LOOK_UP: i32 = 0xaa;
/// Seconds between a hole's cheers, and how long one lasts.
const CHEER_EVERY: f32 = 22.0;
const CHEER_FOR: f32 = 2.5;
/// Half the angle (radians) left open in front of a green, towards where the shots come from.
const OPEN: f32 = 0.30 * PI;
/// How far the rope stands outside the green's last tile, and the closest it comes to the pin (tiles).
const ROPE_OUT: f32 = 0.8;
const ROPE_MIN: f32 = 1.8;
/// The farthest a green reaches from its pin along a ray (tiles): beyond it a green is another hole's.
const GREEN_REACH: f32 = 4.5;
/// The rows: the first this far behind the rope, the others this far apart, spectators this far apart along a row (tiles).
const ROW_BACK: f32 = 0.3;
const ROW_GAP: f32 = 0.32;
const SPACING: f32 = 0.3;
/// The rope's stakes this far apart along it (tiles), its height and the stakes' (sprite pixels).
const STAKE_GAP: f32 = 0.55;
const ROPE_HIGH: f32 = 7.0;
const STAKE_HIGH: f32 = 9.0;

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

const DX: [f32; 8] = [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, -1.0, -1.0];
const DY: [f32; 8] = [-1.0, -1.0, 0.0, 1.0, 1.0, 1.0, 0.0, -1.0];

/// One spectator: where (in tiles), at which hole, which row (0 at the rope) and a number for the hashes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fan {
    pub x: f32,
    pub y: f32,
    pub hole: usize,
    pub row: u32,
    pub id: u32,
}

/// The galleries this tick: the spectators and the ropes (runs of stake places, in tiles).
#[derive(Clone, Debug, Default)]
pub(crate) struct Gallery {
    pub fans: Vec<Fan>,
    pub ropes: Vec<Vec<(f32, f32)>>,
}

/// What a gallery was worked out for: the game tick, the holes with one and their rows, the course's seed.
type GalleryKey = (u32, Vec<(usize, u32)>, u32);

thread_local! {
    /// The last gallery worked out and what for.
    static CACHE: std::cell::RefCell<Option<(GalleryKey, std::rc::Rc<Gallery>)>> = const { std::cell::RefCell::new(None) };
}

/// Ground a spectator may stand on: the rough and the grasses beside it (so not the land outside the club's, whose
/// tiles are sg_core::land::T_OUT).
fn standing(ty: u8) -> bool {
    matches!(ty, TT_ROUGH | TT_DEEP_ROUGH | TT_OVERGROWTH | TT_OVERGROWTH2)
}

fn green(ty: u8) -> bool {
    matches!(ty, TT_PUTTING_GREEN | TT_TRICKY_GREEN)
}

/// Ground the rope's stakes may go in: the rough and the like, not a fairway (the rope would cut across the play), the
/// green, water, sand, woods or a building.
fn ropeable(ty: u8) -> bool {
    standing(ty) || matches!(ty, TT_BRUSH | TT_MARSH | TT_FLOWER_BED)
}

impl App {
    fn crowd_type(&self, x: f32, y: f32) -> Option<u8> {
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        if tx < 0 || ty < 0 || tx >= self.terrain.w || ty >= self.terrain.h {
            return None;
        }
        Some(self.terrain.ty[self.terrain.tile_index(tx, ty)])
    }

    /// Whether a spectator can stand at (x, y) tiles: standing ground there and a little way round, so the rope keeps
    /// the gallery off the green and the fairway's edge.
    fn fan_spot(&self, x: f32, y: f32) -> bool {
        const R: f32 = 0.3;
        [(0.0, 0.0), (R, 0.0), (-R, 0.0), (0.0, R), (0.0, -R)].iter().all(|&(dx, dy)| self.crowd_type(x + dx, y + dy).is_some_and(standing))
    }

    /// The holes with a gallery and how big (3 rows: the last hole, 2: the leading group's, 1: the others in play), or
    /// nothing outside a tournament or a championship.
    fn gallery_holes(&self) -> Vec<(usize, u32)> {
        let tour = self.club.game & sg_core::golfer::game::TOURNAMENT != 0 || self.club.championship();
        if !tour {
            return Vec::new();
        }
        let last = (1..19).rev().find(|&h| self.club.holes.get(h).is_some_and(|r| r.par > 0));
        let Some(last) = last else { return Vec::new() };
        let mut play: Vec<usize> = self
            .club
            .g
            .iter()
            .take(sg_core::golfer::SLOTS)
            .filter(|g| (1..19).contains(&g.hole))
            .map(|g| g.hole as usize)
            .filter(|&h| self.club.holes[h].par > 0)
            .collect();
        play.sort_unstable();
        play.dedup();
        let lead = play.last().copied();
        let mut out = vec![(last, 3)];
        for h in play {
            if h != last {
                out.push((h, if Some(h) == lead { 2 } else { 1 }));
            }
        }
        out
    }

    /// The galleries this tick (worked out once a tick, for the spectators and the ropes both).
    pub(crate) fn gallery(&self) -> std::rc::Rc<Gallery> {
        let key: GalleryKey = (self.game_tick, self.gallery_holes(), self.fx_seed());
        if let Some(g) = CACHE.with(|c| c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, g)| g.clone())) {
            return g;
        }
        let mut out = Gallery::default();
        for &(h, rows) in &key.1 {
            self.green_gallery(h, rows, key.2, &mut out);
        }
        // test hook: SG_GALLERY_DUMP=1 prints each gallery when the holes with one change
        if std::env::var_os("SG_GALLERY_DUMP").is_some()
            && CACHE.with(|c| c.borrow().as_ref().map(|(k, _)| k.1.clone())) != Some(key.1.clone())
        {
            for &(h, rows) in &key.1 {
                let hr = &self.club.holes[h];
                let fans = out.fans.iter().filter(|f| f.hole == h).count();
                println!("gallery: hole {h} pin {:?} tee {:?}, {rows} rows: {fans} spectators", hr.pin, hr.back);
            }
            println!("gallery: {} ropes, {} stakes", out.ropes.len(), out.ropes.iter().map(|r| r.len()).sum::<usize>());
        }
        let g = std::rc::Rc::new(out);
        CACHE.with(|c| *c.borrow_mut() = Some((key, g.clone())));
        g
    }

    /// Where the shots come into hole `h`'s green from, as an angle round the pin: the middle of the fairway near the
    /// green, or the tee when there is none.
    fn approach(&self, h: usize) -> f32 {
        let hr = &self.club.holes[h];
        let (px, py) = (hr.pin.0 as f32 + 0.5, hr.pin.1 as f32 + 0.5);
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for dy in -7..=7 {
            for dx in -7..=7 {
                if dx * dx + dy * dy > 49 {
                    continue;
                }
                let (x, y) = (px + dx as f32, py + dy as f32);
                if self.crowd_type(x, y).is_some_and(|t| matches!(t, TT_FAIRWAY | TT_FIRM_FAIRWAY)) {
                    sx += dx as f32;
                    sy += dy as f32;
                    n += 1.0;
                }
            }
        }
        if n > 0.0 && (sx * sx + sy * sy) > 0.25 * n * n {
            return sy.atan2(sx);
        }
        let (tx, ty) = (hr.back.0 as f32 + 0.5 - px, hr.back.1 as f32 + 0.5 - py);
        ty.atan2(tx)
    }

    /// How far the rope stands from hole `h`'s pin along the angle `a`: a little outside the green's last tile that way.
    fn rope_reach(&self, px: f32, py: f32, a: f32) -> f32 {
        let (c, s) = (a.cos(), a.sin());
        // out along the green from the pin to its edge, over gaps of a step or two (a bunker's lip, a stray tile), not on to
        // another green beyond
        let mut last = 0.0f32;
        let mut r = 0.0;
        while r <= GREEN_REACH && r - last <= 0.5 {
            if self.crowd_type(px + c * r, py + s * r).is_some_and(green) {
                last = r;
            }
            r += 0.2;
        }
        (last + ROPE_OUT).max(ROPE_MIN)
    }

    /// Hole `h`'s gallery of `rows` rows round its green, open towards the fairway, with its rope.
    fn green_gallery(&self, h: usize, rows: u32, seed: u32, out: &mut Gallery) {
        let hr = &self.club.holes[h];
        let (px, py) = (hr.pin.0 as f32 + 0.5, hr.pin.1 as f32 + 0.5);
        let open = self.approach(h);
        let hs = seed ^ (h as u32).wrapping_mul(0x51ed);
        // the angles round the green the gallery takes: all but the opening, from one side of it round to the other
        let (a0, a1) = (open + OPEN, open + TAU - OPEN);
        // the rope: stakes round that arc, a run broken wherever a stake would stand on the green, in water or the like
        let reach = self.rope_reach(px, py, open + PI);
        let n = ((a1 - a0) * reach / STAKE_GAP).ceil().max(2.0) as usize;
        let mut run: Vec<(f32, f32)> = Vec::new();
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            let r = self.rope_reach(px, py, a);
            let p = (px + r * a.cos(), py + r * a.sin());
            // a stake much farther from the last than the gap breaks the run too (the green's edge jumps there)
            let near = run.last().is_none_or(|q: &(f32, f32)| (p.0 - q.0).hypot(p.1 - q.1) < 2.0 * STAKE_GAP);
            let ok = self.crowd_type(p.0, p.1).is_some_and(ropeable);
            if !ok || !near {
                // a run of fewer than four stakes is a stray bit of rope: left out
                if run.len() >= 4 {
                    out.ropes.push(std::mem::take(&mut run));
                }
                run.clear();
            }
            if ok {
                run.push(p);
            }
        }
        if run.len() >= 4 {
            out.ropes.push(run);
        }
        // the rows behind it, each staggered half a place from the one in front
        let mut id = 0u32;
        for row in 0..rows {
            let back = ROW_BACK + ROW_GAP * row as f32;
            let r_mid = reach + back;
            let n = ((a1 - a0) * r_mid / SPACING).floor().max(1.0) as usize;
            for i in 0..n {
                let k = hs ^ id.wrapping_mul(0x2545);
                id += 1;
                let stagger = if row % 2 == 1 { 0.5 } else { 0.0 };
                let a = a0 + (a1 - a0) * (i as f32 + stagger + 0.3 * (hash(k, 7) - 0.5)) / n as f32;
                // gaps in the crowd, fewer at the front and round the last green
                let thin = match rows {
                    3 => 0.08 + 0.10 * row as f32,
                    2 => 0.18 + 0.15 * row as f32,
                    _ => 0.40,
                };
                if hash(k, 8) < thin {
                    continue;
                }
                let r = self.rope_reach(px, py, a) + back + 0.12 * (hash(k, 9) - 0.5);
                let (x, y) = (px + r * a.cos(), py + r * a.sin());
                if !self.fan_spot(x, y) {
                    continue;
                }
                out.fans.push(Fan { x, y, hole: h, row, id: k });
            }
        }
    }

    /// The spectators to draw this tick (wild_ui.rs adds them with the other moving props).
    pub(crate) fn fx_spectators(&mut self) -> Vec<Prop> {
        let gallery = self.gallery();
        if gallery.fans.is_empty() {
            return Vec::new();
        }
        let t = self.fx.time;
        let tick = self.game_tick as i32;
        let seed = self.fx_seed();
        let last = gallery.fans.first().map(|f| f.hole);
        let bodies: Vec<usize> = (0..GOLFER_BODIES.len()).filter(|&b| !GOLFER_BODIES[b].is_empty()).collect();
        let mut out = Vec::new();
        for (i, f) in gallery.fans.iter().enumerate() {
            let h = f.hole;
            let pin = self.club.holes[h].pin;
            // what the gallery watches: a ball in the air on this hole, else a ball on the green, else the pin (in tiles)
            let ball_of = |air: bool| {
                self.club
                    .g
                    .iter()
                    .take(sg_core::golfer::SLOTS)
                    .find(|g| g.hole == h as i32 && g.bx != 0 && (g.bz > 0) == air)
                    .map(|g| (g.bx as f32 / 1024.0, g.by as f32 / 1024.0))
            };
            let flying = ball_of(true);
            let near = |b: (f32, f32)| (b.0 - pin.0 as f32 - 0.5).hypot(b.1 - pin.1 as f32 - 0.5) < GREEN_REACH;
            let look = flying.or(ball_of(false).filter(|&b| near(b))).unwrap_or((pin.0 as f32 + 0.5, pin.1 as f32 + 0.5));
            let hs = seed ^ (h as u32).wrapping_mul(0x51ed);
            let cheering = (t + hash(hs, 99) * CHEER_EVERY).rem_euclid(CHEER_EVERY) < CHEER_FOR;
            let k = f.id;
            let body = bodies[(hash(k, 1) * bodies.len() as f32) as usize % bodies.len()];
            let r = hash(k, 2);
            // sitting on the grass at the front of the last green's gallery; otherwise standing, looking up at a ball in
            // the air or cheering with the hole's gallery (the golfers' fidget swings a club: not for spectators)
            let sitting = Some(h) == last && f.row == 0 && hash(k, 4) < 0.35;
            let clip = if sitting {
                SIT
            } else if cheering && r < 0.75 {
                HAPPY
            } else if flying.is_some() && r < 0.8 {
                LOOK_UP
            } else {
                SQ
            };
            let (b, shadow) = match self.golfer_clip(clip + body as i32) {
                (Some(b), s) => (b, s),
                // a set without the clip: standing instead
                _ => match self.golfer_clip(SQ + body as i32) {
                    (Some(b), s) => (b, s),
                    _ => continue,
                },
            };
            let n = self.frames_of(Some(b));
            let frame = if clip == HAPPY { (tick / 2 + (hash(k, 3) * 40.0) as i32).rem_euclid(n.max(1)) } else { 0 };
            // facing what the gallery watches: the eight-view clips (the original's) take the exact heading, the four-view
            // ones (the free set's) the nearest quarter, whose views run the other way round from the compass
            let (vx, vy) = (look.0 - f.x, look.1 - f.y);
            let heading = vy.atan2(vx).to_degrees();
            let d0 = (0..8).max_by(|&a, &c| (DX[a] * vx + DY[a] * vy).total_cmp(&(DX[c] * vx + DY[c] * vy))).unwrap_or(0);
            let d = (d0 + 4) % 8;
            let id = (k >> 4) % 997;
            let female = body >= 5;
            let skin = (id % 4) as u8;
            let outfit = sg_core::bodies::Outfit {
                female,
                shirt: ((id * 3) % 10) as u8,
                pants: ((id * 7) % 10) as u8,
                skin,
                hat: if female { 9 } else { (id % 3) as u8 },
                hair: ((id * 5) % 6) as u8,
                alt_skin: skin,
            };
            let (x, z) = self.units_to_world((f.x * 1024.0) as i32, (f.y * 1024.0) as i32);
            out.push(Prop {
                x,
                z,
                body: Some(b),
                shadow,
                frame,
                heading,
                facing: -((d as i32 + 2 + (d as i32 & 1)) / 2),
                outfit: Some(outfit),
                track: 0x5200_0000 | (h as u32) << 16 | (i as u32 & 0xffff),
                ..Default::default()
            });
        }
        out
    }

    /// The galleries' ropes, in the sprites' depth order (render.rs draws them with the lamp posts): a white stake at each
    /// place and a rope sagging a little between them, tinted by the light as the sprites are.
    pub(crate) fn fx_rope_quads(&self, white: TextureId) -> Vec<(f32, Option<TextureId>, Vec<Vert>)> {
        let mut out = Vec::new();
        if !crate::fx::settings().crowds {
            return out;
        }
        let gallery = self.gallery();
        let m = self.mv;
        let (rx, ry, rz) = (m[0], m[4], m[8]);
        let (ux, uy, uz) = (m[1], m[5], m[9]);
        let s = sg_core::sprites::SPRITE_UNITS_PER_PIXEL;
        let world = |p: (f32, f32)| {
            let (x, z) = self.units_to_world_f(p.0 * 1024.0, p.1 * 1024.0);
            (x, self.terrain.height_at(x, z), z)
        };
        for run in &gallery.ropes {
            let pts: Vec<(f32, f32, f32)> = run.iter().map(|&p| world(p)).collect();
            for (j, &(x, y, z)) in pts.iter().enumerate() {
                let tint = self.fx_sprite_tint(x, z, false).unwrap_or([1.0; 4]);
                let c = |k: f32| [k * tint[0], k * tint[1], k * tint[2], 1.0];
                let at = |cx: f32, cy: f32, col: [f32; 4]| {
                    let (cx, cy) = (cx * s, cy * s);
                    Vert::new(x + rx * cx + ux * cy, y + ry * cx + uy * cy, z + rz * cx + uz * cy, 0.5, 0.5).col(col)
                };
                // the stake, lit from the left
                let mut v = Vec::new();
                let mut rect = |x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4]| {
                    let (a, b, cc, d) = (at(x0, y1, col), at(x1, y1, col), at(x1, y0, col), at(x0, y0, col));
                    v.extend_from_slice(&[a, b, cc, a, cc, d]);
                };
                rect(-0.6, 0.0, 0.0, STAKE_HIGH, c(0.95));
                rect(0.0, 0.0, 0.6, STAKE_HIGH, c(0.70));
                rect(-0.8, STAKE_HIGH - 0.8, 0.8, STAKE_HIGH, c(0.85));
                let key = m[2] * x + m[6] * y + m[10] * z;
                out.push((key, Some(white), v));
                // the rope to the next stake, in three pieces sagging a pixel and a half in the middle
                let Some(&(x2, y2, z2)) = pts.get(j + 1) else { continue };
                let mut v = Vec::new();
                let sag = |t: f32| ROPE_HIGH - 1.5 * 4.0 * t * (1.0 - t);
                let point = |t: f32| {
                    let (px, py, pz) = (x + (x2 - x) * t, y + (y2 - y) * t, z + (z2 - z) * t);
                    let h = sag(t) * s;
                    (px + ux * h, py + uy * h, pz + uz * h)
                };
                let half = 0.5 * s;
                let col = c(0.92);
                for q in 0..3 {
                    let (t0, t1) = (q as f32 / 3.0, (q + 1) as f32 / 3.0);
                    let (a, b) = (point(t0), point(t1));
                    let up = |p: (f32, f32, f32), k: f32| {
                        Vert::new(p.0 + ux * half * k, p.1 + uy * half * k, p.2 + uz * half * k, 0.5, 0.5).col(col)
                    };
                    let (a0, a1, b0, b1) = (up(a, -1.0), up(a, 1.0), up(b, -1.0), up(b, 1.0));
                    v.extend_from_slice(&[a0, b0, b1, a0, b1, a1]);
                }
                let (mx, my, mz) = ((x + x2) * 0.5, (y + y2) * 0.5, (z + z2) * 0.5);
                out.push((m[2] * mx + m[6] * my + m[10] * mz, Some(white), v));
            }
        }
        out
    }
}
