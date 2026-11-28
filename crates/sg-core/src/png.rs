//! Minimal PNG writer (RGBA8, zlib via miniz_oxide). For screenshots and asset export.
use crate::assets::Rgba;

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        t[n as usize] = c;
    }
    t
}

fn chunk(out: &mut Vec<u8>, table: &[u32; 256], ty: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(ty);
    out.extend_from_slice(body);
    let mut c = 0xFFFF_FFFFu32;
    for &b in &out[start..] {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    out.extend_from_slice(&(!c).to_be_bytes());
}

/// Encodes an image as PNG bytes.
pub fn encode_png(img: &Rgba) -> Option<Vec<u8>> {
    encode_png_level(img, 6, false)
}

/// `encode_png` packed as small as it goes (about a fifth smaller for a picture of the course), for files kept or handed
/// to the player (`write_png`: the accomplishment snapshots the browser keeps with the saves, screenshots and photos): an
/// opaque picture as RGB, and each row under the filter that leaves the smallest differences, as PNG encoders usually
/// choose. Slower than `encode_png`, so not for the many pictures made and read back at once (HD packs). The same pixels.
pub fn encode_png_small(img: &Rgba) -> Option<Vec<u8>> {
    encode_png_level(img, 9, true)
}

/// `encode_png` packed less tightly but several times faster: for pictures made only to be read straight back (the free
/// set's art, drawn at run time), where the bytes' size does not matter. The same pixels.
pub fn encode_png_quick(img: &Rgba) -> Option<Vec<u8>> {
    encode_png_level(img, 1, false)
}

fn encode_png_level(img: &Rgba, level: u8, tight: bool) -> Option<Vec<u8>> {
    if img.w == 0 || img.h == 0 || img.px.len() < img.w as usize * img.h as usize * 4 {
        return None;
    }
    let table = crc_table();
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let (w, h) = (img.w as usize, img.h as usize);
    // (`tight`: see encode_png_small)
    let rgb = tight && img.px[..w * h * 4].chunks_exact(4).all(|p| p[3] == 255);
    let ch = if rgb { 3 } else { 4 };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&img.w.to_be_bytes());
    ihdr.extend_from_slice(&img.h.to_be_bytes());
    ihdr.extend_from_slice(&[8, if rgb { 2 } else { 6 }, 0, 0, 0]);
    chunk(&mut out, &table, b"IHDR", &ihdr);
    let stride = w * ch;
    let rows: Vec<Vec<u8>> = (0..h)
        .map(|y| {
            let src = &img.px[y * w * 4..(y + 1) * w * 4];
            if rgb {
                src.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
            } else {
                src.to_vec()
            }
        })
        .collect();
    let mut raw = Vec::with_capacity((stride + 1) * h);
    let zero = vec![0u8; stride];
    let mut cand = vec![0u8; stride];
    let mut best = vec![0u8; stride];
    for y in 0..h {
        let row = &rows[y];
        if !tight {
            raw.push(0);
            raw.extend_from_slice(row);
            continue;
        }
        let up = if y > 0 { &rows[y - 1] } else { &zero };
        let mut best_f = 0u8;
        let mut best_sum = u64::MAX;
        for f in 0..5u8 {
            let mut sum = 0u64;
            for i in 0..stride {
                let a = if i >= ch { row[i - ch] as i16 } else { 0 };
                let b = up[i] as i16;
                let c = if i >= ch { up[i - ch] as i16 } else { 0 };
                let pred = match f {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => (a + b) / 2,
                    _ => {
                        let (pa, pb, pc) = ((b - c).abs(), (a - c).abs(), (a + b - 2 * c).abs());
                        if pa <= pb && pa <= pc {
                            a
                        } else if pb <= pc {
                            b
                        } else {
                            c
                        }
                    }
                };
                let v = row[i].wrapping_sub(pred as u8);
                cand[i] = v;
                sum += (v as i8).unsigned_abs() as u64;
            }
            if sum < best_sum {
                best_sum = sum;
                best_f = f;
                best.copy_from_slice(&cand);
            }
        }
        raw.push(best_f);
        raw.extend_from_slice(&best);
    }
    let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, level);
    chunk(&mut out, &table, b"IDAT", &z);
    chunk(&mut out, &table, b"IEND", &[]);
    Some(out)
}

pub fn write_png(path: impl AsRef<std::path::Path>, img: &Rgba) -> bool {
    let Some(bytes) = encode_png_small(img) else { return false };
    crate::fsutil::write_file(path, &bytes)
}

/// Decodes a PNG: 8 bits per channel, not interlaced, greyscale, RGB, palette (with tRNS), grey + alpha or RGBA, any row
/// filter. Covers the accomplishment snapshots `encode_png` writes, the HD packs' pictures (docs/HD.md) and the PNG art of
/// content packs (docs/MODDING.md). Anything else gives None.
pub fn decode_png(d: &[u8]) -> Option<Rgba> {
    let r = png_raw(d).filter(|r| r.depth == 8)?;
    let (w, h, ch) = (r.w, r.h, r.ch);
    let mut out = Rgba::new(w as u32, h as u32);
    for (i, o) in out.px.chunks_exact_mut(4).enumerate() {
        let s = &r.px[i * ch..i * ch + ch];
        let px = match r.ct {
            0 => [s[0], s[0], s[0], 255],
            2 => [s[0], s[1], s[2], 255],
            3 => {
                let k = s[0] as usize;
                let c = r.pal.get(k * 3..k * 3 + 3)?;
                [c[0], c[1], c[2], r.trns.get(k).copied().unwrap_or(255)]
            }
            4 => [s[0], s[0], s[0], s[1]],
            _ => [s[0], s[1], s[2], s[3]],
        };
        o.copy_from_slice(&px);
    }
    Some(out)
}

/// An 8-bit palette picture: width, height, the colour indices and the palette.
pub type IndexedPng = (u32, u32, Vec<u8>, [[u8; 3]; 256]);

/// Decodes an 8-bit palette PNG to its colour indices and its palette (missing entries black): the free set's sprite
/// sheets (crate::freeset), which keep the palette slots the engine recolours. None for any other kind of PNG.
pub fn decode_png_indexed(d: &[u8]) -> Option<IndexedPng> {
    let r = png_raw(d)?;
    if r.ct != 3 || r.depth != 8 {
        return None;
    }
    let mut pal = [[0u8; 3]; 256];
    for (i, c) in r.pal.chunks_exact(3).take(256).enumerate() {
        pal[i] = [c[0], c[1], c[2]];
    }
    Some((r.w as u32, r.h as u32, r.px, pal))
}

/// A grey picture of any size, from a PNG of 8 or 16 bits per channel (the course editor's heightmaps, crate::heightmap):
/// each pixel's brightness from 0.0 (black) to 1.0 (white), rows top to bottom. A colour picture is read as its luma.
pub fn decode_png_grey(d: &[u8]) -> Option<(usize, usize, Vec<f32>)> {
    let r = png_raw(d)?;
    let bytes = (r.depth / 8) as usize;
    let max = if bytes == 2 { 65535.0 } else { 255.0 };
    // channel k of pixel i, 0..1
    let at = |i: usize, k: usize| -> f32 {
        let o = (i * r.ch + k) * bytes;
        let v = if bytes == 2 { u16::from_be_bytes([r.px[o], r.px[o + 1]]) as f32 } else { r.px[o] as f32 };
        v / max
    };
    let luma = |c: [f32; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let mut v = Vec::with_capacity(r.w * r.h);
    for i in 0..r.w * r.h {
        v.push(match r.ct {
            0 | 4 => at(i, 0),
            2 | 6 => luma([at(i, 0), at(i, 1), at(i, 2)]),
            _ => {
                let k = r.px[i] as usize;
                let c = r.pal.get(k * 3..k * 3 + 3)?;
                luma([c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0])
            }
        });
    }
    Some((r.w, r.h, v))
}

/// A PNG's pixels after the row filters, `ch` channels of `depth` bits each, with its colour type, palette and transparency.
struct Raw {
    w: usize,
    h: usize,
    ct: u8,
    /// Bits per channel: 8, or 16 (big-endian; only read by `decode_png_grey`).
    depth: u8,
    ch: usize,
    pal: Vec<u8>,
    trns: Vec<u8>,
    px: Vec<u8>,
}

fn png_raw(d: &[u8]) -> Option<Raw> {
    if d.len() < 8 || d[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    let be = |o: usize| u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let (mut w, mut h, mut ct, mut depth) = (0usize, 0usize, 0u8, 8u8);
    let mut pal: Vec<u8> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat = Vec::new();
    let mut p = 8;
    while p + 8 <= d.len() {
        let len = be(p) as usize;
        let ty = &d[p + 4..p + 8];
        let body = d.get(p + 8..p + 8 + len)?;
        match ty {
            b"IHDR" => {
                if len < 13 || !(body[8] == 8 || body[8] == 16 && body[9] != 3) || body[12] != 0 {
                    return None; // only 8 (or 16) bits per channel, not interlaced
                }
                depth = body[8];
                w = u32::from_be_bytes(body[0..4].try_into().ok()?) as usize;
                h = u32::from_be_bytes(body[4..8].try_into().ok()?) as usize;
                ct = body[9];
            }
            b"PLTE" => pal = body.to_vec(),
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        p += 12 + len;
    }
    let ch = match ct {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return None;
    }
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).ok()?;
    // the filters work on whole pixels: `bpp` bytes back
    let bpp = ch * depth as usize / 8;
    let stride = w * bpp;
    if raw.len() < (stride + 1) * h {
        return None;
    }
    let mut px = vec![0u8; stride * h];
    let mut prev = vec![0u8; stride];
    for y in 0..h {
        let line = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let f = line[0];
        let cur = &mut px[y * stride..(y + 1) * stride];
        // the common filters in a loop of their own (the same bytes as the general one below, sooner)
        match f {
            0 => {
                cur.copy_from_slice(&line[1..]);
                prev.copy_from_slice(cur);
                continue;
            }
            1 => {
                cur[..bpp].copy_from_slice(&line[1..=bpp]);
                for i in bpp..stride {
                    cur[i] = line[i + 1].wrapping_add(cur[i - bpp]);
                }
                prev.copy_from_slice(cur);
                continue;
            }
            2 => {
                for ((c, &x), &b) in cur.iter_mut().zip(&line[1..]).zip(&prev) {
                    *c = x.wrapping_add(b);
                }
                prev.copy_from_slice(cur);
                continue;
            }
            _ => {}
        }
        for i in 0..stride {
            let x = line[i + 1];
            let a = if i >= bpp { cur[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            cur[i] = match f {
                0 => x,
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => {
                    let pp = a as i16 + b as i16 - c as i16;
                    let (pa, pb, pc) = ((pp - a as i16).abs(), (pp - b as i16).abs(), (pp - c as i16).abs());
                    x.wrapping_add(if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    })
                }
                _ => return None,
            };
        }
        prev.copy_from_slice(cur);
    }
    Some(Raw { w, h, ct, depth, ch, pal, trns, px })
}

#[cfg(test)]
mod tests {
    #[test]
    fn png_has_signature_and_iend() {
        let img = crate::assets::Rgba::new(2, 2);
        let b = super::encode_png(&img).unwrap();
        assert_eq!(&b[1..4], b"PNG");
        assert_eq!(&b[b.len() - 8..b.len() - 4], b"IEND");
        // CRC of an empty IEND chunk is fixed by the PNG spec.
        assert_eq!(&b[b.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn png_round_trips() {
        let mut img = crate::assets::Rgba::new(3, 2);
        for (i, v) in img.px.iter_mut().enumerate() {
            *v = (i * 7) as u8;
        }
        let back = super::decode_png(&super::encode_png(&img).unwrap()).unwrap();
        assert_eq!((back.w, back.h), (3, 2));
        assert_eq!(back.px, img.px);
        assert!(super::decode_png(b"not a png").is_none());
    }

    #[test]
    fn a_picture_is_packed_tightly_and_reads_back_the_same() {
        // a course-like picture: smooth shades with some noise, opaque (written as RGB) and with transparency (RGBA)
        let (w, h) = (200u32, 160u32);
        let mut img = crate::assets::Rgba::new(w, h);
        let mut seed = 12345u32;
        for y in 0..h as usize {
            for x in 0..w as usize {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let n = (seed >> 28) as usize;
                let i = (y * w as usize + x) * 4;
                img.px[i] = (40 + x / 3 + n) as u8;
                img.px[i + 1] = (120 + y / 4 + n) as u8;
                img.px[i + 2] = (60 + (x + y) / 8) as u8;
                img.px[i + 3] = 255;
            }
        }
        let tight = super::encode_png_small(&img).unwrap();
        let quick = super::encode_png(&img).unwrap();
        assert_eq!(tight[25], 2, "an opaque picture is written as RGB");
        assert!(tight.len() * 10 < quick.len() * 8, "{} bytes against {}", tight.len(), quick.len());
        assert_eq!(super::decode_png(&tight).unwrap().px, img.px);
        img.px[7] = 128;
        let with_alpha = super::encode_png_small(&img).unwrap();
        assert_eq!(with_alpha[25], 6);
        assert_eq!(super::decode_png(&with_alpha).unwrap().px, img.px);
    }

    #[test]
    fn png_filters() {
        // a 2x2 RGB image, row 0 with the Sub filter, row 1 with Paeth
        let raw = [1u8, 10, 20, 30, 5, 5, 5, 4, 1, 2, 3, 4, 5, 6];
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut chunk = |ty: &[u8], body: &[u8]| {
            png.extend_from_slice(&(body.len() as u32).to_be_bytes());
            png.extend_from_slice(ty);
            png.extend_from_slice(body);
            png.extend_from_slice(&[0; 4]); // the decoder does not check CRCs
        };
        chunk(b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0]);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        let img = super::decode_png(&png).unwrap();
        assert_eq!(&img.px[0..8], &[10, 20, 30, 255, 15, 25, 35, 255]);
        // Paeth: first pixel predicts from above (b), second from the left/above/upper-left
        assert_eq!(&img.px[8..11], &[11, 22, 33]);
    }

    #[test]
    fn every_filter_on_every_kind_of_row() {
        // rows of random bytes under random filters, RGBA and RGB: the decoder's pixels against the filters worked out
        // byte by byte from the PNG specification
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for (ct, bpp) in [(6u8, 4usize), (2, 3)] {
            let (w, h) = (7usize, 9usize);
            let stride = w * bpp;
            let mut raw = Vec::new();
            let mut want = vec![0u8; stride * h];
            for y in 0..h {
                let f = (next() % 5) as u8;
                raw.push(f);
                for i in 0..stride {
                    let x = next() as u8;
                    raw.push(x);
                    let a = if i >= bpp { want[y * stride + i - bpp] } else { 0 };
                    let b = if y > 0 { want[(y - 1) * stride + i] } else { 0 };
                    let c = if i >= bpp && y > 0 { want[(y - 1) * stride + i - bpp] } else { 0 };
                    let p = a as i32 + b as i32 - c as i32;
                    let (pa, pb, pc) = ((p - a as i32).abs(), (p - b as i32).abs(), (p - c as i32).abs());
                    let pred = match f {
                        0 => 0,
                        1 => a,
                        2 => b,
                        3 => ((a as u32 + b as u32) / 2) as u8,
                        _ if pa <= pb && pa <= pc => a,
                        _ if pb <= pc => b,
                        _ => c,
                    };
                    want[y * stride + i] = x.wrapping_add(pred);
                }
            }
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
            let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
            let mut chunk = |ty: &[u8], body: &[u8]| {
                png.extend_from_slice(&(body.len() as u32).to_be_bytes());
                png.extend_from_slice(ty);
                png.extend_from_slice(body);
                png.extend_from_slice(&[0; 4]);
            };
            chunk(b"IHDR", &[0, 0, 0, w as u8, 0, 0, 0, h as u8, 8, ct, 0, 0, 0]);
            chunk(b"IDAT", &z);
            chunk(b"IEND", &[]);
            let img = super::decode_png(&png).unwrap();
            let got: Vec<u8> = img.px.chunks_exact(4).flat_map(|p| p[..bpp].to_vec()).collect();
            assert_eq!(got, want, "colour type {ct}");
        }
    }
}

/// Decodes a picture that is a PNG or a WebP (lossy or lossless, with or without alpha), told apart by their signatures.
/// HD packs may hold WebP pictures (docs/HD.md); everything else is PNG.
pub fn decode_image(d: &[u8]) -> Option<Rgba> {
    if d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        return decode_webp(d);
    }
    decode_png(d)
}

fn decode_webp(d: &[u8]) -> Option<Rgba> {
    let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(d)).ok()?;
    let (w, h) = dec.dimensions();
    let alpha = dec.has_alpha();
    let mut buf = vec![0u8; dec.output_buffer_size()?];
    dec.read_image(&mut buf).ok()?;
    let px = if alpha { buf } else { buf.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect() };
    Some(Rgba { w, h, px })
}
