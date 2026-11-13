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
const HALF: f32 = 0.9;
const SHADE: f32 = 0.74;

/// The band of light for a facing `d` to the sun (-1..1): lit, half lit or in shade, with a narrow soft step between
/// them so a rounded body's bands have clean, smooth edges rather than stair steps along its triangles.
fn band(d: f32) -> f32 {
    let step = |e: f32| ((d - e) / 0.08 + 0.5).clamp(0.0, 1.0);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    SHADE + (HALF - SHADE) * s(step(0.05)) + (LIT - HALF) * s(step(0.45))
}

/// Each point in its band of light by its facing to `sun` (unit, towards it): a smooth model's normals give smooth
/// bands, a flat-sided one's (each face's own normal) a band a face. The normals are cleared so the shader takes the
/// colour as it is.
pub fn cel(v: &mut [Vert], sun: [f32; 3]) {
    for t in v.chunks_exact_mut(3) {
        // a point without a normal takes its triangle's
        let tn = [0, 1, 2].map(|k| t[0].normal[k] + t[1].normal[k] + t[2].normal[k]);
        for p in t.iter_mut() {
            let n = if p.normal == [0.0; 3] { tn } else { p.normal };
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let k = if l < 1e-4 { HALF } else { band((n[0] * sun[0] + n[1] * sun[1] + n[2] * sun[2]) / l) };
            for c in &mut p.color[..3] {
                *c = (*c * k).min(1.0);
            }
            p.normal = [0.0; 3];
        }
    }
}

/// The outline shell of `v`: each point `width` out along its normal, in a dark shade of its own colour, unlit.
pub fn outline(v: &[Vert], width: f32, out: &mut Vec<Vert>) {
    out.reserve(v.len());
    for p in v {
        let mut q = *p;
        let n = p.normal;
        q.pos = [p.pos[0] + n[0] * width, p.pos[1] + n[1] * width, p.pos[2] + n[2] * width];
        // a dark shade of the colour itself, as a painter's line is, not black ink
        q.color = [p.color[0] * 0.42 + 0.02, p.color[1] * 0.4 + 0.02, p.color[2] * 0.44 + 0.03, 1.0];
        q.normal = [0.0; 3];
        out.push(q);
    }
}

/// How thick an outline is at `dist` world units from the eye: about a pixel on the screen, never thinner
/// than `least`.
pub fn width(dist: f32, least: f32) -> f32 {
    (dist * 0.0016).max(least)
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
        assert!(o[0].color[0] < 0.3);
    }
}
