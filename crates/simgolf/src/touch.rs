//! Touch controls for phones and tablets (the browser build and the Android app). The original is a mouse and keyboard game, so this is the port's
//! own layer: it turns touches into the mouse clicks and keys the game already understands.
//!
//! * One finger: a tap is a left click; dragging pans the course (or paints with the current tool); holding still for half a
//!   second is a right click (undo, remove, close a screen). With the raise / lower tool armed, holding still over the course
//!   lowers the ground instead (the desktop's Shift+click, which a tablet has no key for), and dragging on from there keeps
//!   lowering.
//! * Two fingers: dragging pans, pinching steps through the three zoom levels, twisting turns the view a quarter.
//! * A few round touch buttons down the right edge stand in for keys the screen has no button for (game speed, repeat the
//!   last message, turn the building, open the hole); they show only once the screen has been touched, Turn and Open hole
//!   only while they have something to do.
//! * When a name is typed (rename, save) the phone's keyboard is raised: through the page in a browser (web/opensgolf.js),
//!   through miniquad on Android.

use crate::app::*;
use crate::gfx::Gfx;
use crate::ui::{rgb, rgba, Screen as Ui};
use crate::Stage;
use miniquad::{EventHandler, KeyCode, KeyMods, MouseButton, TouchPhase};
use sg_core::lang::tr;

/// Drawable pixels a finger may wander before a press becomes a drag.
const SLOP: f32 = 12.0;
/// Seconds a still finger takes to become a right click.
const LONG_PRESS: f64 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Mode {
    /// No finger down.
    #[default]
    Idle,
    /// One finger down and still: a tap, a drag or a long press yet to be decided.
    Pending,
    /// One finger driving the mouse (its button is down).
    Mouse,
    /// Two fingers: pan, pinch and twist.
    Gesture,
    /// The touch has been used up (a long press, a touch button); the rest of it is ignored.
    Spent,
}

/// A finger: its id, where it went down and where it is now.
type Finger = (u64, (f32, f32), (f32, f32));

#[derive(Default)]
pub struct Touch {
    /// The screen has been touched: the touch buttons show from then on.
    pub used: bool,
    mode: Mode,
    /// Fingers down: id, start and current position.
    fingers: Vec<Finger>,
    started: f64,
    /// The two-finger gesture's last midpoint, the spread and the angle the next step is measured from.
    mid: (f32, f32),
    spread: f32,
    angle: f32,
    /// Whether the phone's keyboard is up.
    keyboard: bool,
    /// A click waiting for the pointer's move to reach the screens (they act on what is under the pointer as of the last
    /// frame): the button, where, and frames left.
    queued: Option<(MouseButton, f32, f32, u8)>,
    /// A long press lowering the ground: the Shift it holds down is let go with the finger.
    lowering: bool,
    /// The on-screen keyboard's Shift (capitals for the next letter).
    osk_shift: bool,
    /// The touch button under the finger: it acts when the finger lifts on it (sliding off lets it be).
    held: Option<Btn>,
    /// The button last let go and when, lit for a moment.
    flash: Option<(Btn, f64)>,
}

/// The on-screen keyboard (browser on a touch screen, while a name is typed; issue #50): Safari on an iPad raises its own
/// keyboard only for some taps, and a name box closes only with Enter or Esc. Rows of keys across the bottom of the
/// 800 x 600 screen, below every name box.
const OSK_ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl'", "zxcvbnm,.-"];
const OSK_TOP: f32 = 420.0;
const OSK_H: f32 = 30.0;
const OSK_GAP: f32 = 4.0;

/// What an on-screen key does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum OskKey {
    Char(char),
    Shift,
    Back,
    Space,
    Cancel,
    Ok,
}

/// Every key and its box in virtual pixels.
fn osk_keys() -> Vec<(OskKey, (f32, f32, f32, f32))> {
    let mut v = Vec::new();
    let w = 66.0;
    for (r, row) in OSK_ROWS.iter().enumerate() {
        let y = OSK_TOP + r as f32 * (OSK_H + OSK_GAP);
        let n = row.chars().count() as f32;
        let x0 = 400.0 - (n * (w + OSK_GAP) - OSK_GAP) * 0.5 - if r == 3 { 40.0 } else { 0.0 };
        for (i, c) in row.chars().enumerate() {
            v.push((OskKey::Char(c), (x0 + i as f32 * (w + OSK_GAP), y, w, OSK_H)));
        }
        if r == 3 {
            v.push((OskKey::Back, (x0 + n * (w + OSK_GAP), y, 76.0, OSK_H)));
        }
    }
    let y = OSK_TOP + 4.0 * (OSK_H + OSK_GAP);
    v.push((OskKey::Shift, (30.0, y, 110.0, OSK_H)));
    v.push((OskKey::Cancel, (146.0, y, 110.0, OSK_H)));
    v.push((OskKey::Space, (262.0, y, 276.0, OSK_H)));
    v.push((OskKey::Ok, (544.0, y, 226.0, OSK_H)));
    v
}

fn osk_hit(vx: f32, vy: f32) -> Option<OskKey> {
    osk_keys().into_iter().find(|(_, (x, y, w, h))| vx >= *x && vx < x + w && vy >= *y && vy < y + h).map(|(k, _)| k)
}

/// The touch buttons, top to bottom down the right edge: what each stands in for (an action of controls.rs, run as its key
/// would be whatever key the player gave it) and its name, shown beside it while a finger is on it. Speed and Message are
/// always there and come first, so the two that come and go (Turn with something to turn, Open hole with a hole ready)
/// never move the others.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Btn {
    Speed,
    Message,
    Turn,
    OpenHole,
}
const BUTTONS: [Btn; 4] = [Btn::Speed, Btn::Message, Btn::Turn, Btn::OpenHole];

impl Btn {
    fn action(self) -> &'static str {
        match self {
            Btn::Speed => "speed",
            Btn::Message => "message",
            Btn::Turn => "turn_building",
            Btn::OpenHole => "open_hole",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Btn::Speed => "STR_TOUCH_SPEED",
            Btn::Message => "STR_TOUCH_MESSAGE",
            Btn::Turn => "STR_TOUCH_TURN",
            Btn::OpenHole => "STR_TOUCH_OPEN_HOLE",
        }
    }
}

/// The column's top: under the money, fun and rating gauges (to y 130) and Enhanced graphics' camera button (140 to 164).
const COLUMN_TOP: f32 = 176.0;
/// The buttons' radius in 800 x 600 units: about 36 pixels across on the device's screen (CSS pixels in a browser), kept
/// between the size of the dock's Information button and a size that still leaves the column clear of the build panel.
const R_MIN: f32 = 15.0;
const R_MAX: f32 = 23.0;
const R_SCREEN: f32 = 18.0;
/// The room round a button that still counts as on it (fingers are wider than the art).
const HIT_PAD: f32 = 5.0;
/// Seconds a button stays lit after it is let go.
const FLASH: f64 = 0.25;

/// A button's radius for this many device-independent pixels to an 800 x 600 unit.
fn button_radius(px_per_unit: f32) -> f32 {
    (R_SCREEN / px_per_unit.max(0.01)).clamp(R_MIN, R_MAX)
}

/// The left edge of the OpenSGolf extras' legends at the top right (the path view, the staff areas, the landing map: x 566,
/// y 140 to 372): while one is up the column stands to its left instead of over it.
const LEGEND_LEFT: f32 = 566.0;

/// Slot `i`'s centre for radius `r` with the column's right edge at `right`, in the 800 x 600 layout (held to the right edge
/// with Enhanced widescreen).
fn button_centre(i: usize, r: f32, right: f32) -> (f32, f32) {
    (right - 6.0 - r, COLUMN_TOP + r + i as f32 * (2.0 * (r + HIT_PAD)))
}

/// Whether (vx, vy) is on the button centred at `c`.
fn on_button(c: (f32, f32), r: f32, vx: f32, vy: f32) -> bool {
    let h = r + HIT_PAD;
    (vx - c.0).abs() <= h && (vy - c.1).abs() <= h
}

fn two(f: &[Finger]) -> ((f32, f32), f32, f32) {
    let (a, b) = (f[0].2, f[1].2);
    let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    (mid, (dx * dx + dy * dy).sqrt().max(1.0), dy.atan2(dx))
}

impl Stage {
    pub(crate) fn touch(&mut self, phase: TouchPhase, id: u64, x: f32, y: f32) {
        self.touch.used = true;
        // no pointer to hover with: the extras' name tags stay off (tags_ui)
        self.app.xs.touched = true;
        let now = self.app.clock;
        match phase {
            TouchPhase::Started => {
                self.touch.fingers.retain(|f| f.0 != id);
                self.touch.fingers.push((id, (x, y), (x, y)));
                match self.touch.fingers.len() {
                    1 => {
                        if self.osk_press(x, y) {
                            self.touch.mode = Mode::Spent;
                        } else if let Some(b) = self.touch_button_at(x, y) {
                            // the button takes the whole touch: nothing reaches the course or the interface under it
                            self.touch.held = Some(b);
                            self.touch.mode = Mode::Spent;
                        } else {
                            self.touch.mode = Mode::Pending;
                            self.touch.started = now;
                        }
                    }
                    2 => {
                        // a second finger ends whatever the first was doing
                        self.touch.held = None;
                        if self.touch.mode == Mode::Mouse {
                            let p = self.touch.fingers[0].2;
                            self.mouse_button_up_event(MouseButton::Left, p.0, p.1);
                        }
                        self.touch.mode = Mode::Gesture;
                        let (mid, spread, angle) = two(&self.touch.fingers);
                        (self.touch.mid, self.touch.spread, self.touch.angle) = (mid, spread, angle);
                    }
                    _ => {}
                }
            }
            TouchPhase::Moved => {
                let Some(f) = self.touch.fingers.iter_mut().find(|f| f.0 == id) else { return };
                f.2 = (x, y);
                let start = f.1;
                if self.touch.held.is_some() && self.touch_button_at(x, y) != self.touch.held {
                    self.touch.held = None;
                }
                match self.touch.mode {
                    Mode::Pending => {
                        let (dx, dy) = (x - start.0, y - start.1);
                        if dx * dx + dy * dy > SLOP * SLOP * self.app.dpi * self.app.dpi {
                            self.mouse_motion_event(start.0, start.1);
                            self.mouse_button_down_event(MouseButton::Left, start.0, start.1);
                            self.touch.mode = Mode::Mouse;
                            self.mouse_motion_event(x, y);
                        }
                    }
                    Mode::Mouse => self.mouse_motion_event(x, y),
                    Mode::Gesture if self.touch.fingers.len() >= 2 => self.gesture(),
                    _ => {}
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let was = self.touch.mode;
                let last = self.touch.fingers.len() == 1;
                self.touch.fingers.retain(|f| f.0 != id);
                if let Some(b) = self.touch.held.take() {
                    if phase == TouchPhase::Ended && self.touch_button_at(x, y) == Some(b) {
                        self.touch.flash = Some((b, now));
                        self.touch_button_press(b);
                    }
                }
                match was {
                    Mode::Pending if phase == TouchPhase::Ended => {
                        // a tap: point where the finger was, then press and release once the screen has seen the pointer
                        self.mouse_motion_event(x, y);
                        self.touch.queued = Some((MouseButton::Left, x, y, 2));
                    }
                    Mode::Mouse => self.mouse_button_up_event(MouseButton::Left, x, y),
                    _ => {}
                }
                if self.touch.lowering && was == Mode::Mouse {
                    self.touch.lowering = false;
                    self.shift = false;
                }
                if last || self.touch.fingers.is_empty() {
                    self.touch.mode = Mode::Idle;
                } else if was == Mode::Gesture {
                    // lifting one of two fingers ends the gesture; the other one does nothing more
                    self.touch.mode = Mode::Spent;
                }
            }
        }
    }

    /// Two fingers: the midpoint pans the course, a pinch past a third more or less steps the zoom, a twist past 50
    /// degrees turns the view a quarter.
    fn gesture(&mut self) {
        let (mid, spread, angle) = two(&self.touch.fingers);
        if self.app.screen == Screen::Play {
            let (dx, dy) = (mid.0 - self.touch.mid.0, mid.1 - self.touch.mid.1);
            let k = self.app.upp;
            let sin_p = (pitch_for(self.app.draw_w, self.app.draw_h).to_radians()).sin() as f32;
            self.app.pan(-dx * k, dy * k / sin_p);
            let ratio = spread / self.touch.spread;
            if crate::enhanced::smooth_camera() || self.app.photo.is_some() {
                // Enhanced (and its photo mode): the zoom follows the fingers continuously
                if ratio > 0.0 && ratio.is_finite() {
                    // Zoom towards the pointer (views.rs): the ground between the fingers keeps its place
                    self.app.zoom_about(mid, |a| a.zoom_by(ratio.log2()));
                    self.touch.spread = spread;
                }
            } else if !(0.74..=1.35).contains(&ratio) {
                self.app.zoom_about(mid, |a| a.zoom_step(ratio > 1.0));
                self.touch.spread = spread;
            }
            let mut turn = angle - self.touch.angle;
            while turn > std::f32::consts::PI {
                turn -= std::f32::consts::TAU;
            }
            while turn < -std::f32::consts::PI {
                turn += std::f32::consts::TAU;
            }
            if turn.abs() > 50f32.to_radians() {
                self.app.rotate_view(if turn > 0.0 { -1 } else { 1 });
                self.touch.angle = angle;
            }
        }
        self.touch.mid = mid;
    }

    /// Called every frame: a finger held still long enough becomes a right click; the phone keyboard follows the name boxes.
    pub(crate) fn touch_update(&mut self) {
        if self.touch.mode == Mode::Pending && self.app.clock - self.touch.started > LONG_PRESS {
            let p = self.touch.fingers.first().map(|f| f.1);
            match p {
                // the raise / lower tool over the course: the press lowers, as Shift+click does (issue #39), and the finger
                // drags on lowering until it lifts
                Some(p) if self.lowers_here(p) => {
                    self.mouse_motion_event(p.0, p.1);
                    self.shift = true;
                    self.touch.lowering = true;
                    self.mouse_button_down_event(MouseButton::Left, p.0, p.1);
                    self.touch.mode = Mode::Mouse;
                }
                Some(p) => {
                    self.mouse_motion_event(p.0, p.1);
                    self.touch.queued = Some((MouseButton::Right, p.0, p.1, 2));
                    self.touch.mode = Mode::Spent;
                }
                None => self.touch.mode = Mode::Spent,
            }
        }
        if let Some((b, x, y, n)) = self.touch.queued {
            if n > 0 {
                self.touch.queued = Some((b, x, y, n - 1));
            } else {
                self.touch.queued = None;
                self.mouse_button_down_event(b, x, y);
                self.mouse_button_up_event(b, x, y);
            }
        }
        let typing = self.app.typing();
        if self.touch.used && typing != self.touch.keyboard {
            self.touch.keyboard = typing;
            show_keyboard(typing);
        }
    }

    /// Whether a long press at this drawable-pixel point lowers the ground: the raise / lower tool is armed and the point is
    /// on the course, not on the dock or its panel.
    fn lowers_here(&self, p: (f32, f32)) -> bool {
        if self.app.screen != Screen::Play || !self.app.edit || self.app.tool != 1 || self.app.popup.is_some() {
            return false;
        }
        let (vx, vy) = self.app.view.to_virtual(p.0, p.1);
        vy < 452.0 && !(self.app.ui_ok && crate::render::dock_hit(vx, vy) >= 0)
    }

    /// The touch buttons' radius for this window, in 800 x 600 units.
    fn button_radius(&self) -> f32 {
        button_radius(self.app.view.scale / self.app.dpi.max(0.5))
    }

    /// Slot `i`'s centre on this frame.
    fn slot_centre(&self, i: usize) -> (f32, f32) {
        button_centre(i, self.button_radius(), if self.app.right_legend_up() { LEGEND_LEFT } else { 800.0 })
    }

    /// The touch button shown at this drawable-pixel point, if any.
    fn touch_button_at(&self, x: f32, y: f32) -> Option<Btn> {
        if !self.touch_buttons_shown() {
            return None;
        }
        // in the column's own layout: with Enhanced widescreen it is drawn at the window's right edge, and a press in the room
        // round a button (or on one that has just appeared) is not on anything drawn the last frame for unshift to find
        let s = Ui::new(self.app.draw_w, self.app.draw_h);
        let (vx, vy) = s.view.to_virtual_raw(x, y);
        let (dx, dy) = s.anchor_shift(crate::enhanced::Anchor::Right);
        let (vx, vy) = (vx - dx, vy - dy);
        let r = self.button_radius();
        BUTTONS
            .iter()
            .enumerate()
            .find(|&(i, &b)| self.app.touch_button_wanted(b) && on_button(self.slot_centre(i), r, vx, vy))
            .map(|(_, &b)| b)
    }

    /// A touch button let go: its action, as its key does on the course (play_key after the player's own keys are read, so a
    /// key moved elsewhere, a shot shape on the same key or a Shift left down cannot change what the button does).
    fn touch_button_press(&mut self, b: Btn) {
        let i = crate::controls::action(b.action());
        let d = crate::controls::ACTIONS[i].default;
        self.play_action(d.key, d.shift, Some(i));
    }

    fn osk_shown(&self) -> bool {
        cfg!(target_arch = "wasm32") && self.touch.used && self.app.typing()
    }

    /// A press on the on-screen keyboard: a letter typed, or Backspace, Enter (OK) or Esc (Cancel) pressed.
    fn osk_press(&mut self, x: f32, y: f32) -> bool {
        if !self.osk_shown() {
            return false;
        }
        let (vx, vy) = self.app.view.to_virtual(x, y);
        let Some(k) = osk_hit(vx, vy) else {
            // the keyboard's band takes every press, so a miss between keys does not reach the screen under it
            return vy >= OSK_TOP - OSK_GAP;
        };
        let none = KeyMods::default();
        let key = |me: &mut Self, code| {
            me.key_down_event(code, none, false);
            me.key_up_event(code, none);
        };
        match k {
            OskKey::Char(c) => {
                let c = if std::mem::take(&mut self.touch.osk_shift) { c.to_ascii_uppercase() } else { c };
                self.char_event(c, none, false);
            }
            OskKey::Space => self.char_event(' ', none, false),
            OskKey::Shift => self.touch.osk_shift = !self.touch.osk_shift,
            OskKey::Back => key(self, KeyCode::Backspace),
            OskKey::Ok => key(self, KeyCode::Enter),
            OskKey::Cancel => key(self, KeyCode::Escape),
        }
        true
    }

    pub(crate) fn draw_osk(&mut self) {
        if !self.osk_shown() {
            return;
        }
        let s = Ui::new(self.app.draw_w, self.app.draw_h);
        let g: &mut Gfx = &mut self.g;
        let shift = self.touch.osk_shift;
        s.fill(g, 0.0, OSK_TOP - 8.0, 800.0, 600.0 - OSK_TOP + 8.0, rgba(0.05, 0.05, 0.15, 0.85));
        for (k, (x, y, w, h)) in osk_keys() {
            let lit = k == OskKey::Shift && shift;
            let fill = match k {
                OskKey::Ok => rgba(0.15, 0.45, 0.2, 0.95),
                OskKey::Cancel => rgba(0.45, 0.15, 0.15, 0.95),
                _ if lit => rgba(0.4, 0.4, 0.8, 0.95),
                _ => rgba(0.2, 0.2, 0.35, 0.95),
            };
            s.fill(g, x, y, w, h, fill);
            s.fill(g, x, y, w, 1.0, rgba(0.7, 0.7, 1.0, 0.8));
            let label = match k {
                OskKey::Char(c) if shift => c.to_ascii_uppercase().to_string(),
                OskKey::Char(c) => c.to_string(),
                OskKey::Shift => tr("STR_OSK_SHIFT").to_string(),
                OskKey::Back => tr("STR_OSK_BACK").to_string(),
                OskKey::Space => tr("STR_OSK_SPACE").to_string(),
                OskKey::Cancel => tr("STR_OSK_CANCEL").to_string(),
                OskKey::Ok => tr("STR_OSK_OK").to_string(),
            };
            s.text_centered(g, x + w * 0.5, y + h * 0.5 + 5.0, &label, 15.0, rgb(1.0, 1.0, 1.0));
        }
        g.flush();
    }

    /// Test hooks for scripted stills (after SG_CLICK, before SG_PRESS): SG_TOUCH=1 counts the screen as touched (the touch
    /// buttons show); SG_TAP="x,y;hold:x,y;down:x,y;key:Tab;..." taps, holds for a long press, or puts a finger down and
    /// leaves it there (for stills of a pressed button) at window pixels, through the same path a finger takes, or presses a
    /// key (settings.json's names), with the frames a tap's click waits for drawn in between. Each step prints the button
    /// it pressed and the state the buttons change.
    pub(crate) fn touch_test_hook(&mut self) {
        if std::env::var_os("SG_TOUCH").is_some() {
            self.touch.used = true;
            self.app.xs.touched = true;
        }
        for t in std::env::var("SG_TAP").unwrap_or_default().split(';').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(k) = t.strip_prefix("key:") {
                let Some(b) = crate::controls::Binding::parse(k) else { continue };
                let mods = KeyMods { shift: b.shift, ctrl: b.ctrl, ..Default::default() };
                self.key_down_event(b.key, mods, false);
                self.key_up_event(b.key, KeyMods::default());
                self.draw_frame(None);
                continue;
            }
            let (how, p) = t.split_once(':').filter(|(h, _)| matches!(*h, "hold" | "down")).unwrap_or(("tap", t));
            let v = crate::ints(p);
            let [x, y] = v[..] else { continue };
            let (x, y) = (x as f32, y as f32);
            self.touch(TouchPhase::Started, 1, x, y);
            let held = self.touch.held;
            if how == "down" {
                // the finger stays down: the frame is drawn with the button pressed
                self.draw_frame(None);
                continue;
            }
            if how == "hold" {
                self.touch.started = self.app.clock - LONG_PRESS - 0.1;
                self.touch_update();
            }
            self.touch(TouchPhase::Ended, 1, x, y);
            for _ in 0..4 {
                self.touch_update();
                self.draw_frame(None);
            }
            let a = &self.app;
            println!(
                "touch {how} {x},{y}: {} (speed {}, turn {}, open holes {}, shift {}, tool {} {})",
                held.map_or("no button".to_string(), |b| format!("{b:?} button")),
                a.speed,
                a.turn,
                a.club.holes.iter().filter(|h| h.par != 0).count(),
                self.shift,
                a.edit,
                a.tool
            );
        }
    }

    /// The touch buttons show on the course once the screen has been touched, with the interface up and nothing over the
    /// course that takes the keys (a notice, a SimFoto, the shot analysis, a name being typed).
    fn touch_buttons_shown(&self) -> bool {
        let a = &self.app;
        self.touch.used
            && a.ui_ok
            && a.screen == Screen::Play
            && a.photo.is_none()
            && !a.no_hud
            && !a.hud_hidden()
            && a.notice.is_none()
            && a.great_shot.is_none()
            && a.simfoto.is_none()
            && a.analysis.is_none()
            && !a.typing()
    }

    /// The touch buttons, drawn like the original's small round dock buttons (Interface/3mainLowerLeft.pcx: + - and the
    /// turns, a navy face in a pale rim with a cream picture, the picture gold while lit), by code so that they look the
    /// same with the free base set. They stay see-through until pressed; the button under a finger shows its name.
    pub(crate) fn draw_touch_buttons(&mut self) {
        if !self.touch_buttons_shown() {
            return;
        }
        let s = Ui::new(self.app.draw_w, self.app.draw_h);
        // Enhanced widescreen: at the window's right edge (nothing in Classic)
        s.anchor(crate::enhanced::Anchor::Right);
        let r = self.button_radius();
        let now = self.app.clock;
        let flash = self.touch.flash.filter(|f| now - f.1 < FLASH).map(|f| f.0);
        let centres: Vec<(f32, f32)> = (0..BUTTONS.len()).map(|i| self.slot_centre(i)).collect();
        let g: &mut Gfx = &mut self.g;
        for (i, &b) in BUTTONS.iter().enumerate() {
            if !self.app.touch_button_wanted(b) {
                continue;
            }
            let (cx, cy) = centres[i];
            let lit = self.touch.held == Some(b) || flash == Some(b);
            draw_button(g, &s, cx, cy, r, lit);
            let ink = if lit { GOLD } else { with_alpha(CREAM, 0.9) };
            match b {
                Btn::Speed => icon_speed(g, &s, cx, cy, r, self.app.speed, ink),
                Btn::Message => icon_message(g, &s, cx, cy, r, ink),
                Btn::Turn => icon_turn(g, &s, cx, cy, r, ink),
                Btn::OpenHole => icon_flag(g, &s, cx, cy, r, ink, lit),
            }
            if self.touch.held == Some(b) {
                // its name to the left, as the dock's tooltips are (the camera button's plate, photo.rs)
                let name = tr(b.label());
                // (as large on the device as the buttons are, so readable on a phone too)
                let k = r / R_MIN;
                let size = 13.0 * k;
                let tw = crate::ui::text_width(name, size);
                let (x, y) = (cx - r - 8.0 - tw - 12.0 * k, cy - 10.0 * k);
                s.fill(g, x, y, tw + 12.0 * k, 20.0 * k, rgba(0.0, 0.0, 0.0, 0.7));
                s.text(g, x + 6.0 * k, y + 15.0 * k, name, size, rgb(1.0, 1.0, 1.0));
            }
        }
        g.flush();
    }
}

// ---- the touch buttons' look --------------------------------------------------------------------------------------------
// The colours are the 15-bit palette of the dock's small buttons (3mainLowerLeft.pcx cuts at x 600 and 650): rim 0x101830
// and 0x707098 / 0x8888b8, face 0x303870 to 0x182040, picture 0xf0e0c0 with its shade 0xd8c070, lit 0xf8b800.

const NAVY_DARK: [f32; 4] = [16.0 / 255.0, 24.0 / 255.0, 48.0 / 255.0, 1.0];
const RIM_LIGHT: [f32; 4] = [136.0 / 255.0, 136.0 / 255.0, 184.0 / 255.0, 1.0];
const RIM_SHADE: [f32; 4] = [32.0 / 255.0, 40.0 / 255.0, 80.0 / 255.0, 1.0];
const FACE_MID: [f32; 4] = [56.0 / 255.0, 64.0 / 255.0, 120.0 / 255.0, 1.0];
const FACE_EDGE: [f32; 4] = [24.0 / 255.0, 32.0 / 255.0, 64.0 / 255.0, 1.0];
const CREAM: [f32; 4] = [240.0 / 255.0, 224.0 / 255.0, 192.0 / 255.0, 1.0];
const GOLD: [f32; 4] = [248.0 / 255.0, 184.0 / 255.0, 0.0, 1.0];
const FLAG_RED: [f32; 4] = [216.0 / 255.0, 48.0 / 255.0, 32.0 / 255.0, 1.0];
/// How see-through a button is until pressed.
const IDLE_ALPHA: f32 = 0.65;
/// Pieces of a circle.
const SEGMENTS: usize = 32;

fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
}

fn mix(a: [f32; 4], b: [f32; 4], k: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
}

/// A ring from radius `r0` out to `r1` round (cx, cy) (r0 = 0 for a disc), each piece coloured by `col` of its middle angle
/// (radians, 0 to the right, growing clockwise on the screen). The pieces do not overlap, so a see-through ring is even.
fn ring(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r0: f32, r1: f32, col: impl Fn(f32) -> [f32; 4]) {
    let step = std::f32::consts::TAU / SEGMENTS as f32;
    for k in 0..SEGMENTS {
        let (a0, a1) = (k as f32 * step, (k + 1) as f32 * step);
        let p = |r: f32, a: f32| (cx + r * a.cos(), cy + r * a.sin());
        s.quad(g, [p(r0, a0), p(r1, a0), p(r1, a1), p(r0, a1)], col((a0 + a1) * 0.5));
    }
}

/// A round button: a dark outline, a bevel lit from the top left, and a navy face lighter towards its middle.
fn draw_button(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r: f32, lit: bool) {
    let a = if lit { 1.0 } else { IDLE_ALPHA };
    // a soft shadow below and to the right, outside the button
    ring(g, s, cx + 1.0, cy + 2.0, r, r + 1.5, |ang| rgba(0.0, 0.0, 0.0, 0.3 * a * (ang - 0.8).cos().max(0.0)));
    ring(g, s, cx, cy, r - 1.5, r, |_| with_alpha(NAVY_DARK, a));
    // the bevel: light where it faces the top left (angle 225 degrees), dark where it faces away
    let light = std::f32::consts::PI * 1.25;
    ring(g, s, cx, cy, r - 4.0, r - 1.5, |ang| with_alpha(mix(RIM_SHADE, RIM_LIGHT, ((ang - light).cos() + 1.0) * 0.5), a));
    ring(g, s, cx, cy, r - 5.0, r - 4.0, |_| with_alpha(NAVY_DARK, a));
    let face = r - 5.0;
    let mid = if lit { mix(FACE_MID, RIM_LIGHT, 0.25) } else { FACE_MID };
    let bands = 10;
    for k in 0..bands {
        let (r0, r1) = (face * k as f32 / bands as f32, face * (k + 1) as f32 / bands as f32);
        let c = mix(mid, FACE_EDGE, (k as f32 + 0.5) / bands as f32);
        ring(g, s, cx, cy, r0, r1, |_| with_alpha(c, a));
    }
}

/// A picture drawn twice: a dark shade a little down and right, then the picture, as the dock's pictures are.
fn shaded(g: &mut Gfx, ink: [f32; 4], draw: impl Fn(&mut Gfx, f32, f32, [f32; 4])) {
    draw(g, 0.8, 0.8, with_alpha(NAVY_DARK, ink[3]));
    draw(g, 0.0, 0.0, ink);
}

/// Speed: one, two or three play arrows for normal, x2 and x4.
fn icon_speed(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r: f32, speed: u8, ink: [f32; 4]) {
    let n = match speed {
        1 => 1,
        2 => 2,
        _ => 3,
    };
    let h = r * 0.42;
    let w = r * 0.38;
    let total = w * n as f32 + (n - 1) as f32 * r * 0.04;
    let x0 = cx - total * 0.5;
    shaded(g, ink, |g, dx, dy, c| {
        for k in 0..n {
            let x = x0 + k as f32 * (w + r * 0.04) + dx;
            let y = cy + dy;
            s.quad(g, [(x, y - h), (x + w, y), (x + w, y), (x, y + h)], c);
        }
    });
}

/// Message: a speech bubble with three dots.
fn icon_message(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r: f32, ink: [f32; 4]) {
    let (bw, bh) = (r * 0.5, r * 0.36);
    let by = cy - r * 0.07;
    shaded(g, ink, |g, dx, dy, c| {
        let (x, y) = (cx + dx, by + dy);
        // the bubble: an ellipse in pieces that meet at its middle, and its tail to the lower left
        let step = std::f32::consts::TAU / SEGMENTS as f32;
        for k in 0..SEGMENTS {
            let (a0, a1) = (k as f32 * step, (k + 1) as f32 * step);
            let p = |a: f32| (x + bw * a.cos(), y + bh * a.sin());
            s.quad(g, [(x, y), p(a0), p(a1), (x, y)], c);
        }
        let t = (x - bw * 0.45, y + bh * 0.7);
        s.quad(g, [t, (t.0 + bw * 0.35, t.1), (x - bw * 0.75, y + bh * 1.55), t], c);
    });
    let d = r * 0.07;
    for k in -1..=1 {
        let x = cx + k as f32 * bw * 0.48;
        s.quad(g, [(x - d, by - d), (x + d, by - d), (x + d, by + d), (x - d, by + d)], with_alpha(NAVY_DARK, ink[3]));
    }
}

/// Turn: a thick arrow bending round to the right (the dock's turn buttons' picture, with more of the turn).
fn icon_turn(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r: f32, ink: [f32; 4]) {
    let rad = r * 0.36;
    let w = r * 0.13;
    let (oy, from, to) = (cy + r * 0.04, std::f32::consts::PI * 0.75, std::f32::consts::PI * 2.05);
    shaded(g, ink, |g, dx, dy, c| {
        let n = 14;
        let p = |a: f32, rr: f32| (cx + dx + rr * a.cos(), oy + dy + rr * a.sin());
        for k in 0..n {
            let a0 = from + (to - from) * k as f32 / n as f32;
            let a1 = from + (to - from) * (k + 1) as f32 / n as f32;
            s.quad(g, [p(a0, rad - w), p(a0, rad + w), p(a1, rad + w), p(a1, rad - w)], c);
        }
        // the head, pointing on round the turn
        let tip_a = to + 0.55;
        let (bx, by) = p(to, rad);
        let (ox, oy2) = (to.cos(), to.sin());
        let tip = p(tip_a, rad);
        let hw = w * 2.4;
        s.quad(g, [(bx - ox * hw, by - oy2 * hw), (bx + ox * hw, by + oy2 * hw), tip, tip], c);
    });
}

/// Open hole: the flagstick in its cup, the flag red (as the build panel's green shows it) and the stick cream.
fn icon_flag(g: &mut Gfx, s: &Ui, cx: f32, cy: f32, r: f32, ink: [f32; 4], lit: bool) {
    let px = cx - r * 0.12;
    let (top, foot) = (cy - r * 0.48, cy + r * 0.42);
    let flag = if lit { GOLD } else { with_alpha(FLAG_RED, ink[3]) };
    shaded(g, ink, |g, dx, dy, c| {
        s.line(g, px + dx, top + dy, px + dx, foot + dy, r * 0.09, c);
        // the cup: a flat ring at the stick's foot
        s.quad(
            g,
            [
                (px - r * 0.3 + dx, foot + dy),
                (px + dx, foot - r * 0.07 + dy),
                (px + r * 0.3 + dx, foot + dy),
                (px + dx, foot + r * 0.07 + dy),
            ],
            c,
        );
    });
    let fl = [(px, top), (px + r * 0.5, top + r * 0.16), (px + r * 0.5, top + r * 0.16), (px, top + r * 0.34)];
    s.quad(g, fl.map(|(x, y)| (x + 0.8, y + 0.8)), with_alpha(NAVY_DARK, ink[3]));
    s.quad(g, fl, flag);
}

impl App {
    /// Whether a touch button has something to do now: Speed and Message always; Turn while Tab turns something (a building
    /// or a green's look in hand, the Path styles extra's style with the Pathway tool, a placed building with the Rotate
    /// extra); Open hole once the hole being built has its tee and green.
    fn touch_button_wanted(&self, b: Btn) -> bool {
        match b {
            Btn::Speed | Btn::Message => true,
            Btn::Turn if self.edit => match self.tool {
                4 => true,
                0 => self.paint_variant.is_some_and(|(k, _)| k == self.paint_idx),
                2 => self.club.extras.lamps(),
                _ => false,
            },
            Btn::Turn => self.club.extras.rotate,
            Btn::OpenHole => self.club.can_open_hole(),
        }
    }

    /// Whether one of the extras' legends is up at the top right, under the gauges (pathview_ui, staffview_ui, insight).
    fn right_legend_up(&self) -> bool {
        (self.comfort.staff_view && self.land.is_some())
            || (self.xs.path_view && crate::extras::path_help())
            || (self.xs.insight.map && crate::extras::landing_map())
    }

    /// Whether a name is being typed (rename, save, Customise Golfer).
    pub fn typing(&self) -> bool {
        self.rename.is_some()
            || self.editor_typing()
            || self.title.save_name.is_some()
            || self.content.profile_name.is_some()
            || self.cust.as_ref().is_some_and(|c| c.typing.is_some())
            || self.popup.as_ref().is_some_and(|p| p.kind == crate::popup_ui::PopupKind::Search)
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn sg_keyboard(show: u32);
}

/// The version of web/opensgolf.js's miniquad plugin ("simgolf_fs", version 1) this build goes with: gl.js asks for it and
/// otherwise logs that the plugin is unused.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn simgolf_fs_crate_version() -> u32 {
    1
}

/// Raises or lowers the phone's keyboard: in a browser a hidden text box in the page, on Android the system keyboard through
/// miniquad (its typed letters come back as char events); nothing on the desktop.
fn show_keyboard(show: bool) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        sg_keyboard(show as u32)
    }
    #[cfg(target_os = "android")]
    miniquad::window::show_keyboard(show);
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    let _ = show;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osk_keys_fit_the_screen_below_the_name_boxes_and_do_not_overlap() {
        let keys = osk_keys();
        for (i, (k, (x, y, w, h))) in keys.iter().enumerate() {
            assert!(*x >= 0.0 && x + w <= 800.0 && *y >= 400.0 && y + h <= 600.0, "{k:?} off the screen");
            for (k2, (x2, y2, w2, h2)) in &keys[i + 1..] {
                let apart = x + w <= *x2 || x2 + w2 <= *x || y + h <= *y2 || y2 + h2 <= *y;
                assert!(apart, "{k:?} overlaps {k2:?}");
            }
            assert_eq!(osk_hit(x + w * 0.5, y + h * 0.5), Some(*k));
        }
        // every letter and digit is there, and the keys that close a box
        for c in "abcdefghijklmnopqrstuvwxyz0123456789".chars() {
            assert!(keys.iter().any(|(k, _)| *k == OskKey::Char(c)), "{c}");
        }
        assert!(keys.iter().any(|(k, _)| *k == OskKey::Ok) && keys.iter().any(|(k, _)| *k == OskKey::Cancel));
        // the rename box (240..330) and the save box stay clear of the keyboard
        assert!(keys.iter().all(|(_, (_, y, _, _))| *y > 340.0));
    }

    #[test]
    fn touch_buttons_fit_between_the_gauges_and_the_panel_at_every_size() {
        // a phone in landscape (0.65 pixels to the unit), tablets, a desktop, a 4K screen
        for px in [0.3, 0.65, 1.0, 1.37, 1.71, 2.4, 3.6] {
            let r = button_radius(px);
            assert!((R_MIN..=R_MAX).contains(&r));
            for right in [800.0, LEGEND_LEFT] {
                let c: Vec<(f32, f32)> = (0..BUTTONS.len()).map(|i| button_centre(i, r, right)).collect();
                for (i, &(x, y)) in c.iter().enumerate() {
                    // under the gauges and the camera button (to y 164), above the dock's panel (y 452), on the screen
                    assert!(y - r - HIT_PAD > 164.0 && y + r + HIT_PAD < 452.0, "slot {i} at {px}: {y}");
                    assert!(x + r + HIT_PAD <= right + 0.01 && x - r > 0.0);
                    // no press is on two buttons
                    if let Some(&(_, y2)) = c.get(i + 1) {
                        assert!(y2 - y >= 2.0 * (r + HIT_PAD) - 0.01);
                    }
                    assert!(on_button((x, y), r, x + r, y - r) && !on_button((x, y), r, x, y + r + HIT_PAD + 1.0));
                }
            }
        }
        // about 36 device pixels across where that fits
        assert_eq!(button_radius(1.0), 18.0);
    }
}
