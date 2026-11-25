//! Lockstep: in a shared game every player's computer runs the same simulation from the same actions, so nothing that
//! only one computer sees may reach it: not the screen (the view, the zoom, the pointer, the ticker, where golfers stand
//! on the screen), not the art (its animation frame counts) and not the clock of the drawing. The single-player game
//! hands the simulation all of these (they are the "what the game around the simulation hands it" actions of
//! sg_core::action, logged for replays); in lockstep each one is replaced by a fixed value, the same everywhere:
//!
//! * the tick flags: nothing busy, no advisor, not close, no message, no snapshots ([`FLAGS`]);
//! * the golfers' screen places: every golfer off the screen;
//! * the pointer the pro's employee follows: none;
//! * the tees' arrows: set in the tick, as for the unturned view ([`CAMERA`]), not by the drawing;
//! * the weeds' frame count: the original's ([`WEED_FRAMES`]);
//! * a new plant's growing: [`SPROUT_TICKS`] game ticks, counted in the tick (not by the frames drawn);
//! * the course brought up to date with the land: at the start of every tick, not when a frame or report asks.
//!
//! The hooks outside this folder are one line each behind the `multiplayer` feature (replay.rs: act, tick_flags,
//! screen_hook, sync_hook, tick_begin; app.rs: the staff's world, update_tee_arrows, update_tree_growth).

use crate::app::App;
use sg_core::action::TickFlags;
use std::cell::Cell;

/// The tick flags of a shared game.
// (the arrows on turned views are drawn wrong for now: the renderer should keep its own picture of them)
pub const FLAGS: TickFlags = TickFlags { busy: false, advisor: false, close: false, message: false, snapshots: false };
/// The tees' arrows of a shared game: as for the view unturned.
pub const CAMERA: i32 = 0;
/// The weeds' frames (the original's crabgrass and theme weeds have 8).
pub const WEED_FRAMES: i32 = 8;
/// Game ticks a newly planted tree, scenic tree or flower bed grows before its tile's mark ends (about the original's 15
/// frames at one frame a tick).
pub const SPROUT_TICKS: u32 = 15;

thread_local! {
    static ON: Cell<bool> = Cell::new(std::env::var_os("SG_LOCKSTEP").is_some_and(|v| v != "0"));
}

/// Whether the game is a shared one (SG_LOCKSTEP=1 for tests until joining a club sets it).
pub fn on() -> bool {
    ON.with(|c| c.get())
}

pub fn set(on: bool) {
    ON.with(|c| c.set(on));
}

thread_local! {
    static LOCAL: Cell<u32> = Cell::new(
        // (miniquad's clock: the standard library's has no time in a browser)
        ((miniquad::date::now().fract() * 1e9) as u32) | 1,
    );
}

/// A random 0..n for the player's own interface in a shared game (the brush's variant): its own generator, so the shared
/// one (the exe's, which the simulation steps) moves only with what everyone does.
pub fn local_roll(n: i32) -> i32 {
    LOCAL.with(|c| {
        let mut r = sg_core::land::ExeRng { state: c.get() };
        let v = r.below(n);
        c.set(r.state);
        v
    })
}

/// The staff's world inputs that come from outside the simulation: the pointer's tile and the weeds' frame count.
pub fn staff_inputs(weed_frames: i32, clicked: Option<(i32, i32)>) -> (i32, Option<(i32, i32)>) {
    if on() {
        (WEED_FRAMES, None)
    } else {
        (weed_frames, clicked)
    }
}

/// Test hook SG_MP_STEP=S (seconds): the play a frame of a scripted run may advance, so the drawing runs between ticks as
/// it does in a real game. Unset: all of it at once.
fn test_step() -> Option<f64> {
    static STEP: std::sync::OnceLock<Option<f64>> = std::sync::OnceLock::new();
    *STEP.get_or_init(|| std::env::var("SG_MP_STEP").ok().and_then(|v| v.parse().ok()).filter(|&s: &f64| s > 0.0))
}

/// How far the play may go this frame.
pub fn frame_target(sim_time: f64, time: f64) -> f64 {
    match test_step() {
        Some(s) => time.min(sim_time + s),
        None => time,
    }
}

/// A scripted still waits while SG_MP_STEP has play left to run.
pub fn test_waiting(app: &App) -> bool {
    test_step().is_some() && app.sim_time + 1e-9 < app.time && app.great_shot.is_none()
}

impl App {
    /// A player's action in a shared game: sent to the server (Some(true)), refused (Some(false)), or None when it is
    /// the player's own and is applied at once as usual.
    pub(crate) fn lockstep_act(&mut self, a: &sg_core::action::Action) -> Option<bool> {
        use super::session::{route, Route, ToServer};
        if !on() || !self.rec.depth_zero() {
            return None;
        }
        let s = self.mp.session.as_mut()?;
        match route(a) {
            Route::Local => None,
            Route::Shared => {
                s.send(ToServer::Act(a.clone()));
                Some(true)
            }
            Route::Refused => {
                self.show_toast(sg_core::lang::tr("STR_MP_CANT_IN_SHARED"));
                Some(false)
            }
        }
    }

    /// Plays one game tick of a shared game (its clock decides when: session.rs).
    pub fn lockstep_step(&mut self) {
        self.tick_acc = sg_core::flight::TICK_MS as f64 / 1000.0;
        self.step_game(0.0);
    }

    /// The golfers' screen places in a shared game: all off the screen.
    pub(crate) fn lockstep_screen(&mut self) {
        for g in self.club.g.iter_mut() {
            g.sx = -1;
            g.sy = -1;
        }
    }

    /// The start of a game tick in a shared game (inside the tick, so nothing here is logged): plants grow a tick, those
    /// done end their tiles' mark, and the course is brought up to date with the land.
    pub(crate) fn lockstep_tick(&mut self) {
        if !on() {
            return;
        }
        // the actions the server ordered for this tick
        let due = self.mp.session.as_mut().map(|s| s.due(self.game_tick)).unwrap_or_default();
        for d in due {
            self.apply(&d.action);
        }
        // (a golfer who arrived since the last drawn frame has the screen place he was made with otherwise)
        self.lockstep_screen();
        let mut done: Vec<(i32, i32)> = Vec::new();
        for (&at, c) in self.sprouts.iter_mut() {
            *c = c.saturating_add(1);
            if *c >= SPROUT_TICKS {
                done.push(at);
            }
        }
        if !done.is_empty() {
            // (the order is the map's, which differs run to run: sprouts_grown doesn't depend on it, but keep it sorted)
            done.sort_unstable();
            for at in &done {
                self.sprouts.remove(at);
            }
            self.sprouts_grown(&done);
        }
        self.sync_course();
        if self.tee_arrows_change(CAMERA) {
            self.tee_arrows(CAMERA);
        }
    }

    /// The drawing of growing plants in a shared game: their frames follow the tick's counters, and nothing the drawing
    /// sees goes back to the simulation (update_tree_growth's own does both).
    pub(crate) fn lockstep_tree_growth(&mut self) {
        let (mut trees, mut objects) = (false, false);
        for pi in 0..self.props.len() {
            let p = &self.props[pi];
            let (Some((a, b, _)), Some(bi), tree) = (p.sprout, p.body, p.tree) else { continue };
            let n = self.sprites[bi].s.frames_per_view.max(1);
            let frame = match self.sprouts.get(&(a, b)) {
                Some(&c) => ((c as i64 * (n - 1) as i64) / SPROUT_TICKS as i64).min((n - 1) as i64) as i32,
                None => {
                    // grown: drawn again as a grown plant
                    trees |= tree;
                    objects |= !tree;
                    n - 1
                }
            };
            self.props[pi].frame = frame;
        }
        if trees {
            self.refresh_trees();
        }
        if objects {
            self.props.retain(|p| !p.object);
            self.add_land_objects();
        }
        for p in self.props.iter_mut().filter(|p| p.object) {
            let (Some((a, b)), Some(bi)) = (p.grow_tile, p.body) else { continue };
            let t = &self.staff_tiles;
            let i = (b * t.w + a) as usize;
            let n = self.sprites[bi].s.frames_per_view;
            let c = t.counter.get(i).copied().unwrap_or(0) as i32;
            let washing = a < t.w && t.flags.get(i).is_some_and(|f| f & sg_core::staff::WORKED != 0);
            p.frame = if washing && c < n { c } else { 0 };
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::session::{Room, Session, ToGame, LEAD_MS};
    use super::*;
    use sg_core::action::Action;
    use sg_core::terrain::Terrain;

    /// A flat fairway with hole 1 planned from 10,10 to 10,30 (app.rs's building_game), in a shared game as `me`.
    pub(crate) fn shared_game(me: u32) -> App {
        let mut a = App::with_clock(std::path::PathBuf::new(), 4242);
        let n = sg_core::mapsize::ORIGINAL;
        let tiles = (n * n) as usize;
        a.terrain = Terrain { w: n, h: n, ty: vec![2; tiles], variation: vec![0; tiles], set: vec![0; tiles], ..Default::default() };
        a.terrain.corner = vec![1; ((n + 1) * (n + 1)) as usize];
        a.terrain.path_kind = vec![0; tiles];
        a.terrain.wall_mask = vec![0; tiles];
        a.terrain.paint(10, 10, 0, 0);
        a.terrain.paint(10, 30, 1, 0);
        let h = &mut a.club.holes[1];
        h.back = (10, 10);
        h.fwd = (10, 10);
        h.pin = (10, 30);
        a.sync_course();
        a.game_tick = 1;
        a.mp.session = Some(Session::new(me));
        a
    }

    /// The terrain brush `ty` (app.rs PAINT: 4 rough, 13 woods, which plants trees) on these tiles.
    pub(crate) fn paint(ty: i32, tiles: &[(i32, i32)]) -> Action {
        Action::Paint { ty, vbyte: 0, turn: None, look: None, tiles: tiles.to_vec(), tee: 0 }
    }

    /// Two games, the server between them: messages reach B up to `delay` ms late, and B plays its ticks in bursts.
    fn play(delay: f64, burst: f64) -> (App, App, Room) {
        set(true);
        let (mut a, mut b) = (shared_game(1), shared_game(2));
        let mut room = Room::new(1, 0.0);
        let mut to_a: Vec<ToGame> = Vec::new();
        let mut to_b: Vec<(f64, ToGame)> = Vec::new();
        let mut ms = 0.0;
        let mut next_burst = 0.0;
        // A asks for normal speed, opens the hole and paints; B paints too
        a.act(Action::Speed(1));
        let script: Vec<(f64, u32, Action)> = vec![
            (400.0, 1, Action::OpenHole),
            (900.0, 1, paint(13, &[(20, 20), (21, 20), (22, 20)])),
            (905.0, 2, paint(4, &[(20, 21), (21, 21)])),
            (5000.0, 2, Action::Speed(4)),
            (7000.0, 1, paint(13, &[(30, 30), (31, 30)])),
        ];
        let mut script = script.into_iter().peekable();
        while ms < 12_000.0 {
            while script.peek().is_some_and(|s| s.0 <= ms) {
                let (_, who, act) = script.next().unwrap();
                (if who == 1 { &mut a } else { &mut b }).act(act);
            }
            for (who, g) in [(1, &mut a), (2, &mut b)] {
                for m in g.mp.session.as_mut().unwrap().take_outbox() {
                    for out in room.handle(ms, who, m) {
                        to_b.push((ms + delay, out.clone()));
                        to_a.push(out);
                    }
                }
            }
            // A gets everything at once
            for m in to_a.drain(..) {
                let t = a.game_tick;
                a.mp.session.as_mut().unwrap().receive(m, t);
            }
            // B gets it `delay` later
            while to_b.first().is_some_and(|m| m.0 <= ms) {
                let (_, m) = to_b.remove(0);
                let t = b.game_tick;
                b.mp.session.as_mut().unwrap().receive(m, t);
            }
            let to = a.mp.session.as_ref().unwrap().may_run_to(ms);
            while a.game_tick < to {
                a.lockstep_step();
            }
            if ms >= next_burst {
                next_burst += burst;
                let to = b.mp.session.as_ref().unwrap().may_run_to(ms);
                while b.game_tick < to {
                    b.lockstep_step();
                }
            }
            ms += 10.0;
        }
        let to = a.game_tick.max(b.game_tick);
        while a.game_tick < to {
            a.lockstep_step();
        }
        while b.game_tick < to {
            b.lockstep_step();
        }
        set(false);
        (a, b, room)
    }

    #[test]
    fn two_shared_games_stay_the_same() {
        let (a, b, room) = play(300.0, 250.0);
        assert!(a.game_tick > 100, "the clock ran ({} ticks)", a.game_tick);
        assert_eq!(a.mp.session.as_ref().unwrap().late, None);
        assert_eq!(b.mp.session.as_ref().unwrap().late, None);
        assert!(room.log.len() >= 7, "every action went through the server");
        assert_eq!(a.fingerprint().total(), b.fingerprint().total(), "the same game on both");
        assert_eq!(a.terrain.ty, b.terrain.ty);
        assert_eq!(a.terrain.ty[a.terrain.tile_index(20, 21)], 4, "B's paint reached A");
        assert_eq!(b.terrain.ty[b.terrain.tile_index(30, 30)], 13, "A's reached B");
    }

    #[test]
    fn an_action_too_late_is_caught() {
        // more delay than the lead: B has played past the tick an action was for
        let (_, b, _) = play(LEAD_MS * 3.0, 10.0);
        assert!(b.mp.session.as_ref().unwrap().late.is_some());
    }

    #[test]
    fn off_unless_asked_and_then_fixed() {
        set(false);
        assert_eq!(staff_inputs(5, Some((1, 2))), (5, Some((1, 2))));
        set(true);
        assert_eq!(staff_inputs(5, Some((1, 2))), (WEED_FRAMES, None));
        set(false);
    }
}
