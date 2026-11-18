//! Fine elevation on a touch screen (an OpenSGolf extra on the Controls page, off by default; docs/PLAYING.md, "Fine
//! elevation on touch screens"): with the raise / lower tool or an Elevation panel tool in hand, a tap on the course picks
//! the corner under it instead of raising it, and the Raise and Lower touch buttons then step that corner (or the panel
//! tool's square or area) up or down one notch a press. The pick is ringed on the course. Only after the screen has been
//! touched: a mouse raises and lowers as before.
use crate::app::App;
use crate::gfx::Gfx;
use crate::ui::Screen as Ui;
use sg_core::lang::tr;

impl App {
    /// Whether a tap with the elevation tool picks a corner instead of raising it.
    pub fn fine_height_on(&self) -> bool {
        crate::extras::fine_height() && self.xs.touched && self.edit && self.tool == 1
    }

    /// The corner the Raise and Lower buttons step, while the elevation tool is in hand.
    pub fn height_picked(&self) -> Option<(i32, i32)> {
        self.xs.height_pick.filter(|_| self.fine_height_on())
    }

    /// A tap with the elevation tool: corner (a, b) picked.
    pub fn pick_height(&mut self, a: i32, b: i32) {
        let first = self.xs.height_pick.is_none();
        self.xs.height_pick = Some((a, b));
        if first {
            self.show_toast(tr("STR_TOAST_HEIGHT_PICKED"));
        }
    }

    /// Raise (+1) or Lower (-1) pressed: the picked corner stepped as the tool in hand would step it, kept for undo as a click.
    pub fn height_step(&mut self, delta: i32) -> bool {
        let Some((a, b)) = self.height_picked() else { return false };
        self.undo_press();
        if self.panel == 2 && self.pstate.alt && self.panel_art.ready() {
            self.edit_elevation(a, b, delta);
        } else {
            self.edit_raise(a, b, delta);
        }
        self.undo_release();
        true
    }

    /// The picked corner ringed on the course.
    pub fn draw_height_pick(&self, g: &mut Gfx, s: &Ui) {
        let Some((a, b)) = self.height_picked() else { return };
        let (wx, wz) = self.units_to_world(a * 1024, b * 1024);
        let Some((x, y)) = self.screen_of_world(wx, wz) else { return };
        let gold = [1.0, 0.84, 0.25, 0.95];
        let dark = [0.05, 0.08, 0.15, 0.7];
        for (r, w, c) in [(11.0, 3.5, dark), (10.0, 2.0, gold)] {
            let n = 16;
            for k in 0..n {
                let a0 = std::f32::consts::TAU * k as f32 / n as f32;
                let a1 = std::f32::consts::TAU * (k + 1) as f32 / n as f32;
                // flattened as the ground is seen
                let p = |a: f32, rr: f32| (x + rr * a.cos(), y + rr * 0.6 * a.sin());
                s.quad(g, [p(a0, r - w * 0.5), p(a0, r + w * 0.5), p(a1, r + w * 0.5), p(a1, r - w * 0.5)], c);
            }
        }
        s.quad(g, [(x - 1.5, y - 1.5), (x + 1.5, y - 1.5), (x + 1.5, y + 1.5), (x - 1.5, y + 1.5)], gold);
    }
}
