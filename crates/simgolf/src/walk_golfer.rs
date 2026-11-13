//! Golfers in the round for the walk camera: jointed bodies that act out what the game has them doing.
//!
//! A golfer's pictures are drawn for eight headings and a few dozen pixels tall; seen from the ground at a few paces they
//! are flat and blocky, and no single solid can stand for them because they change with every step and every swing. So a
//! golfer here is a small figure of the project's own design: hips, a body, a head with a cap (or hair), arms and legs
//! jointed at the knees and elbows, shoes, and a club, coloured from the golfer's own outfit (the shirt, trousers, skin,
//! hat, hair and hand colours the game composes for them from the swap palettes, `sg_core::bodies`).
//!
//! The game says what each golfer is doing by the clip it plays (`app::GOLFER_CLIPS`: walking, addressing the ball,
//! swinging, chipping, putting, lining up a putt, cheering, sulking, sitting, pointing...) and how far through it they
//! are. Each clip is a pose here, moving with the clip's progress: the walk's stride, the swing's backswing, downswing and
//! follow-through, the putter's pendulum. Feet and hands are placed where the pose wants them and the knees and elbows
//! found between (two-bone reaching), so a pose is a handful of targets rather than every joint's angle.

use crate::gfx::Vert;
use std::f32::consts::{PI, TAU};

type V = [f32; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: V, k: f32) -> V {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: V) -> f32 {
    dot(a, a).sqrt()
}
fn norm(a: V) -> V {
    mul(a, 1.0 / len(a).max(1e-6))
}
/// `a` turned about the vertical by `t` (radians; +x towards +z).
fn yaw(a: V, t: f32) -> V {
    let (s, c) = t.sin_cos();
    [a[0] * c - a[2] * s, a[1], a[0] * s + a[2] * c]
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A golfer's colours (linear 0..1).
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub shirt: [f32; 3],
    pub pants: [f32; 3],
    pub skin: [f32; 3],
    pub hat: [f32; 3],
    pub hair: [f32; 3],
    pub hands: [f32; 3],
    pub female: bool,
}

impl Look {
    /// From a composed golfer palette (`Swaps::compose`): the middle of each range's ramp (shirt 0..20, trousers 20..40,
    /// skin 40..60, hat 60..80, then hair 80..90 and arms 90..97 for a woman, arms 80..90 for a man).
    pub fn from_palette(pal: &[u8; 768], female: bool) -> Look {
        let ramp = |first: usize, n: usize| {
            // the ramp's entries by brightness, and the middle third's mean: neither its highlight nor its shadow
            let mut c: Vec<[f32; 3]> =
                (first..first + n).map(|i| [pal[i * 3], pal[i * 3 + 1], pal[i * 3 + 2]].map(|v| v as f32 / 255.0)).collect();
            c.sort_by(|a, b| (a[0] + a[1] + a[2]).partial_cmp(&(b[0] + b[1] + b[2])).unwrap_or(std::cmp::Ordering::Equal));
            let mid = &c[n / 3..(2 * n / 3).max(n / 3 + 1)];
            let s = mid.iter().fold([0.0; 3], |a, v| [a[0] + v[0], a[1] + v[1], a[2] + v[2]]);
            s.map(|v| v / mid.len() as f32)
        };
        let skin = ramp(40, 20);
        Look {
            shirt: ramp(0, 20),
            pants: ramp(20, 20),
            skin,
            hat: ramp(60, 20),
            hair: if female { ramp(80, 10) } else { [0.22, 0.15, 0.1] },
            hands: if female { ramp(90, 7) } else { ramp(80, 10) },
            female,
        }
    }

    /// When the game has no swap palettes: colours from the outfit's numbers.
    pub fn plain(o: &sg_core::bodies::Outfit) -> Look {
        const CLOTH: [[f32; 3]; 10] = [
            [0.92, 0.92, 0.9],
            [0.75, 0.15, 0.15],
            [0.15, 0.3, 0.7],
            [0.2, 0.55, 0.25],
            [0.95, 0.8, 0.2],
            [0.5, 0.25, 0.6],
            [0.15, 0.15, 0.18],
            [0.9, 0.5, 0.65],
            [0.55, 0.45, 0.3],
            [0.4, 0.75, 0.85],
        ];
        const SKIN: [[f32; 3]; 4] = [[0.93, 0.76, 0.62], [0.8, 0.6, 0.45], [0.6, 0.42, 0.3], [0.38, 0.25, 0.18]];
        let skin = SKIN[o.skin as usize % 4];
        Look {
            shirt: CLOTH[o.shirt as usize % 10],
            pants: CLOTH[(o.pants as usize + 6) % 10],
            skin,
            hat: CLOTH[o.hat as usize % 10],
            hair: [0.25, 0.17, 0.1],
            hands: if o.alt_skin == 4 { [0.95, 0.95, 0.92] } else { skin },
            female: o.female,
        }
    }
}

/// What a golfer is doing: the game's clip, by its place in `app::GOLFER_CLIPS`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Stand,
    Walk {
        tired: bool,
    },
    Fidget,
    Happy,
    /// A fist pumped (a great shot).
    Pump,
    Sad,
    /// The club raised and slammed down in a temper, then hands on hips (a bad shot).
    Fail,
    Swing {
        reach: f32,
    },
    Putt,
    Address {
        putt: bool,
    },
    LineUp,
    Point,
    Lean(f32),
    LookUp,
    Sit,
    Shake,
}

impl Act {
    /// The act of a clip by its name in `app::GOLFER_CLIPS`.
    pub fn of_clip(name: &str) -> Act {
        match name {
            "NormalWalk" => Act::Walk { tired: false },
            "TiredWalk" => Act::Walk { tired: true },
            "Fidget" => Act::Fidget,
            "Happy" => Act::Happy,
            "SuccessA" => Act::Pump,
            "Sad" => Act::Sad,
            "FailureA" => Act::Fail,
            "PerfectSwing" | "NoAccSwing" | "NoimagSwing" | "NormalSwing" => Act::Swing { reach: 1.0 },
            "Pitch" => Act::Swing { reach: 0.55 },
            "Putt" => Act::Putt,
            "NormalAddress" | "PitchAddress" => Act::Address { putt: false },
            "PuttAddress" => Act::Address { putt: true },
            "LineUpPutt" => Act::LineUp,
            "PointAt" => Act::Point,
            "LeanRight" => Act::Lean(1.0),
            "LeanLeft" => Act::Lean(-1.0),
            "LookUp" => Act::LookUp,
            "Sitting" | "SitSq" | "Cart" => Act::Sit,
            "HandShake" => Act::Shake,
            _ => Act::Stand,
        }
    }

    /// Whether the golfer stands side on to the heading (the shot's line), as at the ball; otherwise they face it.
    pub fn at_ball(self) -> bool {
        matches!(self, Act::Swing { .. } | Act::Putt | Act::Address { .. })
    }
}

/// The body's targets for a pose, in metres for a figure 1.78 m tall: x to the golfer's right, y up, z forward.
struct Pose {
    /// The middle of the hips.
    hips: V,
    /// The hips' turn (radians, about the vertical, + turns the right hip back) and the shoulders' turn on top of it.
    hip_turn: f32,
    chest_turn: f32,
    /// The spine's bend forward and lean to the right (radians).
    bend: f32,
    lean: f32,
    /// The head's nod (radians, + looks down).
    nod: f32,
    /// The feet (ankles) and the hands (right, left).
    feet: [V; 2],
    hands: [V; 2],
    /// The club: where it is held (the top hand) and its direction down the shaft; none when it isn't in hand.
    club: Option<(V, V, f32)>,
}

/// The two-bone reach from `a` towards `t` with bone lengths `l1`, `l2`, bending towards `hint`: the middle joint and
/// the end (at `t` when it is in reach).
fn reach(a: V, t: V, l1: f32, l2: f32, hint: V) -> (V, V) {
    let d = sub(t, a);
    let dist = len(d).clamp(0.05, (l1 + l2) * 0.999);
    let dir = norm(d);
    let ca = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let sa = (1.0 - ca * ca).sqrt();
    let mut h = sub(hint, mul(dir, dot(hint, dir)));
    if len(h) < 1e-4 {
        h = cross(dir, [1.0, 0.0, 0.0]);
    }
    let h = norm(h);
    let mid = add(a, add(mul(dir, l1 * ca), mul(h, l1 * sa)));
    (mid, add(a, mul(dir, dist)))
}

/// The pose for an act `t` of the way through its clip (0..1), `time` in seconds for idle movement.
fn pose(act: Act, t: f32, time: f32, club_len: f32) -> Pose {
    let stand = Pose {
        hips: [0.0, 0.95, 0.0],
        hip_turn: 0.0,
        chest_turn: 0.0,
        bend: 0.04,
        lean: 0.0,
        nod: 0.05,
        feet: [[0.12, 0.08, 0.0], [-0.12, 0.08, 0.0]],
        hands: [[0.27, 0.68, 0.04], [-0.27, 0.68, 0.04]],
        club: None,
    };
    let breathe = (time * 1.7).sin() * 0.008;
    match act {
        Act::Stand | Act::Fidget | Act::LookUp => {
            let shift = if act == Act::Fidget { (t * TAU).sin() * 0.04 } else { 0.0 };
            let mut p = Pose { hips: [shift, 0.95 + breathe, 0.0], lean: shift * 1.2, ..stand };
            if act == Act::LookUp {
                // watching the ball fly, a hand shading the eyes, the club in the other
                p.nod = -0.45;
                p.hands = [[0.27, 0.68, 0.06], [-0.05, 1.72, 0.2]];
                p.club = Some(([0.27, 0.68, 0.06], norm([0.08, -1.0, 0.18]), club_len));
            }
            if act == Act::Fidget {
                // a hand to the cap's peak at the clip's middle
                let k = smooth(1.0 - ((t - 0.5).abs() * 3.0));
                p.hands[0] = [lerp(0.27, 0.1, k), lerp(0.68, 1.66, k), lerp(0.04, 0.12, k)];
            }
            p
        }
        Act::Walk { tired } => {
            let ph = t * TAU;
            let stride = if tired { 0.22 } else { 0.32 };
            let foot = |k: f32| {
                let s = (ph + k).sin();
                let lift = (ph + k).cos().max(0.0) * 0.09;
                (s * stride, 0.08 + lift)
            };
            let (zr, yr) = foot(0.0);
            let (zl, yl) = foot(PI);
            let bob = (ph * 2.0).cos().abs() * 0.025;
            let swing = ph.sin() * 0.22;
            Pose {
                hips: [0.0, 0.93 + bob, 0.0],
                hip_turn: -swing * 0.25,
                chest_turn: swing * 0.35,
                bend: if tired { 0.2 } else { 0.06 },
                nod: if tired { 0.3 } else { 0.05 },
                feet: [[0.1, yr, zr], [-0.1, yl, zl]],
                hands: [[0.25, 0.7, -swing], [-0.25, 0.7, swing]],
                // a tired golfer drags the club behind him
                club: tired.then(|| ([0.25, 0.7, -swing], norm([0.12, -0.7, -0.7]), club_len)),
                ..stand
            }
        }
        Act::Happy => {
            let jump = ((t * TAU * 2.0).sin()).max(0.0) * 0.07;
            Pose {
                hips: [0.0, 0.95 + jump, 0.0],
                nod: -0.15,
                feet: [[0.13, 0.08 + jump, 0.0], [-0.13, 0.08 + jump, 0.0]],
                hands: [[0.3, 1.95 + jump, 0.08], [-0.3, 1.95 + jump, 0.08]],
                ..stand
            }
        }
        Act::Sad => Pose { bend: 0.28, nod: 0.55, hips: [0.0, 0.94, 0.0], hands: [[0.17, 0.95, 0.08], [-0.17, 0.95, 0.08]], ..stand },
        Act::Pump => {
            // the right fist pumped down twice, a little crouch with each; the club in the left hand
            let k = 0.5 + 0.5 * (t * TAU * 2.0).cos();
            let fist = [lerp(0.16, 0.26, k), lerp(0.95, 1.6, k), lerp(0.2, 0.18, k)];
            Pose {
                hips: [0.0, 0.93 - (1.0 - k) * 0.05, 0.0],
                bend: 0.1 + (1.0 - k) * 0.12,
                nod: -0.1,
                feet: [[0.17, 0.08, 0.02], [-0.17, 0.08, 0.0]],
                hands: [fist, [-0.26, 0.7, 0.05]],
                club: Some(([-0.26, 0.7, 0.05], norm([-0.05, -0.6, 0.8]), club_len)),
                ..stand
            }
        }
        Act::Fail => {
            // the club up over the head, slammed down in front, then hands on hips glaring at it
            let (grip, shaft, hands, bend) = if t < 0.3 {
                let k = smooth(t / 0.3);
                let g = [0.0, lerp(0.85, 1.95, k), lerp(0.3, -0.05, k)];
                (g, norm([0.0, lerp(-0.9, 0.6, k), lerp(0.4, -0.8, k)]), [add(g, [0.05, -0.05, 0.0]), g], lerp(0.1, -0.1, k))
            } else if t < 0.4 {
                let k = smooth((t - 0.3) / 0.1);
                let g = [0.0, lerp(1.95, 0.75, k), lerp(-0.05, 0.4, k)];
                (g, norm([0.0, lerp(0.6, -0.8, k), lerp(-0.8, 0.6, k)]), [add(g, [0.05, -0.05, 0.0]), g], lerp(-0.1, 0.45, k))
            } else {
                // the club left lying where it was thrown
                ([0.12, 0.03, 0.4], norm([0.2, 0.0, 1.0]), [[0.2, 0.98, -0.04], [-0.2, 0.98, -0.04]], 0.15)
            };
            Pose { bend, nod: 0.35, hands, club: Some((grip, shaft, club_len)), feet: [[0.16, 0.08, 0.0], [-0.16, 0.08, 0.0]], ..stand }
        }
        Act::Point => Pose { nod: -0.05, hands: [[0.12, 1.5, 0.62], [-0.27, 0.68, 0.04]], ..stand },
        Act::Shake => Pose { hands: [[0.08, 1.0, 0.45], [-0.27, 0.68, 0.04]], ..stand },
        Act::Lean(side) => {
            let k = smooth(1.0 - ((t - 0.5).abs() * 2.2)) * side;
            Pose { lean: k * 0.3, hips: [-k * 0.05, 0.95, 0.0], nod: 0.15, ..stand }
        }
        Act::Sit => Pose {
            hips: [0.0, 0.5, -0.05],
            bend: -0.05,
            feet: [[0.14, 0.08, 0.42], [-0.14, 0.08, 0.42]],
            hands: [[0.18, 0.6, 0.3], [-0.18, 0.6, 0.3]],
            ..stand
        },
        Act::LineUp => Pose {
            hips: [0.0, 0.48, -0.15],
            bend: 0.55,
            nod: 0.1,
            feet: [[0.2, 0.08, 0.15], [-0.2, 0.08, -0.05]],
            hands: [[0.2, 0.55, 0.42], [-0.15, 0.55, 0.3]],
            club: Some(([-0.15, 0.55, 0.3], norm([0.1, -0.45, 0.6]), club_len)),
            ..stand
        },
        Act::Swing { reach } => full_swing(reach, t, club_len),
        Act::Address { putt: false } => {
            // at the ball, the club waggling a little
            let mut p = full_swing(1.0, 0.0, club_len);
            if let Some((g, d, l)) = p.club {
                let w = (time * 2.2).sin() * 0.06;
                p.club = Some((g, norm(add(d, [w, 0.0, 0.0])), l));
                p.hands[0] = add(g, mul(p.club.unwrap().1, 0.09));
            }
            p
        }
        Act::Putt | Act::Address { .. } => {
            let putt = matches!(act, Act::Putt | Act::Address { putt: true });
            // the club's turn round the swing (radians): 0 at the ball, + back over the right shoulder, - through
            let theta = match act {
                Act::Swing { reach } => {
                    let top = 2.4 * reach;
                    let end = -2.6 * reach;
                    if t < 0.12 {
                        0.0
                    } else if t < 0.48 {
                        top * smooth((t - 0.12) / 0.36)
                    } else if t < 0.56 {
                        top * (1.0 - ((t - 0.48) / 0.08).powf(1.6))
                    } else if t < 0.78 {
                        end * smooth((t - 0.56) / 0.22)
                    } else {
                        end
                    }
                }
                Act::Putt => {
                    if t < 0.15 {
                        0.0
                    } else if t < 0.5 {
                        0.32 * (smooth((t - 0.15) / 0.35))
                    } else if t < 0.62 {
                        0.32 * (1.0 - smooth((t - 0.5) / 0.12))
                    } else {
                        -0.35 * smooth((t - 0.62) / 0.2)
                    }
                }
                _ => (time * 2.2).sin() * 0.05,
            };
            let bend: f32 = if putt { 0.62 } else { 0.45 };
            let hips = [0.0, if putt { 0.9 } else { 0.88 }, -0.04];
            let spine = 0.5;
            let shoulders = add(hips, [0.0, spine * bend.cos(), spine * bend.sin()]);
            // the hands' circle round the shoulders, in the plane of the target line and the club's line at the ball
            let down = norm([0.0, -0.9, if putt { 0.3 } else { 0.48 }]);
            let r = if putt { 0.5 } else { 0.6 };
            let side = [1.0, 0.0, 0.0];
            // the backswing lifts the hands up and round, the follow-through round to the left
            let grip = add(shoulders, mul(add(mul(side, theta.sin()), mul(down, theta.cos())), r));
            let grip = if theta.abs() > 1.4 { add(grip, [0.0, (theta.abs() - 1.4) * 0.12, 0.0]) } else { grip };
            // the wrists cock in the backswing and release through the ball
            let cock = if putt {
                theta
            } else if theta > 0.0 {
                theta + (theta / 2.4).min(1.0) * 1.3
            } else {
                theta * 1.25
            };
            let shaft = norm(add(mul(side, cock.sin()), mul(down, cock.cos())));
            let shaft = norm(add(shaft, [0.0, if theta.abs() > 1.6 { 0.2 } else { 0.0 }, 0.0]));
            let right = add(grip, mul(shaft, 0.09));
            let turn = theta.clamp(-2.6, 2.4);
            let finish = (-theta - 1.2).max(0.0);
            Pose {
                hips: add(hips, [turn * -0.03, 0.0, 0.0]),
                hip_turn: turn * 0.18,
                chest_turn: turn * 0.38,
                bend: bend - finish * 0.12,
                lean: 0.06 + turn * -0.02,
                nod: if putt { 0.9 } else { 0.75 - finish * 0.35 },
                feet: [[0.23, 0.08, 0.0], [-0.23, 0.08 + finish * 0.03, 0.0]],
                hands: [right, grip],
                club: Some((grip, shaft, club_len)),
            }
        }
    }
}

/// A full swing's keys through its clip (the game strikes the ball three quarters of the way through a swing or chip clip,
/// golfer.rs, so impact is keyed there): (t, the top hand's place, the club's direction down the shaft, the shoulders'
/// turn), in metres with x to the golfer's right, y up, z towards the ball; the target is on their left (-x).
const SWING: [(f32, V, V, f32); 9] = [
    // at the ball, still
    (0.0, [-0.04, 0.84, 0.3], [0.04, -0.87, 0.48], 0.0),
    (0.1, [-0.04, 0.84, 0.3], [0.04, -0.87, 0.48], 0.0),
    // the takeaway: hands back to the right hip, the club along the target line, toe up
    (0.3, [0.3, 0.86, 0.3], [1.0, 0.12, -0.05], 0.55),
    // halfway back: the wrists cocked, the club upright
    (0.45, [0.42, 1.18, 0.14], [0.35, 1.0, -0.2], 1.0),
    // the top: the hands over the right shoulder, the club across the back of the neck towards the target
    (0.6, [0.26, 1.52, -0.04], [-1.0, -0.08, -0.3], 1.45),
    // the downswing: the club lagging behind the hands
    (0.7, [0.34, 1.02, 0.22], [0.55, 0.95, -0.05], 0.75),
    // impact: back at the ball, the hands a little ahead
    (0.77, [-0.1, 0.84, 0.31], [0.1, -0.87, 0.47], 0.15),
    // through: the arms out towards the target, the club along the line
    (0.85, [-0.4, 0.95, 0.28], [-1.0, 0.2, 0.05], -0.85),
    // the finish: the hands high by the left ear, the club down the back, the chest to the target
    (0.96, [-0.2, 1.48, 0.02], [0.65, -0.45, -0.62], -1.6),
];

/// A chip's keys: the full swing's, back only to halfway and through only to the arms out along the line.
fn chip_keys() -> [(f32, V, V, f32); 6] {
    let k = |i: usize, t: f32| (t, SWING[i].1, SWING[i].2, SWING[i].3);
    [k(0, 0.0), k(1, 0.15), k(2, 0.38), k(3, 0.58), k(6, 0.77), k(7, 0.93)]
}

/// The pose of a full swing (`reach` 1) or a chip (less) `t` of the way through its clip.
fn full_swing(reach: f32, t: f32, club_len: f32) -> Pose {
    let chip;
    let keys: &[(f32, V, V, f32)] = if reach < 1.0 {
        chip = chip_keys();
        &chip
    } else {
        &SWING
    };
    let k = keys.iter().rposition(|key| key.0 <= t).unwrap_or(0);
    let (a, b) = (keys[k], keys[(k + 1).min(keys.len() - 1)]);
    let f = if b.0 > a.0 { smooth((t - a.0) / (b.0 - a.0)) } else { 0.0 };
    let mix = |p: V, q: V| add(p, mul(sub(q, p), f));
    let (mut grip, shaft, turn) = (mix(a.1, b.1), norm(mix(a.2, b.2)), lerp(a.3, b.3, f));
    // a longer club (the driver) held a little higher at the ball, its head on the ground
    let near = 1.0 - (turn.abs() / 0.6).min(1.0);
    grip[1] += (club_len - 0.96) * 0.87 * near;
    let finish = (-turn - 0.6).max(0.0);
    let bend: f32 = 0.45 - finish * 0.22;
    Pose {
        hips: [turn * -0.025, 0.88 + finish * 0.04, -0.04],
        hip_turn: turn * 0.45,
        chest_turn: turn * 0.55,
        bend,
        lean: 0.06 - turn * 0.02,
        nod: 0.75 - finish * 0.55,
        // the right heel up and the knee turned through at the finish
        feet: [[0.23, 0.08 + finish * 0.07, finish * 0.05], [-0.23, 0.08, 0.0]],
        hands: [add(grip, mul(shaft, 0.09)), grip],
        club: Some((grip, shaft, club_len)),
    }
}

/// A tube from `a` to `b` (radii `ra`, `rb`), elliptical: `flat` times as deep (along `across` × the tube) as wide.
#[allow(clippy::too_many_arguments)]
fn tube(out: &mut Vec<Vert>, a: V, b: V, ra: f32, rb: f32, col: [f32; 3], across: V, flat: f32, sides: usize) {
    let ax = norm(sub(b, a));
    let mut e1 = sub(across, mul(ax, dot(across, ax)));
    if len(e1) < 1e-4 {
        e1 = cross(ax, [0.0, 0.0, 1.0]);
        if len(e1) < 1e-4 {
            e1 = cross(ax, [1.0, 0.0, 0.0]);
        }
    }
    let e1 = norm(e1);
    let e2 = cross(ax, e1);
    let c = [col[0], col[1], col[2], 1.0];
    let ring = |i: usize| {
        let t = TAU * i as f32 / sides as f32;
        let (s, k) = t.sin_cos();
        (add(mul(e1, k), mul(e2, s * flat)), norm(add(mul(e1, k * flat), mul(e2, s))))
    };
    let v = |p: V, n: V| {
        let mut v = Vert::new(p[0], p[1], p[2], 0.0, 0.0).col(c);
        v.normal = n;
        v
    };
    for i in 0..sides {
        let (o0, n0) = ring(i);
        let (o1, n1) = ring(i + 1);
        let (p0, p1, p2, p3) = (add(a, mul(o0, ra)), add(a, mul(o1, ra)), add(b, mul(o1, rb)), add(b, mul(o0, rb)));
        out.extend([v(p0, n0), v(p1, n1), v(p2, n1), v(p0, n0), v(p2, n1), v(p3, n0)]);
        // the ends closed
        out.extend([v(a, mul(ax, -1.0)), v(p1, mul(ax, -1.0)), v(p0, mul(ax, -1.0))]);
        out.extend([v(b, ax), v(p3, ax), v(p2, ax)]);
    }
}

/// A ball at `c`, its radii `r` along x, y and z.
fn ball(out: &mut Vec<Vert>, c: V, r: V, col: [f32; 3]) {
    let (seg, rings) = (10, 7);
    let col = [col[0], col[1], col[2], 1.0];
    let at = |i: usize, j: usize| {
        let (th, ph) = (TAU * i as f32 / seg as f32, PI * j as f32 / rings as f32);
        let n = [ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin()];
        let mut v = Vert::new(c[0] + n[0] * r[0], c[1] + n[1] * r[1], c[2] + n[2] * r[2], 0.0, 0.0).col(col);
        v.normal = norm([n[0] / r[0], n[1] / r[1], n[2] / r[2]]);
        v
    };
    for j in 0..rings {
        for i in 0..seg {
            let (a, b, cc, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            out.extend([a, b, cc, a, cc, d]);
        }
    }
}

/// Where the ball lies for a golfer at the ball (x to their right, z forward, metres): under the club's head at address.
/// The game keeps the ball at the golfer's own place until it is struck, so the body is drawn back from it by this much.
pub fn ball_spot(act: Act, club: i32, female: bool) -> [f32; 2] {
    let a = match act {
        Act::Putt | Act::Address { putt: true } => Act::Address { putt: true },
        Act::Swing { reach } => Act::Swing { reach },
        _ => Act::Address { putt: false },
    };
    let p = pose(a, 0.0, 0.0, club_length(club));
    let Some((grip, shaft, l)) = p.club else { return [0.0, 0.6] };
    let h = add(grip, mul(shaft, l));
    let tall = if female { 0.94 } else { 1.0 };
    [h[0] * tall, h[2] * tall]
}

fn club_length(club: i32) -> f32 {
    match club {
        13 => 0.86,
        0 => 1.12,
        1..=3 => 1.05,
        _ => 0.96,
    }
}

/// Who a golfer is, for their figure: their colours, their build (from their body's pictures: `height` and `girth` about
/// 1), what kind of golfer they are (`Golfer::kind`: 0x20 a pro, 0x40 the commissioner, 0x60 a CEO, 0x80 the heiress)
/// and their mood (-10..10).
#[derive(Clone, Copy, Debug)]
pub struct Who {
    pub look: Look,
    pub height: f32,
    pub girth: f32,
    pub kind: u8,
    pub mood: i32,
    /// A beard, as the golfer's own portrait shows (in the hair's colour).
    pub beard: bool,
}

impl Who {
    #[cfg(test)]
    pub fn plain(look: Look) -> Who {
        Who { look, height: 1.0, girth: 1.0, kind: 0, mood: 0, beard: false }
    }
}

/// A golfer's body in metres, standing at the origin, as `figure`, for someone of ordinary build and mood.
#[cfg(test)]
pub fn body(look: &Look, act: Act, t: f32, time: f32, club: i32) -> Vec<Vert> {
    figure(&Who::plain(*look), act, t, time, club)
}

/// A golfer's figure in metres, standing at the origin: x to their right, y up, z forward (`at_ball` acts: towards the
/// ball, the target on their left). `t` is how far through the clip, `time` seconds for idle movement, `club` the club
/// in hand (0..13, 13 the putter). Drawn as a cartoon is: a big head with a face that shows the mood, big hands and
/// feet, and the build of the golfer's own pictures.
pub fn figure(who: &Who, act: Act, t: f32, time: f32, club: i32) -> Vec<Vert> {
    let look = &who.look;
    let mut p = pose(act, t, time, club_length(club));
    // a club held at the side or dragged rests on the ground rather than through it
    if let Some((grip, shaft, l)) = p.club {
        if !act.at_ball() && grip[1] + shaft[1] * l < 0.02 {
            let dy = ((0.02 - grip[1]) / l).clamp(-0.999, 0.999);
            let h = (shaft[0] * shaft[0] + shaft[2] * shaft[2]).sqrt().max(1e-4);
            let k = (1.0 - dy * dy).sqrt() / h;
            p.club = Some((grip, [shaft[0] * k, dy, shaft[2] * k], l));
        }
    }
    let mut out = Vec::with_capacity(6000);
    let o = &mut out;
    let up = [0.0, 1.0, 0.0];
    let tall = if look.female { 0.94 } else { 1.0 } * who.height.clamp(0.85, 1.15);
    let g = who.girth.clamp(0.8, 1.35);
    let kind = who.kind & 0xe0;
    let shoe = [0.93, 0.93, 0.9];
    let belt = [0.12, 0.1, 0.09];
    let dark = [0.06, 0.05, 0.05];
    // the head drawn big, as a cartoon's is
    let hs = 1.32;

    // the hips and the spine: bent forward and leant, the shoulders turned on the hips
    let hip_right = yaw([1.0, 0.0, 0.0], -p.hip_turn);
    let chest_turn = p.hip_turn + p.chest_turn;
    let chest_right = yaw([1.0, 0.0, 0.0], -chest_turn);
    let chest_fwd = yaw([0.0, 0.0, 1.0], -chest_turn);
    let spine = norm(add(add(mul(up, p.bend.cos()), mul(chest_fwd, p.bend.sin())), mul(chest_right, p.lean.sin())));
    let hips = p.hips;
    let waist = add(hips, mul(spine, 0.12));
    let shoulders = add(hips, mul(spine, 0.5));
    let neck = add(shoulders, mul(spine, 0.07));
    let face = norm(add(mul(chest_fwd, p.nod.cos()), mul(up, -p.nod.sin())));
    let head = add(add(neck, mul(spine, 0.11 * hs)), mul(face, 0.02));
    let side = norm(cross(up, face));
    let crown = norm(cross(face, side));

    // legs: from the hip joints to the feet, the knees forward; big shoes
    for (k, s) in [(0usize, 1.0f32), (1, -1.0)] {
        let hip = add(hips, mul(hip_right, s * 0.1 * g));
        let foot = p.feet[k];
        let hint = add(yaw([0.0, 0.0, 1.0], -p.hip_turn * 0.5), [s * 0.1, 0.0, 0.0]);
        let (knee, ankle) = reach(hip, foot, 0.45, 0.43, hint);
        tube(o, hip, knee, 0.095 * g, 0.07 * g.sqrt(), look.pants, hip_right, 0.9, 8);
        tube(o, knee, ankle, 0.068 * g.sqrt(), 0.05, look.pants, hip_right, 0.9, 8);
        let toe = add(ankle, [s * 0.02, -0.05, 0.22]);
        let heel = add(ankle, [0.0, -0.05, -0.06]);
        tube(o, heel, toe, 0.062, 0.058, shoe, [0.0, 1.0, 0.0], 0.65, 8);
    }
    // the body: trousers to the belt, the shirt to the shoulders, wider than deep
    tube(o, sub(hips, mul(spine, 0.06)), waist, 0.16 * g, 0.155 * g, look.pants, hip_right, 0.68, 10);
    tube(o, waist, add(waist, mul(spine, 0.03)), 0.158 * g, 0.158 * g, belt, chest_right, 0.68, 10);
    tube(o, add(waist, mul(spine, 0.03)), shoulders, 0.158 * g, 0.19 * g.sqrt(), look.shirt, chest_right, 0.62, 10);
    ball(o, shoulders, [0.19 * g.sqrt(), 0.06, 0.12], look.shirt);
    // a businessman's belly and tie
    let belly = if matches!(kind, 0x40 | 0x60) { 1.0 } else { (g - 1.0).max(0.0) * 3.0 };
    if belly > 0.0 {
        ball(o, add(add(waist, mul(spine, 0.12)), mul(chest_fwd, 0.06 * g)), [0.16 * g, 0.16, 0.12 + 0.03 * belly], look.shirt);
    }
    if matches!(kind, 0x40 | 0x60) {
        let tie = if kind == 0x60 { [0.12, 0.16, 0.45] } else { [0.6, 0.1, 0.12] };
        let a = add(add(shoulders, mul(spine, -0.03)), mul(chest_fwd, 0.125 * g.sqrt()));
        let b = add(add(waist, mul(spine, 0.14)), mul(chest_fwd, 0.2 * g));
        tube(o, a, b, 0.028, 0.045, tie, chest_right, 0.25, 6);
        ball(o, a, [0.03, 0.028, 0.02], tie);
    }
    tube(o, shoulders, neck, 0.058, 0.055, look.skin, chest_right, 1.0, 8);
    if kind == 0x80 {
        // the heiress's pearls
        for i in 0..11 {
            let a = (i as f32 / 10.0 - 0.5) * 2.6;
            let at = add(add(add(neck, mul(spine, -0.035)), mul(chest_fwd, a.cos() * 0.075)), mul(chest_right, a.sin() * 0.075));
            ball(o, at, [0.014; 3], [0.97, 0.96, 0.92]);
        }
    }

    // the head and face: eyes that look where the head does, brows and a mouth that show the mood
    let mood = match act {
        Act::Happy | Act::Pump => 1.0,
        Act::Sad => -1.0,
        Act::Fail => -2.0,
        _ => (who.mood as f32 / 6.0).clamp(-1.0, 1.0),
    };
    let r = [0.095 * hs, 0.11 * hs, 0.1 * hs];
    let front = |d: f32, x: f32, y: f32| add(add(add(head, mul(face, d)), mul(side, x)), mul(crown, y));
    {
        ball(o, head, r, look.skin);
        // a big nose and ears
        ball(o, front(r[2] * 0.98, 0.0, -0.01), [0.026, 0.03, 0.03], look.skin);
        for s in [-1.0f32, 1.0] {
            ball(o, add(head, mul(side, s * r[0] * 0.98)), [0.016, 0.034, 0.026], look.skin);
        }
        for s in [-1.0f32, 1.0] {
            let eye = front(r[2] * 0.84, s * 0.045, 0.032);
            ball(o, eye, [0.027, 0.032, 0.02], [0.97, 0.97, 0.95]);
            ball(o, add(eye, mul(face, 0.016)), [0.013, 0.016, 0.01], dark);
            // the brows: raised when happy, down at the middle when cross, down at the outside when sad
            let tilt = if mood < -1.5 {
                -0.02
            } else if mood < -0.3 {
                0.015
            } else {
                0.0
            };
            let lift = if mood > 0.3 { 0.012 } else { 0.0 };
            let inner = front(r[2] * 0.86, s * 0.022, 0.072 + lift + tilt);
            let outer = front(r[2] * 0.8, s * 0.07, 0.07 + lift - tilt);
            tube(o, inner, outer, 0.009, 0.008, if look.female { look.hair } else { dark }, crown, 0.6, 5);
        }
        if who.beard {
            // round the jaw and over the lip, in the hair's colour
            ball(o, front(r[2] * 0.45, 0.0, -0.1), [r[0] * 0.8, r[1] * 0.34, r[2] * 0.6], look.hair);
            ball(o, front(r[2] * 0.95, 0.0, -0.03), [0.04, 0.012, 0.012], look.hair);
        }
        // the mouth: a smile, a line or a frown, made of beads along a curve
        for i in 0..7 {
            let u = i as f32 / 6.0 - 0.5;
            let bend = -(mood.clamp(-1.0, 1.0) * 0.022 * (u * u * 4.0 - 1.0));
            let at = front(r[2] * 0.9, u * 0.07, -0.05 - bend);
            ball(o, at, [0.009, 0.009, 0.007], [0.45, 0.12, 0.12]);
        }
        if kind == 0x60 {
            // the CEO's cigar
            let a = front(r[2] * 0.88, 0.03, -0.055);
            let b = add(add(a, mul(face, 0.09)), mul(side, 0.03));
            tube(o, a, b, 0.012, 0.012, [0.38, 0.22, 0.1], side, 1.0, 6);
            ball(o, b, [0.013; 3], [1.0, 0.45, 0.1]);
        }
    }
    // the hair and the hat: a cap (a visor for a pro), a woman's hair (the heiress's sun hat)
    let top = add(head, mul(crown, r[1] * 0.4));
    if look.female {
        ball(o, add(head, add(mul(face, -0.025), mul(crown, 0.02))), [r[0] * 1.12, r[1] * 1.02, r[2] * 1.1], look.hair);
        ball(o, add(head, add(mul(face, -0.16), mul(crown, -0.03))), [0.055, 0.075, 0.055], look.hair);
        if kind == 0x80 {
            tube(o, top, add(top, mul(crown, 0.08)), r[0] * 1.05, r[0] * 0.8, look.hat, face, 1.0, 12);
            tube(o, top, add(top, mul(crown, 0.012)), r[0] * 2.3, r[0] * 2.2, look.hat, face, 1.0, 14);
        } else {
            tube(o, add(head, mul(crown, 0.07)), add(head, mul(crown, 0.105)), r[0] * 1.13, r[0] * 1.12, look.hat, face, 1.0, 10);
            tube(o, front(r[2] * 0.95, 0.0, 0.09), front(r[2] * 1.9, 0.0, 0.082), 0.075, 0.065, look.hat, side, 0.15, 8);
        }
    } else {
        ball(o, add(head, mul(face, -0.03)), [r[0] * 1.03, r[1] * 0.98, r[2] * 1.02], look.hair);
        if kind == 0x20 {
            tube(o, add(head, mul(crown, 0.055)), add(head, mul(crown, 0.085)), r[0] * 1.06, r[0] * 1.05, look.hat, face, 1.0, 10);
        } else {
            ball(o, add(head, mul(crown, 0.055)), [r[0] * 1.1, r[1] * 0.68, r[2] * 1.1], look.hat);
            ball(o, add(head, mul(crown, r[1] * 0.62)), [0.018; 3], look.hat);
        }
        tube(o, front(r[2] * 0.85, 0.0, 0.06), front(r[2] * 2.0, 0.0, 0.045), 0.09, 0.075, look.hat, side, 0.12, 8);
    }

    // arms: short sleeves, then the arm and a big hand; the elbows down and out
    for (k, s) in [(0usize, 1.0f32), (1, -1.0)] {
        let shoulder = add(add(shoulders, mul(chest_right, s * 0.2 * g.sqrt())), mul(spine, -0.02));
        let hint = add(add([0.0, -1.0, 0.0], mul(chest_right, s * 0.6)), mul(chest_fwd, -0.3));
        let (elbow, wrist) = reach(shoulder, p.hands[k], 0.3, 0.28, hint);
        let sleeve = add(shoulder, mul(sub(elbow, shoulder), 0.5));
        tube(o, shoulder, sleeve, 0.062, 0.06, look.shirt, chest_right, 1.0, 8);
        tube(o, sleeve, elbow, 0.045, 0.04, look.skin, chest_right, 1.0, 8);
        tube(o, elbow, wrist, 0.04, 0.034, look.skin, chest_right, 1.0, 8);
        ball(o, wrist, [0.055, 0.062, 0.055], look.hands);
    }
    // the club: grip, shaft and head
    if let Some((grip, shaft, l)) = p.club {
        let top = sub(grip, mul(shaft, 0.06));
        let end = add(grip, mul(shaft, l));
        tube(o, top, add(grip, mul(shaft, 0.22)), 0.018, 0.015, [0.1, 0.1, 0.1], up, 1.0, 6);
        tube(o, add(grip, mul(shaft, 0.22)), end, 0.011, 0.008, [0.72, 0.73, 0.75], up, 1.0, 6);
        let face_dir = norm(cross(shaft, [1.0, 0.0, 0.0]));
        let toe = norm(cross(face_dir, shaft));
        let head_col = if club <= 2 { [0.15, 0.15, 0.17] } else { [0.78, 0.79, 0.82] };
        let size = if club <= 2 { [0.07, 0.042, 0.06] } else { [0.055, 0.025, 0.016] };
        let c = add(end, mul(toe, -size[0] * 0.8));
        tube(o, add(c, mul(toe, -size[0])), add(c, mul(toe, size[0])), size[1], size[1], head_col, face_dir, size[2] / size[1], 8);
    }
    if tall != 1.0 {
        for v in out.iter_mut() {
            v.pos = mul(v.pos, tall);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look::plain(&sg_core::bodies::Outfit { female: false, shirt: 1, pants: 2, skin: 0, hat: 3, hair: 0, alt_skin: 0 })
    }

    fn top(v: &[Vert]) -> f32 {
        v.iter().map(|p| p.pos[1]).fold(f32::MIN, f32::max)
    }

    #[test]
    fn a_standing_golfer_is_a_golfers_height_on_the_ground() {
        let v = body(&look(), Act::Stand, 0.0, 0.0, 0);
        assert!(v.len() % 3 == 0);
        let (lo, hi) = (v.iter().map(|p| p.pos[1]).fold(f32::MAX, f32::min), top(&v));
        assert!(lo.abs() < 0.04, "feet at {lo}");
        assert!((1.7..1.9).contains(&hi), "height {hi}");
        let wide = v.iter().map(|p| p.pos[0].abs()).fold(0.0, f32::max);
        assert!(wide < 0.45, "wide {wide}");
    }

    #[test]
    fn every_act_keeps_the_golfer_together_through_its_clip() {
        let acts = [
            Act::Stand,
            Act::Walk { tired: false },
            Act::Walk { tired: true },
            Act::Fidget,
            Act::Happy,
            Act::Sad,
            Act::Pump,
            Act::Fail,
            Act::Swing { reach: 1.0 },
            Act::Swing { reach: 0.55 },
            Act::Putt,
            Act::Address { putt: false },
            Act::Address { putt: true },
            Act::LineUp,
            Act::Point,
            Act::Lean(1.0),
            Act::LookUp,
            Act::Sit,
            Act::Shake,
        ];
        for a in acts {
            for k in 0..=10 {
                let v = body(&look(), a, k as f32 / 10.0, k as f32, 4);
                assert!(v.iter().all(|p| p.pos.iter().all(|c| c.is_finite())), "{a:?}");
                let far = v.iter().map(|p| len(p.pos)).fold(0.0, f32::max);
                assert!(far < 3.0, "{a:?} at {k}: {far}");
                assert!(v.iter().all(|p| p.pos[1] > -0.1), "{a:?} at {k} below the ground");
            }
        }
    }

    #[test]
    fn the_swing_goes_back_over_the_right_shoulder_and_through_to_the_left() {
        // the club's head: furthest point of the club's colour from the body's middle
        let head = |t: f32| {
            let v = body(&look(), Act::Swing { reach: 1.0 }, t, 0.0, 4);
            v.iter().filter(|p| (p.color[0] - 0.78).abs() < 0.005 && (p.color[2] - 0.82).abs() < 0.005).map(|p| p.pos).fold(
                [0.0; 3],
                |a, p| {
                    if len(p) > len(a) {
                        p
                    } else {
                        a
                    }
                },
            )
        };
        let (address, top, finish) = (head(0.0), head(0.6), head(0.98));
        assert!(address[1] < 0.3, "at the ball {address:?}");
        // (at the top, across the back of the neck towards the target)
        assert!(top[1] > 1.3 && top[0] < 0.0 && top[2] < 0.0, "at the top {top:?}");
        // (at the finish, hanging down the back)
        assert!(finish[2] < -0.2 && finish[1] > 0.6, "at the finish {finish:?}");
    }

    #[test]
    fn colours_come_from_the_palettes_ranges() {
        let mut pal = [0u8; 768];
        for i in 0..20 {
            pal[i * 3] = 200; // the shirt red
            pal[(20 + i) * 3 + 2] = 200; // the trousers blue
        }
        let l = Look::from_palette(&pal, false);
        assert!(l.shirt[0] > 0.7 && l.shirt[2] < 0.1);
        assert!(l.pants[2] > 0.7 && l.pants[0] < 0.1);
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    /// Writes swing frames' triangles for looking at outside the game: SG_GOLFER_DUMP=DIR cargo test dump_swing -- --ignored
    #[test]
    #[ignore]
    fn dump_swing() {
        let Ok(dir) = std::env::var("SG_GOLFER_DUMP") else { return };
        let look = Look::plain(&sg_core::bodies::Outfit { female: false, shirt: 0, pants: 2, skin: 1, hat: 1, hair: 0, alt_skin: 1 });
        let o = |female: bool, shirt: u8, pants: u8, skin: u8, hat: u8| sg_core::bodies::Outfit {
            female,
            shirt,
            pants,
            skin,
            hat,
            hair: 2,
            alt_skin: skin,
        };
        let cast = [
            ("joe", Who { mood: 8, ..Who::plain(Look::plain(&o(false, 1, 2, 0, 3))) }, Act::Happy, 0.25),
            ("ceo", Who { kind: 0x60, girth: 1.2, ..Who::plain(Look::plain(&o(false, 0, 6, 1, 6))) }, Act::Stand, 0.0),
            ("heiress", Who { kind: 0x80, mood: 5, ..Who::plain(Look::plain(&o(true, 7, 0, 0, 4))) }, Act::Stand, 0.0),
            ("pro", Who { kind: 0x20, mood: 3, ..Who::plain(Look::plain(&o(false, 2, 0, 2, 0))) }, Act::Pump, 0.0),
            ("cross", Who { mood: -8, ..Who::plain(Look::plain(&o(false, 4, 3, 1, 2))) }, Act::Fail, 0.8),
            ("sad", Who { mood: -6, girth: 0.85, height: 1.1, ..Who::plain(Look::plain(&o(true, 3, 5, 3, 1))) }, Act::Sad, 0.5),
        ];
        for (k, (name, who, act, t)) in cast.iter().enumerate() {
            let v = figure(who, *act, *t, 0.0, 4);
            let s: String = v
                .iter()
                .map(|p| {
                    format!(
                        "{} {} {} {} {} {} {} {} {}\n",
                        p.pos[0], p.pos[1], p.pos[2], p.normal[0], p.normal[1], p.normal[2], p.color[0], p.color[1], p.color[2]
                    )
                })
                .collect();
            std::fs::write(format!("{dir}/cast-{k:02}-{name}.txt"), s).unwrap();
        }
        let acts = [("swing", Act::Swing { reach: 1.0 }, 4), ("chip", Act::Swing { reach: 0.55 }, 9), ("putt", Act::Putt, 13)];
        for (name, act, club) in acts {
            for k in 0..=20 {
                let v = body(&look, act, k as f32 / 20.0, 0.0, club);
                let s: String = v
                    .iter()
                    .map(|p| {
                        format!(
                            "{} {} {} {} {} {} {} {} {}\n",
                            p.pos[0], p.pos[1], p.pos[2], p.normal[0], p.normal[1], p.normal[2], p.color[0], p.color[1], p.color[2]
                        )
                    })
                    .collect();
                std::fs::write(format!("{dir}/{name}-{k:02}.txt"), s).unwrap();
            }
        }
    }
}
