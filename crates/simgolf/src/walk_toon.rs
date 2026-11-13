//! The drawn look for the walk camera's models: flat bands of light and a dark outline, as a cartoon is inked and
//! coloured, so the 3D golfers and buildings sit with the game's hand-drawn pictures rather than looking like plastic.
//!
//! The bands are worked out on the CPU, a whole triangle at once, from its facing to the sun: lit, half lit or in shade.
//! The outline is the model again, a little fatter (each point pushed out along its normal), in a dark shade of its own
//! colour, and drawn pushed back from the eye (`WalkUniforms::pull` below zero) by more than the model is deep: inside the
//! model's outline the model is nearer and covers it, so only the rim round the outside shows.

use crate::gfx::Vert;

/// The bands: lit, half lit, in shade (multiplying the colour).
const LIT: f32 = 1.1;
const HALF: f32 = 0.86;
const SHADE: f32 = 0.64;

/// Each triangle in one flat band of light by its facing to `sun` (unit, towards it). The normals are cleared so the
/// shader takes the colour as it is.
pub fn cel(v: &mut [Vert], sun: [f32; 3]) {
    for t in v.chunks_exact_mut(3) {
        let n = [0, 1, 2].map(|k| t[0].normal[k] + t[1].normal[k] + t[2].normal[k]);
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let band = if l < 1e-4 {
            HALF
        } else {
            let d = (n[0] * sun[0] + n[1] * sun[1] + n[2] * sun[2]) / l;
            if d > 0.45 {
                LIT
            } else if d > 0.05 {
                HALF
            } else {
                SHADE
            }
        };
        for p in t.iter_mut() {
            for c in &mut p.color[..3] {
                *c = (*c * band).min(1.0);
            }
            p.normal = [0.0; 3];
        }
    }
}

/// The outline shell of `v`: each point `width` out along its normal, in a dark shade of its colour, unlit.
pub fn outline(v: &[Vert], width: f32, out: &mut Vec<Vert>) {
    out.reserve(v.len());
    for p in v {
        let mut q = *p;
        let n = p.normal;
        q.pos = [p.pos[0] + n[0] * width, p.pos[1] + n[1] * width, p.pos[2] + n[2] * width];
        q.color = [p.color[0] * 0.2 + 0.01, p.color[1] * 0.2 + 0.01, p.color[2] * 0.22 + 0.015, 1.0];
        q.normal = [0.0; 3];
        out.push(q);
    }
}

/// How thick an outline is at `dist` world units from the eye: about a pixel and a half on the screen, never thinner
/// than `least`.
pub fn width(dist: f32, least: f32) -> f32 {
    (dist * 0.0022).max(least)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(n: [f32; 3]) -> Vec<Vert> {
        (0..3)
            .map(|k| {
                let mut v = Vert::new(k as f32, 0.0, 0.0, 0.0, 0.0).col([0.5, 0.5, 0.5, 1.0]);
                v.normal = n;
                v
            })
            .collect()
    }

    #[test]
    fn a_face_to_the_sun_is_lit_and_one_away_in_shade() {
        let sun = [0.0, 1.0, 0.0];
        let (mut up, mut down) = (tri([0.0, 1.0, 0.0]), tri([0.0, -1.0, 0.0]));
        cel(&mut up, sun);
        cel(&mut down, sun);
        assert!(up[0].color[0] > 0.5 && down[0].color[0] < 0.5);
        assert_eq!(up[0].normal, [0.0; 3]);
    }

    #[test]
    fn the_outline_stands_out_along_the_normals_and_is_dark() {
        let mut o = Vec::new();
        outline(&tri([0.0, 0.0, 1.0]), 0.1, &mut o);
        assert!((o[0].pos[2] - 0.1).abs() < 1e-6);
        assert!(o[0].color[0] < 0.2);
    }
}
