//! The text renderer 0x409cda (class 46 instances), literal port: lays out a string of glyph records on the 8-bit layer, using the bilinear blitter.
use crate::blit::{blit_bilinear, Surf};

/// one glyph record: three mip levels (full, half, quarter) as in the original's 0x600-byte record (objects at +0, +0x200, +0x400)
pub struct GlyphRec { pub mips: [Surf; 3] }
/// the viewport/layer object (class 3, handler 0x408211): the 8-bit layer plus its scale constants (+0x2c = w/4, +0x30 = w/2, +0x34 = 65536.0, +0x38 = 65536*h/w)
pub struct Layer { pub surf: Surf, pub f2c: f32, pub f30: f32, pub f34: f32, pub f38: f32 }
/// the text object: +0x28/+0x2c position, +0x34 scale, +0x38 spacing, +0x3c alpha, +0x40 flags (bit 0: force drawing even with the 't' flag), +0x44.. glyph list
pub struct Text { pub x: f32, pub y: f32, pub scale: f32, pub spacing: f32, pub alpha: i32, pub force: bool, pub flag_t: bool }

const K1: f32 = 1.08;    // 0x429eda
const K2: f32 = 0.535;   // 0x429ed6
fn fist(x: f32) -> i32 { if x.is_finite() && x.abs() < 2147483648.0 { x.round_ties_even() as i32 } else { i32::MIN } }

pub fn render_text(t: &Text, glyphs: &[&GlyphRec], layer: &mut Layer) {
    if !t.force && t.flag_t { return; }   // test [esi+0x40],1 -> draw; else the command-line flag 't' ([ebp+0x18]) suppresses it
    let alpha = if t.alpha <= 1 { return } else if t.alpha >= 0x100 { 0xff } else { t.alpha };
    // total width S (fldz; per glyph: w*scale*f2c added; spacing*f2c between glyphs)
    let mut s = 0f32;
    for (i, g) in glyphs.iter().enumerate() {
        let w = g.mips[0].w as f32;
        s += (w * t.scale) * layer.f2c;
        if i + 1 < glyphs.len() { s += t.spacing * layer.f2c; }
    }
    let first = glyphs[0];
    let mut x = fist(((t.x + layer.f34) * layer.f30) - s);
    let hf = ((first.mips[0].h as f32 * t.scale) * layer.f2c) * K1;
    let h = fist(hf);
    let y = fist(((-t.y + layer.f38) * layer.f30) - hf * K2);
    let tint_param = alpha << 16;
    for g in glyphs {
        let a = (g.mips[0].w as f32 * t.scale) * layer.f2c;
        let xa = x as f32 + a;
        let w = fist(a * K1);
        x = fist(xa);
        if g.mips[0].data.len() > 0 {
            let mut lvl = 0;
            if ((h >> 16) as i16) < 15 { lvl = 1; if ((h >> 16) as i16) < 5 { lvl = 2; } }
            blit_bilinear(tint_param, [x, y, w, h], &g.mips[lvl], &mut layer.surf);
        }
        let adv = (t.spacing * layer.f30) + a;
        x = fist(adv + x as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    fn u32_at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) }
    fn f32_at(b: &[u8], o: usize) -> f32 { f32::from_bits(u32_at(b, o)) }
    /// re/text_capture.txt: line 1 obj hex, 2 view hex, 3 layer "w h pitch_raw before after" hex, then per glyph pointer: "ptr mips..." 
    #[test] fn please_wait_matches_the_emulator() {
        let cap = include_str!("../../re/text_capture.txt"); let mut ln = cap.lines();
        let obj = hex(ln.next().unwrap()); let view = hex(ln.next().unwrap());
        let l: Vec<&str> = ln.next().unwrap().split(' ').collect();
        let (lw, lh, lp): (i32, i32, i32) = (l[0].parse().unwrap(), l[1].parse().unwrap(), l[2].parse().unwrap());
        let (before, after) = (hex(l[3]), hex(l[4]));
        let list: Vec<usize> = ln.next().unwrap().split(' ').map(|x| x.parse().unwrap()).collect();   // glyph record index per char
        let mut recs: Vec<GlyphRec> = Vec::new();
        for line in ln { let f: Vec<&str> = line.split(' ').collect();   // 3 mips: "w h pitch_raw data" x3
            let m = |k: usize| Surf { w: f[4 * k].parse().unwrap(), h: f[4 * k + 1].parse().unwrap(), pitch: f[4 * k + 2].parse::<i32>().unwrap() >> 2, data: hex(f[4 * k + 3]) };
            recs.push(GlyphRec { mips: [m(0), m(1), m(2)] }); }
        let gl: Vec<&GlyphRec> = list.iter().map(|&i| &recs[i]).collect();
        let t = Text { x: f32_at(&obj, 0x28), y: f32_at(&obj, 0x2c), scale: f32_at(&obj, 0x34), spacing: f32_at(&obj, 0x38), alpha: u32_at(&obj, 0x3c) as i32, force: obj[0x40] & 1 != 0, flag_t: false };
        let mut layer = Layer { surf: Surf { w: lw, h: lh, pitch: lp >> 2, data: before }, f2c: f32_at(&view, 0x2c), f30: f32_at(&view, 0x30), f34: f32_at(&view, 0x34), f38: f32_at(&view, 0x38) };
        render_text(&t, &gl, &mut layer);
        let n = (lw * lh) as usize; let diff = (0..n).filter(|&i| layer.surf.data[i] != after[i]).count();
        assert_eq!(diff, 0, "{diff} of {n} layer pixels differ");
    }
}
