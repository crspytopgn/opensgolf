//! The showcase fly-over (the port's own; docs/PLAYING.md, "Showcase"): a key (Shift+O, remappable, controls.rs) hides the
//! interface and glides the camera round the course hole by hole, from the back tee down to the green with a short pause on
//! each and a caption with the hole's number, par and length, going round the holes until a key, a click or a tap ends it;
//! the camera, the zoom and the tool in hand then come back as they were. Golfers play on meanwhile. It only moves the
//! camera, as the hole keys do, so it works the same in Classic and Enhanced graphics, and nothing changes until it is used.
use crate::app::{App, Screen};
use crate::gfx::Gfx;
use crate::ui::{rgba, Screen as Ui, F_MANUAL20};
use sg_core::lang::{tr, trf};
use sg_core::terrain::TILE_SIZE;

/// Seconds the camera rests on a tee and on a green.
const TEE_PAUSE: f32 = 1.6;
const GREEN_PAUSE: f32 = 2.2;
/// Seconds the caption takes to fade in or out.
const FADE: f32 = 0.6;

/// An open hole for the fly-over: its number, back tee and green (world units).
pub type HoleSpot = (usize, (f32, f32), (f32, f32));

/// One stretch of the fly-over: the camera from one place to another (the same place: a pause) in `secs`, showing `hole`
/// (0 on the way from where the player was to the first tee).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub secs: f32,
    pub hole: usize,
}

/// How long a glide over `d` world units takes: longer for longer holes, never a dash nor a crawl.
fn glide_secs(d: f32) -> f32 {
    (1.4 + 0.42 * d / TILE_SIZE).clamp(2.4, 9.0)
}

/// The fly-over from the camera at `cam` over `holes` (hole number, tee and green in world units, in play order): the way to
/// the first tee, then the loop it goes round (tee pause, down the hole, green pause, on to the next tee, back to the first
/// after the last).
pub fn plan(cam: (f32, f32), holes: &[HoleSpot]) -> (Leg, Vec<Leg>) {
    let dist = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).hypot(a.1 - b.1);
    let first = holes.first().map(|h| h.1).unwrap_or(cam);
    let to_first = Leg { from: cam, to: first, secs: glide_secs(dist(cam, first)), hole: 0 };
    let mut cycle = Vec::new();
    for (i, &(n, tee, green)) in holes.iter().enumerate() {
        cycle.push(Leg { from: tee, to: tee, secs: TEE_PAUSE, hole: n });
        cycle.push(Leg { from: tee, to: green, secs: glide_secs(dist(tee, green)), hole: n });
        cycle.push(Leg { from: green, to: green, secs: GREEN_PAUSE, hole: n });
        let next = holes[(i + 1) % holes.len()].1;
        cycle.push(Leg { from: green, to: next, secs: glide_secs(dist(green, next)), hole: 0 });
    }
    (to_first, cycle)
}

/// Where the camera is `t` seconds into the fly-over, which hole it shows (0 between holes) and how far the caption is
/// faded in (0 .. 1).
pub fn at(first: &Leg, cycle: &[Leg], t: f32) -> ((f32, f32), usize, f32) {
    let ease = |k: f32| k * k * k * (k * (k * 6.0 - 15.0) + 10.0);
    let along = |l: &Leg, s: f32| {
        let k = ease((s / l.secs).clamp(0.0, 1.0));
        (l.from.0 + (l.to.0 - l.from.0) * k, l.from.1 + (l.to.1 - l.from.1) * k)
    };
    if t < first.secs || cycle.is_empty() {
        return (along(first, t), 0, 0.0);
    }
    let total: f32 = cycle.iter().map(|l| l.secs).sum();
    let mut s = (t - first.secs) % total.max(1e-3);
    for (i, l) in cycle.iter().enumerate() {
        if s < l.secs || i == cycle.len() - 1 {
            // the caption: up over the hole's three legs (tee, the hole, green), fading in and out at their ends
            let caption = if l.hole == 0 {
                0.0
            } else {
                let into: f32 = cycle[..i].iter().rev().take_while(|p| p.hole == l.hole).map(|p| p.secs).sum::<f32>() + s;
                let left: f32 = cycle[i..].iter().take_while(|p| p.hole == l.hole).map(|p| p.secs).sum::<f32>() - s;
                (into / FADE).min(left / FADE).clamp(0.0, 1.0)
            };
            return (along(l, s), l.hole, caption);
        }
        s -= l.secs;
    }
    (first.to, 0, 0.0)
}

/// The fly-over under way: when it started and what comes back when it ends.
#[derive(Clone, Debug)]
pub struct Showcase {
    start: f64,
    /// SG_SHOWCASE: held this many seconds in, for stills.
    held: Option<f32>,
    cam: (f32, f32),
    zoom: f32,
    edit: bool,
}

impl App {
    /// The open holes in play order: (number, back tee, green) in world units.
    fn showcase_holes(&self) -> Vec<HoleSpot> {
        (1..=18)
            .filter_map(|n| {
                let h = self.club.holes.get(n).filter(|h| h.par != 0)?;
                let tee = self.units_to_world(h.back.0 * 1024 + 512, h.back.1 * 1024 + 512);
                let green = self.units_to_world(h.pin.0 * 1024 + 512, h.pin.1 * 1024 + 512);
                Some((n, tee, green))
            })
            .collect()
    }

    /// The showcase key: the fly-over starts (or, under way, ends). Needs an open hole.
    pub fn toggle_showcase(&mut self) {
        if self.showcase.is_some() {
            self.end_showcase();
            return;
        }
        if self.showcase_holes().is_empty() {
            self.show_toast(tr("STR_SHOWCASE_NO_HOLES"));
            return;
        }
        self.showcase = Some(Showcase { start: self.clock, held: None, cam: (self.cam_x, self.cam_z), zoom: self.zoom, edit: self.edit });
        // the closest of the original's three zoom levels, as a golfer would see the hole
        self.zoom = crate::app::ZOOM_UNIT * 4.0;
        self.edit = false;
        self.xs.follow = None;
        self.enh.pan_left = (0.0, 0.0);
        self.enh.pan_vel = (0.0, 0.0);
    }

    /// Ends the fly-over: the camera, the zoom and the tool come back as they were. True when one was under way (the key,
    /// click or tap that ended it does nothing else).
    pub fn end_showcase(&mut self) -> bool {
        let Some(s) = self.showcase.take() else { return false };
        (self.cam_x, self.cam_z) = s.cam;
        self.zoom = s.zoom;
        self.edit = s.edit;
        true
    }

    /// Test hook: SG_SHOWCASE=T starts the fly-over held T seconds in (for stills; docs/PLAYING.md).
    pub fn showcase_test_hook(&mut self) {
        let Some(t) = std::env::var("SG_SHOWCASE").ok().and_then(|v| v.parse::<f64>().ok()) else { return };
        if self.showcase.is_none() {
            self.toggle_showcase();
        }
        if let Some(s) = &mut self.showcase {
            s.held = Some(t as f32);
        }
    }

    /// Seconds into the fly-over.
    fn showcase_secs(&self, s: &Showcase) -> f32 {
        s.held.unwrap_or((self.clock - s.start) as f32)
    }

    /// Once a frame: the camera where the fly-over has got to. It ends by itself when another screen opens.
    pub fn showcase_frame(&mut self) {
        let Some(s) = &self.showcase else { return };
        if self.screen != Screen::Play || self.photo.is_some() || self.editor.is_some() {
            self.end_showcase();
            return;
        }
        let holes = self.showcase_holes();
        if holes.is_empty() {
            self.end_showcase();
            return;
        }
        let (first, cycle) = plan(s.cam, &holes);
        let ((x, z), _, _) = at(&first, &cycle, self.showcase_secs(s));
        self.cam_x = x;
        self.cam_z = z;
    }

    /// The fly-over's caption: the hole's number, par and length low on the screen, in the game's own font with a shadow.
    pub fn draw_showcase_caption(&self, g: &mut Gfx) {
        let Some(s) = &self.showcase else { return };
        let holes = self.showcase_holes();
        let (first, cycle) = plan(s.cam, &holes);
        let (_, n, k) = at(&first, &cycle, self.showcase_secs(s));
        if n == 0 || k <= 0.0 {
            return;
        }
        let Some(h) = self.club.holes.get(n) else { return };
        let text = trf("STR_SHOWCASE_HOLE", &[("HOLE", &n.to_string()), ("PAR", &h.par.to_string()), ("YARDS", &h.length.to_string())]);
        let ui = Ui::new(self.draw_w, self.draw_h);
        let w = F_MANUAL20.width(&text);
        let (x, y) = ((800.0 - w) * 0.5, 548.0);
        ui.fill(g, x - 14.0, y - 6.0, w + 28.0, 32.0, rgba(0.0, 0.0, 0.0, 0.42 * k));
        ui.put(g, F_MANUAL20, x + 1.0, y + 1.0, &text, rgba(0.0, 0.0, 0.0, 0.8 * k));
        ui.put(g, F_MANUAL20, x, y, &text, rgba(1.0, 1.0, 0.9, k));
        g.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course() -> Vec<HoleSpot> {
        vec![(1, (0.0, 0.0), (0.0, 1500.0)), (2, (300.0, 1500.0), (2000.0, 1500.0)), (4, (2000.0, 0.0), (500.0, 0.0))]
    }

    #[test]
    fn the_flight_goes_tee_to_green_hole_by_hole() {
        let (first, cycle) = plan((-800.0, -800.0), &course());
        assert_eq!(first.to, (0.0, 0.0));
        assert_eq!(cycle.len(), 12);
        // it starts where the camera was
        assert_eq!(at(&first, &cycle, 0.0).0, (-800.0, -800.0));
        // then rests on the first tee with the caption up
        let t = first.secs + TEE_PAUSE * 0.5;
        let (p, n, _) = at(&first, &cycle, t);
        assert_eq!((p, n), ((0.0, 0.0), 1));
        // down the hole: on the line from tee to green
        let t = first.secs + TEE_PAUSE + cycle[1].secs * 0.5;
        let (p, n, k) = at(&first, &cycle, t);
        assert_eq!(n, 1);
        assert!(p.0.abs() < 1e-3 && (p.1 - 750.0).abs() < 1.0);
        assert_eq!(k, 1.0);
        // the holes in order, then round again
        let holes: Vec<usize> = cycle.iter().map(|l| l.hole).filter(|&h| h != 0).collect();
        assert_eq!(holes, [1, 1, 1, 2, 2, 2, 4, 4, 4]);
        assert_eq!(cycle.last().unwrap().to, (0.0, 0.0));
        let total: f32 = cycle.iter().map(|l| l.secs).sum();
        let (a, _, _) = at(&first, &cycle, first.secs + 1.0);
        let (b, _, _) = at(&first, &cycle, first.secs + 1.0 + total);
        assert!((a.0 - b.0).abs() < 1e-2 && (a.1 - b.1).abs() < 1e-2);
    }

    #[test]
    fn the_caption_fades_between_holes() {
        let (first, cycle) = plan((0.0, 0.0), &course());
        // on the way to the first tee and between holes: no caption
        assert_eq!(at(&first, &cycle, 0.1).2, 0.0);
        let between = first.secs + TEE_PAUSE + cycle[1].secs + GREEN_PAUSE + cycle[3].secs * 0.5;
        assert_eq!(at(&first, &cycle, between).1, 0);
        // fading in on the tee
        let k = at(&first, &cycle, first.secs + FADE * 0.5).2;
        assert!(k > 0.4 && k < 0.6);
    }

    #[test]
    fn glides_are_neither_a_dash_nor_a_crawl() {
        assert_eq!(glide_secs(0.0), 2.4);
        assert_eq!(glide_secs(100.0 * TILE_SIZE), 9.0);
        assert!(glide_secs(12.0 * TILE_SIZE) > 5.0);
    }
}
