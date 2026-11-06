//! The texture-script ops that wrap the image program and the 8-bit/32-bit surface objects (0x408279 op7, 0x40828d op20, 0x409ffc op25, 0x40a320 op27, 0x4082b9 op6 + the ported filter ops),
//! replayed on an object table keyed by object index (ptr = ebp + 0x1e0 + idx*0x200). Verified op by op against the original's surfaces (re/tex_expect.txt, from re/h7emu.py with H7TEX=1).
use crate::blit::{blit_bilinear, Surf};
use crate::imgprog::Vm;
use crate::pack::read_int;
use crate::texops::*;
use std::collections::HashMap;

/// texels per row of an 8-bit surface (0x40b27b: ((w+7)&~7)*4 bytes >> 2)
pub fn pitch8(w: usize) -> usize { (w + 7) & !7 }

/// A texture object: +0x18 w, +0x1c h, +0x28 8-bit data (stride pitch8(w)), +0x24 32-bit data (stride pitch8(w) dwords, 0x40b251).
#[derive(Default, Clone)]
pub struct Obj { pub w: usize, pub h: usize, pub d8: Vec<u8>, pub d32: Vec<u32> }
impl Obj {
    /// 0x40b27b: allocate an 8-bit surface (zeroed; (h+1) rows + slack)
    pub fn alloc8(&mut self, w: usize, h: usize) { self.w = w; self.h = h; if self.d8.is_empty() { self.d8 = vec![0; pitch8(w) * (h + 1) + 16]; } }
    pub fn alloc32(&mut self, w: usize, h: usize) { self.w = w; self.h = h; if self.d32.is_empty() { self.d32 = vec![0; pitch8(w) * (h + 1) + 1]; } }
    /// the visible w x h bytes of the 8-bit surface
    pub fn vis8(&self) -> Vec<u8> { let p = pitch8(self.w); (0..self.h).flat_map(|y| self.d8[y * p..y * p + self.w].to_vec()).collect() }
    pub fn vis32(&self) -> Vec<u8> { let p = pitch8(self.w); (0..self.h).flat_map(|y| self.d32[y * p..y * p + self.w].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).collect() }
}

/// op 7 (0x408279): 256x256 32-bit surface = the image program's output; returns the stream bytes consumed.
pub fn op7(o: &mut Obj, s: &[u8]) -> usize { o.alloc32(256, 256); let (out, n) = Vm::new().run_len(s); o.d32[..out.len()].copy_from_slice(&out); n }

/// op 20 (0x40828d): 256x256 8-bit surface = (low byte of the image program's output) >> 1 (`lodsd; shr al,1; mov [ebx],al`).
pub fn op20(o: &mut Obj, s: &[u8]) -> usize { o.alloc8(256, 256); let (out, n) = Vm::new().run_len(s); for (i, v) in out.iter().enumerate() { o.d8[i] = (*v as u8) >> 1; } n }

/// op 25 (0x409ffc): emboss/edge pass over the low bytes of a 256x256 32-bit source into a new 256x256 32-bit surface. Literal: dx = 0xfefe + pixel index (16-bit, so the window
/// start (dh:dl) picks up the carry from dl: rows are offset by one for columns >= 2), the vertical term r1 = sum of 5 taps of row dh - 5 taps of row dh+4 (x dl..dl+4) and the horizontal
/// term r2 = sum of 5 taps of column dl (rows dh..dh+4) - 5 taps of column dl+4, all 8-bit wrapped coordinates, 16-bit sums; stored dword = r2 << 16 | r1.
pub fn op25(src: &[u32]) -> Vec<u32> {
    let g = |dh: u8, dl: u8| (src[(dh as usize) << 8 | dl as usize] & 0xff) as u16;
    let mut out = vec![0u32; 65536]; let mut dx: u16 = 0xfefe;
    for o in out.iter_mut() {
        let (dl, dh) = (dx as u8, (dx >> 8) as u8);
        let mut a = 0u16; for i in 0..5u8 { a = a.wrapping_add(g(dh, dl.wrapping_add(i))); }
        for i in 0..5u8 { a = a.wrapping_sub(g(dh.wrapping_add(4), dl.wrapping_add(4 - i))); }
        let mut b = 0u16; for i in 0..5u8 { b = b.wrapping_add(g(dh.wrapping_add(4 - i), dl)); }
        for i in 0..5u8 { b = b.wrapping_sub(g(dh.wrapping_add(i), dl.wrapping_add(4))); }
        *o = (b as u32) << 16 | a as u32; dx = dx.wrapping_add(1);
    }
    out
}

/// op 6 (0x4082b9): 1-bit bitmap -> 8-bit surface (0x7f for set bits). Stream: w, h (read_int), then per 8-row block (from the bottom, row index h-8r-bit going UP, bit 0 first) and per 8-column
/// block 8 bytes (one per column, bit i = row i of the block). Rows that would fall before the surface start (h not a multiple of 8) are dropped. Returns the bytes consumed (after the two ints).
pub fn op6(o: &mut Obj, s: &[u8], p: &mut usize) {
    let w = read_int(s, p) as usize; let h = read_int(s, p) as usize;
    let hpad = ((h + 7) & !7) + 0x80; o.w = w; o.h = hpad; o.alloc8(w, hpad); o.h = h;
    let pi = pitch8(w) as isize; let (bw, bh) = ((w + 7) >> 3, (h + 7) >> 3);
    for r in 0..bh { for c in 0..bw { for col in 0..8 {
        let byte = s[*p]; *p += 1;
        for bit in 0..8 { if byte >> bit & 1 != 0 {
            let row = h as isize - 8 * r as isize - bit as isize; let x = (8 * c + col) as isize;
            let i = row * pi + x; if i >= 0 && (i as usize) < o.d8.len() { o.d8[i as usize] = 0x7f; }
        } }
    } } }
}

/// op 27 (0x40a320): resize to (w, h) (read_int each; negative = current size * [ebp+0x138] / 0x140). Equal size: nothing. Otherwise a new zeroed surface is allocated and, when the old one has data,
/// the bilinear blitter (tint parameter 0x00ff00ff, rect = centre/half-size (w,h)<<15) scales the old surface into it.
pub fn op27(o: &mut Obj, s: &[u8], p: &mut usize, g138: u32) {
    let mut a = read_int(s, p); if a < 0 { a = ((o.w as u64 * g138 as u64) / 0x140) as i32; }
    let mut b = read_int(s, p); if b < 0 { b = ((o.h as u64 * g138 as u64) / 0x140) as i32; }
    let (w, h) = (a as usize, b as usize); if w == o.w && h == o.h { return; }
    let mut dst = Surf { w: w as i32, h: h as i32, pitch: pitch8(w) as i32, data: vec![0; pitch8(w) * (h + 1) + 16] };
    if !o.d8.is_empty() {
        let src = Surf { w: o.w as i32, h: o.h as i32, pitch: pitch8(o.w) as i32, data: o.d8.clone() };
        blit_bilinear(0x00ff_00ff, [(w as i32) << 15, (h as i32) << 15, (w as i32) << 15, (h as i32) << 15], &src, &mut dst);
    }
    o.w = w; o.h = h; o.d8 = dst.data;
}

pub struct Ctx { pub objs: HashMap<usize, Obj>, pub g138: u32 }
impl Ctx {
    pub fn new() -> Ctx { Ctx { objs: HashMap::new(), g138: 0x140 } }
    /// run one script op (handler address as in the original) on object `cur`, stream `s` at `*p` (just after the opcode byte); returns nothing, advances `*p` like the original's edi.
    pub fn run_op(&mut self, handler: u32, cur: usize, s: &[u8], p: &mut usize) {
        let mut o = self.objs.remove(&cur).unwrap_or_default();
        match handler {
            0x408279 => { *p += op7(&mut o, &s[*p..]); }
            0x40828d => { *p += op20(&mut o, &s[*p..]); }
            0x409ffc => { let i = read_int(s, p) as usize; let src = &self.objs[&i]; o.alloc32(256, 256); o.d32 = op25(&src.d32); o.d32.push(0); }
            0x4082b9 => op6(&mut o, s, p),
            0x40a320 => op27(&mut o, s, p, self.g138),
            0x409c0f => { let mut d = Obj::default(); let (w, h) = (o.w / 2, o.h / 2); d.alloc8(w, h); let dp = pitch8(w); down2(&o.d8, o.w, o.h, pitch8(o.w), &mut d.d8, dp); self.objs.insert(cur + 1, d); }
            0x409574 => { let w = s[*p] as u32; *p += 1; let pi = pitch8(o.w); crate::font::blur(&mut o.d8, o.w, o.h, pi, w); }
            0x40a42a => { let i = read_int(s, p) as usize; let src = &self.objs[&i]; copy_tiled(&mut o.d8, o.w, o.h, &src.d8); }
            0x40a3c3 => { let i = read_int(s, p) as usize; let src = &self.objs[&i]; mul_tiled(&mut o.d8, o.w, o.h, &src.d8); }
            0x40a468 => { let i = read_int(s, p) as usize; let src = &self.objs[&i]; light_tiled(&mut o.d8, o.w, o.h, &src.d8); }
            0x409c7c => { let pi = pitch8(o.w); let g = 2 * pi; let mut buf = vec![0u8; g]; buf.extend_from_slice(&o.d8); buf.extend(vec![0u8; g]); crack(&mut buf, g, o.w, o.h, pi); o.d8 = buf[g..g + o.d8.len()].to_vec(); }
            h => panic!("op handler {h:#x} not modelled"),
        }
        self.objs.insert(cur, o);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fnv(b: &[u8]) -> u64 { b.iter().fold(0xcbf29ce484222325u64, |h, &c| (h ^ c as u64).wrapping_mul(0x100000001b3)) }
    /// replay the captured texture-script ops from the original's own script bytes and compare the visible surface of the current object after every op (FNV-1a) with the emulator's
    #[test] fn script_ops_match_the_original() {
        let img = include_bytes!("../../re/image.bin");
        let mut cx = Ctx::new(); let (mut n, mut bad) = (0, vec![]);
        for line in include_str!("../../re/tex_expect.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let handler = u32::from_str_radix(&f[1][2..], 16).unwrap(); let idx: usize = f[2].parse().unwrap();
            let (a, e) = (usize::from_str_radix(f[3], 16).unwrap(), usize::from_str_radix(f[4], 16).unwrap());
            if matches!(handler, 0x40832b) { continue; }
            let mut p = a - 0x400000; cx.run_op(handler, idx, img, &mut p);
            let o = &cx.objs[&idx]; let (w, h): (usize, usize) = (f[6].parse().unwrap(), f[7].parse().unwrap());
            let ok_end = p == e - 0x400000;
            let ok8 = f[8] == "-" || fnv(&o.vis8()).to_string() == f[8]; let ok32 = f[9] == "-" || fnv(&o.vis32()).to_string() == f[9];
            if !(ok_end && ok8 && ok32 && o.w == w && o.h == h) { bad.push(format!("#{} {} end={} 8={} 32={} dims {}x{} want {w}x{h}", f[0], f[1], ok_end, ok8, ok32, o.w, o.h)); }
            n += 1;
        }
        assert!(bad.is_empty(), "{} of {n} ops differ: {:#?}", bad.len(), bad);
    }
}
