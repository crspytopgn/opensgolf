//! Modelled buildings for the walk camera.
//!
//! A building's sprite is one picture from above. Stretched over a solid fitted to its outline (walk_objects.rs
//! `fit_proxy`), a shaped building such as the clubhouse, with its octagonal halls, awnings and garden, wears its roofs and
//! shrubs smeared down its walls. A building with a model here is drawn as that model instead: plain coloured geometry
//! of the project's own design, lit like the other solids (walk_gl.rs `mode::FLAT`). A building without one keeps its
//! fitted solid or its card.
//!
//! Models are built in metres about the footprint's centre, facing 0 (their front towards +z), and turned by the
//! object's facing.

use crate::gfx::Vert;
use std::f32::consts::{FRAC_PI_8, TAU};

const M: f32 = crate::walk::METRE;

/// A colour, opaque.
type Rgb = [f32; 3];

/// A part's colours: the theme's take on one building.
#[derive(Clone, Copy)]
struct Palette {
    wall: Rgb,
    trim: Rgb,
    roof: Rgb,
    deck: Rgb,
    rail: Rgb,
    glass: Rgb,
    frame: Rgb,
    door: Rgb,
    awning: Rgb,
    gold: Rgb,
    leaf: Rgb,
    leaf_dark: Rgb,
    trunk: Rgb,
    flower: Rgb,
    lamp: Rgb,
    stone: Rgb,
}

/// The clubhouse's colours by theme (app::THEMES: 0 Parkland, 1 Links, 2 Desert, 3 Tropical).
fn clubhouse_palette(theme: usize) -> Palette {
    let parkland = Palette {
        wall: [0.90, 0.85, 0.70],
        trim: [0.55, 0.16, 0.13],
        roof: [0.30, 0.26, 0.25],
        deck: [0.52, 0.40, 0.29],
        rail: [0.58, 0.17, 0.14],
        glass: [0.20, 0.26, 0.32],
        frame: [0.96, 0.95, 0.90],
        door: [0.42, 0.13, 0.10],
        awning: [0.78, 0.14, 0.12],
        gold: [0.86, 0.70, 0.30],
        leaf: [0.22, 0.42, 0.18],
        leaf_dark: [0.12, 0.27, 0.12],
        trunk: [0.36, 0.25, 0.16],
        flower: [0.82, 0.48, 0.78],
        lamp: [0.30, 0.66, 0.72],
        stone: [0.62, 0.58, 0.52],
    };
    match theme {
        1 => Palette {
            wall: [0.92, 0.91, 0.87],
            roof: [0.25, 0.30, 0.34],
            trim: [0.20, 0.30, 0.38],
            rail: [0.92, 0.92, 0.90],
            awning: [0.18, 0.32, 0.52],
            ..parkland
        },
        2 => Palette {
            wall: [0.86, 0.72, 0.55],
            roof: [0.66, 0.34, 0.22],
            trim: [0.48, 0.30, 0.20],
            deck: [0.70, 0.58, 0.44],
            rail: [0.48, 0.30, 0.20],
            awning: [0.20, 0.50, 0.50],
            leaf: [0.40, 0.48, 0.26],
            ..parkland
        },
        3 => Palette {
            wall: [0.95, 0.93, 0.86],
            roof: [0.62, 0.52, 0.34],
            trim: [0.18, 0.46, 0.44],
            rail: [0.95, 0.95, 0.92],
            awning: [0.18, 0.55, 0.52],
            leaf: [0.20, 0.50, 0.22],
            ..parkland
        },
        _ => parkland,
    }
}

/// Triangles in metres, built part by part.
#[derive(Default)]
struct Build {
    v: Vec<Vert>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The corners of a regular polygon of `n` sides about (cx, cz), radius `r` to its corners, its first corner at `turn`.
fn ring(cx: f32, cz: f32, r: f32, n: usize, turn: f32) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let a = turn + TAU * i as f32 / n as f32;
            (cx + r * a.cos(), cz + r * a.sin())
        })
        .collect()
}

/// An octagon with its sides square to the axes.
const OCT: f32 = FRAC_PI_8;

impl Build {
    /// A flat polygon, its normal turned away from `inside`. `shade` darkens its corners below `y` (a wall's foot).
    fn poly(&mut self, p: &[[f32; 3]], inside: [f32; 3], col: Rgb) {
        if p.len() < 3 {
            return;
        }
        let mut n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
        let c = p.iter().fold([0.0; 3], |a, q| [a[0] + q[0], a[1] + q[1], a[2] + q[2]]).map(|s| s / p.len() as f32);
        let out = sub(c, inside);
        if n[0] * out[0] + n[1] * out[1] + n[2] * out[2] < 0.0 {
            n = n.map(|q| -q);
        }
        let vert = |q: [f32; 3]| {
            let mut v = Vert::new(q[0], q[1], q[2], 0.0, 0.0).col([col[0], col[1], col[2], 1.0]);
            v.normal = n;
            v
        };
        for k in 1..p.len() - 1 {
            self.v.extend([vert(p[0]), vert(p[k]), vert(p[k + 1])]);
        }
    }

    /// Upright walls round a polygon, from `y0` up to `y1`.
    fn walls(&mut self, corners: &[(f32, f32)], y0: f32, y1: f32, col: Rgb) {
        let (cx, cz) = centre(corners);
        for i in 0..corners.len() {
            let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
            self.poly(&[[a.0, y0, a.1], [b.0, y0, b.1], [b.0, y1, b.1], [a.0, y1, a.1]], [cx, (y0 + y1) * 0.5, cz], col);
        }
    }

    /// A polygon's flat top (or floor) at `y`.
    fn cap(&mut self, corners: &[(f32, f32)], y: f32, up: bool, col: Rgb) {
        let (cx, cz) = centre(corners);
        let p: Vec<[f32; 3]> = corners.iter().map(|c| [c.0, y, c.1]).collect();
        self.poly(&p, [cx, if up { y - 1.0 } else { y + 1.0 }, cz], col);
    }

    /// A prism: walls and a top.
    fn prism(&mut self, corners: &[(f32, f32)], y0: f32, y1: f32, col: Rgb) {
        self.walls(corners, y0, y1, col);
        self.cap(corners, y1, true, col);
    }

    /// A box from `lo` to `hi`.
    fn cuboid(&mut self, lo: [f32; 3], hi: [f32; 3], col: Rgb) {
        let c = [(lo[0], lo[2]), (hi[0], lo[2]), (hi[0], hi[2]), (lo[0], hi[2])];
        self.prism(&c, lo[1], hi[1], col);
    }

    /// A roof sloping from a polygon at `y0` up to the same polygon shrunk by `top` (0: a point) at `y1`.
    fn roof(&mut self, corners: &[(f32, f32)], y0: f32, y1: f32, top: f32, col: Rgb) {
        let (cx, cz) = centre(corners);
        let inner: Vec<(f32, f32)> = corners.iter().map(|&(x, z)| (cx + (x - cx) * top, cz + (z - cz) * top)).collect();
        let inside = [cx, y0 - 2.0, cz];
        // in rows, a little lighter and darker by turns, for its courses of shingles
        let rows = (((y1 - y0) / 0.9).round() as usize).clamp(1, 12);
        let at = |p: (f32, f32), t: f32| {
            let k = 1.0 - t * (1.0 - top.max(0.0));
            [cx + (p.0 - cx) * k, y0 + (y1 - y0) * t, cz + (p.1 - cz) * k]
        };
        for i in 0..corners.len() {
            let j = (i + 1) % corners.len();
            let (a, b) = (corners[i], corners[j]);
            for r in 0..rows {
                let (t0, t1) = (r as f32 / rows as f32, (r + 1) as f32 / rows as f32);
                let c = scale(col, if r % 2 == 0 { 1.0 } else { 0.9 });
                if top <= 0.0 && r == rows - 1 {
                    self.poly(&[at(a, t0), at(b, t0), [cx, y1, cz]], inside, c);
                } else {
                    self.poly(&[at(a, t0), at(b, t0), at(b, t1), at(a, t1)], inside, c);
                }
            }
            // the eaves' underside
            self.poly(&[[a.0, y0, a.1], [b.0, y0, b.1], [cx, y0, cz]], [cx, y0 + 1.0, cz], scale(col, 0.6));
        }
        if top > 0.0 {
            let inner: Vec<(f32, f32)> = inner.to_vec();
            self.cap(&inner, y1, true, col);
        }
    }

    /// A ball, of eight by six faces.
    fn ball(&mut self, c: [f32; 3], r: f32, col: Rgb) {
        let (nu, nv) = (10, 7);
        let at = |i: usize, j: usize| {
            let (u, v) = (TAU * i as f32 / nu as f32, std::f32::consts::PI * j as f32 / nv as f32);
            [c[0] + r * v.sin() * u.cos(), c[1] + r * v.cos(), c[2] + r * v.sin() * u.sin()]
        };
        for i in 0..nu {
            for j in 0..nv {
                let (a, b, cc, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
                if j == 0 {
                    self.poly(&[a, cc, d], c, col);
                } else if j == nv - 1 {
                    self.poly(&[a, b, cc], c, col);
                } else {
                    self.poly(&[a, b, cc, d], c, col);
                }
            }
        }
    }

    /// Panels on a polygon's walls: on side `i` (from corner i to i + 1), a rectangle `w` wide and from `y0` to `y1`, at
    /// `along` (0..1) across the side, standing `out` off the wall.
    #[allow(clippy::too_many_arguments)]
    fn panel(&mut self, corners: &[(f32, f32)], i: usize, along: f32, w: f32, y0: f32, y1: f32, out: f32, col: Rgb) {
        let (cx, cz) = centre(corners);
        let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dz * dz).sqrt().max(1e-6);
        let (tx, tz) = (dx / len, dz / len);
        let (mut nx, mut nz) = (tz, -tx);
        let (mx, mz) = (a.0 + dx * along, a.1 + dz * along);
        if (mx - cx) * nx + (mz - cz) * nz < 0.0 {
            nx = -nx;
            nz = -nz;
        }
        let (px, pz) = (mx + nx * out, mz + nz * out);
        let h = w * 0.5;
        let p = [
            [px - tx * h, y0, pz - tz * h],
            [px + tx * h, y0, pz + tz * h],
            [px + tx * h, y1, pz + tz * h],
            [px - tx * h, y1, pz - tz * h],
        ];
        self.poly(&p, [cx, (y0 + y1) * 0.5, cz], col);
    }

    /// A window: a frame and its glass.
    #[allow(clippy::too_many_arguments)]
    fn window(&mut self, corners: &[(f32, f32)], i: usize, along: f32, w: f32, y0: f32, y1: f32, pal: &Palette) {
        self.panel(corners, i, along, w + 0.3, y0 - 0.15, y1 + 0.15, 0.04, pal.frame);
        self.panel(corners, i, along, w, y0, y1, 0.07, pal.glass);
        // its mullion
        self.panel(corners, i, along, 0.1, y0, y1, 0.09, pal.frame);
    }

    /// A striped awning over side `i` of a polygon: from the wall at `y` out `reach` and down `drop`.
    #[allow(clippy::too_many_arguments)]
    fn awning(&mut self, corners: &[(f32, f32)], i: usize, w: f32, y: f32, reach: f32, drop: f32, pal: &Palette) {
        let (cx, cz) = centre(corners);
        let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
        let (dx, dz) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dz * dz).sqrt().max(1e-6);
        let (tx, tz) = (dx / len, dz / len);
        let (mut nx, mut nz) = (tz, -tx);
        let (mx, mz) = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        if (mx - cx) * nx + (mz - cz) * nz < 0.0 {
            nx = -nx;
            nz = -nz;
        }
        let stripes = 7;
        for k in 0..stripes {
            let (s0, s1) = (-w * 0.5 + w * k as f32 / stripes as f32, -w * 0.5 + w * (k + 1) as f32 / stripes as f32);
            let col = if k % 2 == 0 { pal.awning } else { pal.frame };
            let wall = |s: f32| [mx + tx * s + nx * 0.05, y, mz + tz * s + nz * 0.05];
            let edge = |s: f32, d: f32| [mx + tx * s + nx * reach, y - drop - d, mz + tz * s + nz * reach];
            self.poly(&[wall(s0), wall(s1), edge(s1, 0.0), edge(s0, 0.0)], [cx, y - 3.0, cz], col);
            // its valance
            self.poly(&[edge(s0, 0.0), edge(s1, 0.0), edge(s1, 0.35), edge(s0, 0.35)], [cx, y - drop, cz], col);
        }
    }

    /// A railing round a polygon's edge at deck height `y`, its posts at most `step` apart; `gap` leaves side `gap` open
    /// (for steps).
    fn railing(&mut self, corners: &[(f32, f32)], y: f32, step: f32, gap: Option<usize>, col: Rgb) {
        let (cx, cz) = centre(corners);
        let h = 1.05;
        for i in 0..corners.len() {
            let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let n = (len / step).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let (x, z) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                self.cuboid([x - 0.09, y, z - 0.09], [x + 0.09, y + h + 0.1, z + 0.09], col);
            }
            if gap == Some(i) {
                continue;
            }
            // top rail and bottom rail, as thin slabs
            for (y0, y1) in [(y + h - 0.08, y + h + 0.04), (y + 0.15, y + 0.25)] {
                let inset = |p: (f32, f32), d: f32| (p.0 + (cx - p.0) * d, p.1 + (cz - p.1) * d);
                let (a2, b2) = (inset(a, 0.0), inset(b, 0.0));
                let (a3, b3) = (inset(a, 0.006), inset(b, 0.006));
                self.poly(&[[a2.0, y0, a2.1], [b2.0, y0, b2.1], [b2.0, y1, b2.1], [a2.0, y1, a2.1]], [cx, y0, cz], col);
                self.poly(&[[a2.0, y1, a2.1], [b2.0, y1, b2.1], [b3.0, y1, b3.1], [a3.0, y1, a3.1]], [cx, y0, cz], col);
                // balusters, close set, between
                let _ = (a3, b3);
            }
            let m = (len / 0.45).floor() as usize;
            for k in 1..m {
                let t = k as f32 / m as f32;
                let (x, z) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                self.cuboid([x - 0.03, y + 0.2, z - 0.03], [x + 0.03, y + h - 0.05, z + 0.03], scale(col, 0.95));
            }
        }
    }

    /// A cypress: a trunk under a tall, narrow cone of leaves.
    fn cypress(&mut self, x: f32, z: f32, h: f32, pal: &Palette) {
        self.cuboid([x - 0.15, 0.0, z - 0.15], [x + 0.15, 0.8, z + 0.15], pal.trunk);
        let base = ring(x, z, h * 0.17, 8, OCT);
        let mid = ring(x, z, h * 0.15, 8, OCT);
        self.roof(&base, 0.6, h * 0.45, 0.0, pal.leaf_dark);
        // (a second, upper cone over the first gives the cypress its taper)
        let _ = mid;
        let upper = ring(x, z, h * 0.15, 8, 0.0);
        self.walls(&base, 0.6, h * 0.3, pal.leaf_dark);
        self.roof(&upper, h * 0.3, h, 0.0, pal.leaf_dark);
    }

    /// A shrub: a low leafy ball.
    fn shrub(&mut self, x: f32, z: f32, r: f32, pal: &Palette, dark: bool) {
        let c = if dark { pal.leaf_dark } else { pal.leaf };
        self.ball([x, r * 0.7, z], r, c);
        for k in 0..3 {
            let a = TAU * (k as f32 / 3.0 + hash(x, z));
            self.ball([x + a.cos() * r * 0.6, r * 0.55, z + a.sin() * r * 0.6], r * 0.65, scale(c, 0.9 + 0.1 * k as f32));
        }
    }

    /// A lamp: a post with a coloured head.
    fn lamp(&mut self, x: f32, z: f32, pal: &Palette) {
        let iron = [0.16, 0.17, 0.17];
        self.cuboid([x - 0.12, 0.0, z - 0.12], [x + 0.12, 0.4, z + 0.12], iron);
        self.cuboid([x - 0.05, 0.4, z - 0.05], [x + 0.05, 2.7, z + 0.05], iron);
        self.ball([x, 2.95, z], 0.22, pal.lamp);
        self.roof(&ring(x, z, 0.3, 6, 0.0), 3.12, 3.35, 0.0, iron);
    }

    /// A flower bed: a low mound and its flowers.
    fn bed(&mut self, x: f32, z: f32, w: f32, d: f32, pal: &Palette) {
        self.cuboid([x - w * 0.5, 0.0, z - d * 0.5], [x + w * 0.5, 0.25, z + d * 0.5], pal.leaf);
        let n = ((w * d) / 0.8).ceil() as usize;
        for k in 0..n {
            let h = hash(k as f32 * 1.7 + x, z + k as f32 * 0.3);
            let h2 = hash(z - k as f32 * 2.1, x * 0.7 + k as f32);
            let (fx, fz) = (x + (h - 0.5) * w * 0.9, z + (h2 - 0.5) * d * 0.9);
            self.ball([fx, 0.4, fz], 0.22, if k % 3 == 0 { pal.frame } else { pal.flower });
        }
    }

    /// An octagonal hall: walls with a band of trim, a window on each side (two storeys when tall), and a roof.
    #[allow(clippy::too_many_arguments)]
    fn hall(
        &mut self,
        cx: f32,
        cz: f32,
        r: f32,
        floor: f32,
        wall: f32,
        roof: f32,
        finial: bool,
        pal: &Palette,
        door: Option<usize>,
    ) -> Vec<(f32, f32)> {
        let c = ring(cx, cz, r, 8, OCT);
        self.walls(&c, 0.0, floor + wall, pal.wall);
        // its clapboards: a shadow line under each board
        let side_len = 2.0 * r * OCT.sin();
        let mut y = floor + 0.45;
        while y < floor + wall - 0.6 {
            for i in 0..8 {
                self.panel(&c, i, 0.5, side_len, y - 0.06, y, 0.015, scale(pal.wall, 0.86));
            }
            y += 0.45;
        }
        // its foundation and a band under the eaves
        self.walls(&ring(cx, cz, r + 0.12, 8, OCT), 0.0, floor, pal.stone);
        self.walls(&ring(cx, cz, r + 0.08, 8, OCT), floor + wall - 0.5, floor + wall, pal.trim);
        let storeys = if wall > 6.5 { 2 } else { 1 };
        let side = 2.0 * r * OCT.sin();
        let sh = wall / storeys as f32;
        for i in 0..8 {
            for s in 0..storeys {
                let y0 = floor + sh * s as f32 + sh * 0.3;
                let y1 = floor + sh * s as f32 + sh * 0.8;
                if door == Some(i) && s == 0 {
                    // double doors and their frame
                    self.panel(&c, i, 0.5, 2.2, floor, floor + 2.8, 0.04, pal.frame);
                    self.panel(&c, i, 0.5, 1.9, floor, floor + 2.6, 0.07, pal.door);
                    self.panel(&c, i, 0.5, 0.08, floor, floor + 2.6, 0.09, pal.frame);
                } else if side > 4.0 {
                    let ww = (side * 0.22).min(1.4);
                    self.window(&c, i, 0.3, ww, y0, y1, pal);
                    self.window(&c, i, 0.7, ww, y0, y1, pal);
                } else {
                    self.window(&c, i, 0.5, (side * 0.4).min(1.4), y0, y1, pal);
                }
            }
            if storeys == 2 {
                // the band between the storeys
                self.panel(&c, i, 0.5, side, floor + sh - 0.15, floor + sh + 0.15, 0.06, pal.trim);
            }
        }
        let eave = ring(cx, cz, r * 1.12, 8, OCT);
        let top = floor + wall;
        if finial {
            self.roof(&eave, top, top + roof, 0.0, pal.roof);
            self.cuboid([cx - 0.08, top + roof - 0.1, cz - 0.08], [cx + 0.08, top + roof + 0.9, cz + 0.08], pal.gold);
            self.ball([cx, top + roof + 1.0, cz], 0.32, pal.gold);
        }
        c
    }
}

fn centre(c: &[(f32, f32)]) -> (f32, f32) {
    let n = c.len().max(1) as f32;
    let (x, z) = c.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    (x / n, z / n)
}

fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|q| q * k)
}

fn hash(x: f32, z: f32) -> f32 {
    ((x * 12.9898 + z * 78.233).sin() * 43758.547).fract().abs()
}

/// The clubhouse: a two storey octagonal hall under a hipped roof and a lantern, three octagonal pavilions with striped
/// awnings at its corners, all on a railed veranda deck, with cypresses, shrubs, flower beds and lamps round it. In
/// metres, front to +z, on a footprint 4 tiles (91 m) across.
fn clubhouse(theme: usize) -> Build {
    let pal = clubhouse_palette(theme);
    let mut b = Build::default();
    let floor = 1.4;
    // the veranda: an octagon stretched to the building, raised on a stone skirt
    let deck: Vec<(f32, f32)> =
        vec![(-24.0, -30.0), (24.0, -30.0), (31.0, -23.0), (31.0, 22.0), (24.0, 29.0), (-24.0, 29.0), (-31.0, 22.0), (-31.0, -23.0)];
    b.walls(&deck, -0.6, floor, pal.stone);
    b.cap(&deck, floor, true, pal.deck);
    // its boards
    let mut x = -30.5;
    while x < 31.0 {
        b.poly(
            &[[x, floor + 0.01, -29.5], [x + 0.06, floor + 0.01, -29.5], [x + 0.06, floor + 0.01, 28.5], [x, floor + 0.01, 28.5]],
            [x, 0.0, 0.0],
            scale(pal.deck, 0.8),
        );
        x += 0.6;
    }
    b.railing(&deck, floor, 2.4, Some(4), pal.rail);
    // steps down from the front, between two lamps
    for k in 0..4 {
        let y = floor - (k as f32 + 1.0) * floor / 4.0;
        let z = 29.0 + k as f32 * 0.45;
        b.cuboid([-5.0, -0.5, z], [5.0, y + floor / 4.0, z + 0.45], pal.stone);
    }
    b.lamp(-6.5, 31.5, &pal);
    b.lamp(6.5, 31.5, &pal);
    // the hall: two storeys under a roof that rises to a lantern
    let r = 13.0;
    b.hall(0.0, -2.0, r, floor, 10.0, 0.0, false, &pal, Some(2));
    // a porch round the hall's ground floor: a sloping roof on posts
    let porch_in = ring(0.0, -2.0, r * 1.03, 8, OCT);
    let porch_out = ring(0.0, -2.0, r + 3.6, 8, OCT);
    let (py, pdrop) = (floor + 4.6, 0.8);
    for i in 0..8 {
        let j = (i + 1) % 8;
        let (a, bb, c, d) = (porch_in[i], porch_in[j], porch_out[j], porch_out[i]);
        b.poly(&[[a.0, py, a.1], [bb.0, py, bb.1], [c.0, py - pdrop, c.1], [d.0, py - pdrop, d.1]], [0.0, 0.0, -2.0], pal.roof);
        // its fascia
        b.poly(
            &[[d.0, py - pdrop, d.1], [c.0, py - pdrop, c.1], [c.0, py - pdrop - 0.35, c.1], [d.0, py - pdrop - 0.35, d.1]],
            [0.0, py, -2.0],
            pal.trim,
        );
        b.poly(
            &[[a.0, py - 0.05, a.1], [bb.0, py - 0.05, bb.1], [c.0, py - pdrop - 0.05, c.1], [d.0, py - pdrop - 0.05, d.1]],
            [0.0, py + 5.0, -2.0],
            scale(pal.wall, 0.8),
        );
        for t in [0.0, 0.5] {
            let (x, z) = (d.0 + (c.0 - d.0) * t, d.1 + (c.1 - d.1) * t);
            let (x, z) = (x * 0.97, -2.0 + (z + 2.0) * 0.97);
            b.cuboid([x - 0.14, floor, z - 0.14], [x + 0.14, py - pdrop, z + 0.14], pal.rail);
        }
    }
    let top = floor + 10.0;
    let eave = ring(0.0, -2.0, r * 1.12, 8, OCT);
    b.roof(&eave, top, top + 6.5, 0.32, pal.roof);
    // the lantern on top, its roof and finial
    let lantern = ring(0.0, -2.0, 3.6, 8, OCT);
    let ly = top + 6.5;
    b.walls(&lantern, ly, ly + 3.2, pal.wall);
    for i in 0..8 {
        b.window(&lantern, i, 0.5, 1.2, ly + 0.6, ly + 2.6, &pal);
    }
    b.walls(&ring(0.0, -2.0, 3.7, 8, OCT), ly + 2.9, ly + 3.2, pal.trim);
    b.roof(&ring(0.0, -2.0, 4.4, 8, OCT), ly + 3.2, ly + 7.0, 0.0, pal.roof);
    b.cuboid([-0.08, ly + 6.8, -2.08], [0.08, ly + 8.0, -1.92], pal.gold);
    b.ball([0.0, ly + 8.1, -2.0], 0.4, pal.gold);
    // the pavilions at three corners, each with awnings over the windows facing out
    for &(px, pz, out) in &[(-19.0, 15.0, [0usize, 1, 2]), (19.0, 15.0, [6, 7, 0]), (-19.0, -17.0, [3, 4, 5])] {
        // (sides are counted from +x round towards +z: 0 faces +x, 2 faces +z, 4 faces -x, 6 faces -z)
        let c = b.hall(px, pz, 7.0, floor, 6.0, 5.5, true, &pal, None);
        for &i in &out {
            let side = 2.0 * 7.0 * OCT.sin();
            b.awning(&c, i, side * 0.8, floor + 4.7, 1.3, 0.8, &pal);
        }
    }
    // the garden round the deck
    let cypresses = [
        (-34.0, 26.0),
        (34.0, 26.0),
        (-35.0, -10.0),
        (35.0, -14.0),
        (-26.0, -34.0),
        (12.0, -35.0),
        (-12.0, 35.0),
        (14.0, 35.5),
        (36.0, 5.0),
    ];
    for (k, &(x, z)) in cypresses.iter().enumerate() {
        b.cypress(x, z, 6.5 + hash(x, z) * 2.5 + (k % 2) as f32, &pal);
    }
    for k in 0..26 {
        let a = TAU * k as f32 / 26.0;
        let (x, z) = (34.0 * a.cos(), 33.0 * a.sin());
        if z > 28.0 && x.abs() < 9.0 {
            continue;
        }
        b.shrub(x, z, 0.9 + hash(x, z) * 0.7, &pal, k % 3 == 0);
    }
    b.bed(-14.0, 33.0, 8.0, 2.4, &pal);
    b.bed(16.0, 33.0, 8.0, 2.4, &pal);
    b.bed(-34.5, 6.0, 2.4, 9.0, &pal);
    b.lamp(-33.0, -24.0, &pal);
    b.lamp(33.0, -26.0, &pal);
    b
}

/// The model of an object kind (data.rs OBJECT_IDS) in a theme, if it has one: its triangles in world units about the
/// footprint's centre, front to +z (facing 0).
pub fn model(kind: usize, theme: usize) -> Option<Vec<Vert>> {
    let b = match sg_core::data::object_id(kind) {
        "clubhouse" => clubhouse(theme),
        _ => return None,
    };
    Some(
        b.v.into_iter()
            .map(|mut v| {
                v.pos = v.pos.map(|q| q * M);
                v
            })
            .collect(),
    )
}

/// The model's half size (world units) for what stops a walk: the veranda.
pub fn model_half(kind: usize) -> Option<(f32, f32)> {
    match sg_core::data::object_id(kind) {
        "clubhouse" => Some((31.0 * M, 30.0 * M)),
        _ => None,
    }
}

/// `v` turned `quarters` quarter turns about the vertical (counter clockwise from above, x towards -z) and moved to
/// `at`, its ground at `ground`.
pub fn placed(v: &[Vert], quarters: i32, at: [f32; 3]) -> impl Iterator<Item = Vert> + '_ {
    let q = quarters.rem_euclid(4);
    let turn = move |x: f32, z: f32| match q {
        0 => (x, z),
        1 => (z, -x),
        2 => (-x, -z),
        _ => (-z, x),
    };
    v.iter().map(move |v| {
        let mut o = *v;
        let (x, z) = turn(v.pos[0], v.pos[2]);
        o.pos = [at[0] + x, at[1] + v.pos[1], at[2] + z];
        let (nx, nz) = turn(v.normal[0], v.normal[2]);
        o.normal = [nx, v.normal[1], nz];
        o
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clubhouse_has_a_model_in_every_theme_and_stands_in_its_footprint() {
        let kind = (0..20).find(|&k| sg_core::data::object_id(k) == "clubhouse").unwrap();
        for theme in 0..4 {
            let v = model(kind, theme).unwrap();
            assert!(v.len() > 1000 && v.len() % 3 == 0);
            let half = 2.0 * sg_core::terrain::TILE_SIZE;
            for p in &v {
                assert!(p.pos[0].abs() < half && p.pos[2].abs() < half, "{:?}", p.pos);
                assert!(p.pos[1] > -1.0 * M && p.pos[1] < 40.0 * M);
            }
        }
        assert!(model(0, 0).is_none());
    }

    #[test]
    fn a_turned_model_keeps_its_normals_with_its_faces() {
        let mut v = Vert::new(1.0, 0.0, 0.0, 0.0, 0.0);
        v.normal = [1.0, 0.0, 0.0];
        for q in 0..4 {
            let o: Vec<Vert> = placed(&[v], q, [0.0; 3]).collect();
            assert!((o[0].pos[0] - o[0].normal[0]).abs() < 1e-6 && (o[0].pos[2] - o[0].normal[2]).abs() < 1e-6);
        }
    }
}
