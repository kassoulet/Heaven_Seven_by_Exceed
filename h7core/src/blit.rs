//! The bilinear tinted glyph blitter (0x40c8d0), literal port: the destination is an 8-bit intensity layer, `rect` = {x, y, w, h} in 16.16 (the sprite is anchored at its
//! bottom-right: it covers x-w..x, y-h..y), `tint_param` = the second dword of the parameter pair (its high word << 2 = the tint), positions step in 1/4096 texel units.
use crate::mmx::*;

pub struct Surf { pub w: i32, pub h: i32, pub pitch: i32, pub data: Vec<u8> }
const C1E0: u64 = 0x7fff_7fff_7fff_7fff;   // 0x42a1e0
const C1F0: u64 = 0x0000_7fff_0000_7fff;   // 0x42a1f0
const C1F8: u64 = 0x0000_0000_7fff_7fff;   // 0x42a1f8

fn rd(v: &[u8], i: i64) -> u32 { if i >= 0 && (i as usize) < v.len() { v[i as usize] as u32 } else { 0 } }

pub fn blit_bilinear(tint_param: i32, rect: [i32; 4], src: &Surf, dst: &mut Surf) {
    if rect[2] <= 0x8000 || rect[3] <= 0x8000 { return; }
    let sp3 = src.pitch;
    let stepx = (((src.w as i64) << 27) / rect[2] as i64) as i32;
    let stepy = (((src.h as i64) << 27) / rect[3] as i64) as i32;
    // vertical clip
    let mut ebx = rect[1].wrapping_sub(rect[3]); let mut eax: i32 = 0xffff;
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edi = ebx; ebx = ebx.wrapping_sub(1); edi = edi.wrapping_add(0xffff); ebx &= 0xffff; edi >>= 16;
    eax = eax.wrapping_sub(ebx);
    let mut sp1 = (((eax as i64) * (stepy as i64)) >> 16) as i32;
    let edx = edi; edi = edi.wrapping_mul(dst.pitch * 4) >> 2;
    let e = dst.h << 16; let mut ebx = rect[3].wrapping_add(rect[1]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx; if ebx <= 0 { return; }
    let mut rows = ebx;
    // horizontal clip
    let mut ebx = rect[0].wrapping_sub(rect[2]); let mut eax: i32 = 0xffff;
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edx = ebx; ebx = ebx.wrapping_sub(1); edx = edx.wrapping_add(0xffff); ebx &= 0xffff; edx >>= 16;
    eax = eax.wrapping_sub(ebx); edi = edi.wrapping_add(edx);
    let sp4 = (((eax as i64) * (stepx as i64)) >> 16) as i32;
    let e = dst.w << 16; let mut ebx = rect[2].wrapping_add(rect[0]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx; if ebx <= 0 { return; }
    let cols = ebx; let skip = dst.pitch - cols;
    let ebp = sp3 as i64;
    let tint = ((((tint_param as u32) >> 16) as u16).wrapping_shl(2)) as u64; let mm4 = tint | tint << 16 | tint << 32 | tint << 48;
    let mut edi = edi as i64;
    while rows > 0 {
        let mut eax = sp1; let mut esi = eax; eax = eax.wrapping_add(stepy); sp1 = eax; eax = esi;
        esi = (esi & !0xfff).wrapping_mul(sp3).wrapping_add(sp4);
        let mut mm6 = eax as u32 as u64; mm6 = psllw(mm6, 3); mm6 = punpcklwd(mm6, mm6); mm6 = punpckldq(mm6, mm6); mm6 &= C1E0; mm6 ^= C1F8;
        let mut eax = ((esi as u32) >> 12) as i64;
        let mut ecx = cols;
        loop {
            let a0 = rd(&src.data, eax + ebp); let a1 = rd(&src.data, eax + ebp + 1);
            let mut mm3 = esi as u32 as u64; mm3 = psllw(mm3, 3); eax -= sp3 as i64; mm3 = punpcklwd(mm3, mm3); mm3 = punpckldq(mm3, mm3); mm3 &= C1E0;
            mm3 ^= C1F0; // pxor mm7
            let b0 = rd(&src.data, eax + ebp); let b1 = rd(&src.data, eax + ebp + 1);
            mm3 = pmulhw(mm3, mm6);
            let edx32 = a1 << 24 | a0 << 16 | b1 << 8 | b0;
            let mut mm0 = unpcklbw_dup(edx32); mm0 = pmulhw(mm0, mm3);
            let dest = rd(&dst.data, edi) as u8; let e = ((dest as u32) << 24) as i32 >> 8;
            mm0 = pmaddwd(mm0, mm4);
            let mm1 = e as u32 as u64;
            esi = esi.wrapping_add(stepx);
            let mut mm2 = mm0; mm0 >>= 32; mm2 = paddd(mm2, mm1); eax = ((esi as u32) >> 12) as i64; mm2 = paddd(mm2, mm0);
            mm2 = psrad(mm2, 16); edi += 1; mm2 = packsswb_same(mm2);
            ecx -= 1;
            if edi - 1 >= 0 && ((edi - 1) as usize) < dst.data.len() { dst.data[(edi - 1) as usize] = mm2 as u8; }
            if ecx == 0 { break; }
        }
        edi += skip as i64; rows -= 1;
    }
}

/// The nearest-neighbour sprite blitter 0x40cace (same entry convention as 0x40c8d0; only the high word of the second parameter dword matters: tint = hw << 7 per 16-bit lane). Literal: 4 pixels per
/// iteration (bytes -> words b*257, pmulhw tint, psraw 7, paddsb into the destination dword), then a remainder loop that reuses the stale edx (its low byte and upper word feed the multiply: a quirk, reproduced).
pub fn blit_nearest(p1: i32, rect: [i32; 4], src: &Surf, dst: &mut Surf) {
    if rect[2] <= 0x8000 || rect[3] <= 0x8000 { return; }
    let sp = src.pitch as i32;
    let stepx = (((src.w as i64) << 27) / rect[2] as i64) as i32;
    let stepy = (((src.h as i64) << 27) / rect[3] as i64) as i32;
    let mut ebx = rect[1].wrapping_sub(rect[3]); let mut eax: i32 = 0xffff;
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edi = ebx; ebx = ebx.wrapping_sub(1); edi = edi.wrapping_add(0xffff); ebx &= 0xffff; edi >>= 16;
    eax = eax.wrapping_sub(ebx);
    let mut ypos = (((eax as i64) * (stepy as i64)) >> 16) as i32;
    let edx0 = edi; edi = edi.wrapping_mul(dst.pitch * 4) >> 2;
    let e = dst.h << 16; let mut ebx = rect[3].wrapping_add(rect[1]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx0; if ebx <= 0 { return; }
    let mut rows = ebx;
    let mut ebx = rect[0].wrapping_sub(rect[2]); let mut eax: i32 = 0xffff;
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edx = ebx; ebx = ebx.wrapping_sub(1); edx = edx.wrapping_add(0xffff); ebx &= 0xffff; edx >>= 16;
    eax = eax.wrapping_sub(ebx); edi = edi.wrapping_add(edx);
    let xstart = (((eax as i64) * (stepx as i64)) >> 16) as i32;
    let e = dst.w << 16; let mut ebx = rect[2].wrapping_add(rect[0]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx; if ebx <= 0 { return; }
    let cols = ebx; let skip = dst.pitch - cols;
    let t = (((p1 as u32) >> 16) as u16).wrapping_shl(7) as u64; let mm4 = t | t << 16 | t << 32 | t << 48;
    let mut edi = edi as i64; let mut edx = edx as u32;
    let dget = |d: &Surf, i: i64| -> u8 { if i >= 0 && (i as usize) < d.data.len() { d.data[i as usize] } else { 0 } };
    while rows > 0 {
        let mut esi = ypos; ypos = ypos.wrapping_add(stepy);
        esi = (esi & !0xfff).wrapping_mul(sp).wrapping_add(xstart);
        let mut eax = esi as u32 >> 12; esi = esi.wrapping_add(stepx);
        let mut ecx = cols - 4;
        let s = |i: u32| rd(&src.data, i as i64) as u32;
        if ecx >= 0 { loop {
            let mut p = [0u32; 4];
            for k in 0..4 { p[k] = s(eax); eax = esi as u32 >> 12; esi = esi.wrapping_add(stepx); }
            // edx = p0 | p1<<8 | p2<<16 | p3<<24 (shl 16 / rol 16 sequence)
            edx = p[0] | p[1] << 8 | p[2] << 16 | p[3] << 24;
            let d0 = (0..4).fold(0u64, |a, k| a | (dget(dst, edi + k) as u64) << (8 * k));
            let mm1 = psraw(pmulhw(unpcklbw_dup(edx), mm4), 7);
            let r = paddsb(d0, packsswb_lo(mm1));
            for k in 0..4 { if edi + k >= 0 && ((edi + k) as usize) < dst.data.len() { dst.data[(edi + k) as usize] = (r >> (8 * k)) as u8; } }
            edi += 4; ecx -= 4; if ecx <= 0 { break; }
        } }
        ecx += 4;
        while ecx != 0 {
            edx = (edx & !0xff00) | s(eax) << 8; let mm0 = pmulhw(edx as u64, mm4);
            edx = (edx & !0xff00) | (dget(dst, edi) as u32) << 8; let mm1 = psraw(edx as u64, 1);
            let mm0 = psraw(paddsw(mm0, mm1), 7); edx = packsswb_same(mm0) as u32;
            eax = esi as u32 >> 12; esi = esi.wrapping_add(stepx);
            if edi >= 0 && (edi as usize) < dst.data.len() { dst.data[edi as usize] = edx as u8; } edi += 1; ecx -= 1;
        }
        edi += skip as i64; rows -= 1;
    }
}

/// The rotated sprite blitter 0x40ccc5 (entry convention as the others, the angle (f32 bits) comes in ebx; only the high word of the second parameter dword matters: tint = hw << 2).
/// Destination = the bounding box (X = w|cos| + h|sin|, Y = h|cos| + w|sin| half extents, rounded, in 16.16); per destination pixel the source position is stepped by (s cos, s sin) per x and
/// (-t sin, t cos) per row (s,t = the nearest-blitter steps); a pixel is only drawn when 0 <= x < 64 texels and 0 <= row < 256 - the source is addressed with a FIXED pitch of 64 bytes (it is meant
/// for 64-wide sprite textures; `src.pitch` is ignored) - and replaces the destination byte with the bilinear (weights from 15-bit fractions; row +0x40 is the second row) tinted value.
pub fn blit_rotated(angle: f32, p1: i32, rect: [i32; 4], src: &Surf, dst: &mut Surf) {
    if rect[2] <= 0x8000 || rect[3] <= 0x8000 { return; }
    let (sn, cs) = ((angle as f64).sin(), (angle as f64).cos()); let (asn, acs) = (sn.abs(), cs.abs());
    let (w, h) = (rect[2] as f32, rect[3] as f32);
    let m = |a: f32, b: f64| (a as f64 * b) as f32;
    let xb = (m(w, acs) + m(h, asn)).round_ties_even() as i32;
    let yb = (m(h, acs) + m(w, asn)).round_ties_even() as i32;
    let mut pos = [(src.w << 11), (src.h << 11)];
    let s = (((src.w as i64) << 27) / rect[2] as i64) as i32; let t = (((src.h as i64) << 27) / rect[3] as i64) as i32;
    let rnd = |v: i32, f: f64| ((v as f64 * f) as f32).round_ties_even() as i32;
    let sstep = [rnd(s, cs), rnd(s, sn)];
    let rstep = [((-((t as f64 * sn) as f32)).round_ties_even()) as i32, rnd(t, cs)];
    let mut eax: i32 = 0xffff - yb; let mut ebx = rect[1].wrapping_sub(yb);
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edi = ebx; ebx = ebx.wrapping_sub(1); edi = edi.wrapping_add(0xffff); ebx &= 0xffff; edi >>= 16;
    eax = eax.wrapping_sub(ebx);
    let mulsh = |a: i32, b: i32| (((a as i64) * (b as i64)) >> 16) as i32;
    pos[0] = pos[0].wrapping_add(mulsh(eax, rstep[0])); pos[1] = pos[1].wrapping_add(mulsh(eax, rstep[1]));
    let edx0 = edi; edi = edi.wrapping_mul(dst.pitch * 4) >> 2;
    let e = dst.h << 16; let mut ebx = yb.wrapping_add(rect[1]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx0; if ebx <= 0 { return; }
    let mut rows = ebx;
    let mut eax: i32 = 0xffff - xb; let mut ebx = rect[0].wrapping_sub(xb);
    if ebx < 0 { eax = eax.wrapping_sub(ebx); ebx = 0; }
    let mut edx = ebx; ebx = ebx.wrapping_sub(1); edx = edx.wrapping_add(0xffff); ebx &= 0xffff; edx >>= 16;
    eax = eax.wrapping_sub(ebx); edi = edi.wrapping_add(edx);
    pos[0] = pos[0].wrapping_add(mulsh(eax, sstep[0])); pos[1] = pos[1].wrapping_add(mulsh(eax, sstep[1]));
    let e = dst.w << 16; let mut ebx = xb.wrapping_add(rect[0]); if ebx > e { ebx = e; }
    ebx = ebx.wrapping_add(0xffff) >> 16; ebx -= edx; if ebx <= 0 { return; }
    let cols = ebx; let skip = dst.pitch - cols;
    let t4 = (((p1 as u32) >> 16) as u16).wrapping_shl(2) as u64; let mm4 = t4 | t4 << 16 | t4 << 32 | t4 << 48;
    let mut edi = edi as i64;
    let pack2 = |a: i32, b: i32| a as u32 as u64 | (b as u32 as u64) << 32;
    let mut row0 = pack2(pos[0], pos[1]);
    let (step, rstp) = (pack2(sstep[0], sstep[1]), pack2(rstep[0], rstep[1]));
    while rows > 0 {
        let mut mm6 = row0; row0 = paddd(row0, rstp);
        for _ in 0..cols {
            let x = mm6 as u32; let y6 = (mm6 >> 38) as u32;
            if x & 0xfc0000 == 0 && y6 & 0xffff_c000 == 0 {
                let mm1 = pslld3(mm6) & 0x7fff_7fff_7fff_7fff;
                let (fx, fy) = ((mm1 & 0xffff) as u64, (mm1 >> 32 & 0xffff) as u64);
                let b = pk([fy as u16; 4]) ^ 0x0000_0000_7fff_7fff; let a = pk([fx as u16; 4]) ^ 0x0000_7fff_0000_7fff;
                let wgt = pmulhw(b, a);
                let idx = ((x >> 12) & 0x3f) as i64 + (y6 & 0x3fc0) as i64;
                let e = (rd(&src.data, idx) | rd(&src.data, idx + 1) << 8 | rd(&src.data, idx + 0x40) << 16 | rd(&src.data, idx + 0x41) << 24) as u32;
                let mut mm0 = pmulhw(unpcklbw_dup(e), wgt); mm0 = pmaddwd(mm0, mm4);
                let mm1 = paddd(mm0, mm0 >> 32); let r = packsswb_same(psrad(mm1, 16));
                if edi >= 0 && (edi as usize) < dst.data.len() { dst.data[edi as usize] = r as u8; }
            }
            mm6 = paddd(mm6, step); edi += 1;
        }
        edi += skip as i64; rows -= 1;
    }
}
fn pslld3(x: u64) -> u64 { (((x as u32) << 3) as u64) | ((((x >> 32) as u32) << 3) as u64) << 32 }
/// punpcklbw mm0,mm0: each of the four low bytes duplicated into a 16-bit lane (b*257)
fn unpcklbw_dup(x: u32) -> u64 { let b = |i: u32| ((x >> (8 * i)) & 0xff) as u16 * 257; pk([b(0), b(1), b(2), b(3)]) }

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    #[test] fn matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/blit_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let p = |i: usize| -> i64 { f[i].parse().unwrap() };
            let (sw, sh, dw, dh) = (p(0) as i32, p(1) as i32, p(2) as i32, p(3) as i32);
            let src = Surf { w: sw, h: sh, pitch: (sw + 7) & !7, data: hex(f[10]) };
            let mut dst = Surf { w: dw, h: dh, pitch: dw, data: hex(f[11]) };
            blit_bilinear(p(5) as i32, [p(6) as i32, p(7) as i32, p(8) as i32, p(9) as i32], &src, &mut dst);
            assert_eq!(dst.data, hex(f[12]), "vector {n}"); n += 1;
        }
        assert_eq!(n, 300);
    }
    #[test] fn nearest_matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/blit_nearest_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let p = |i: usize| -> i64 { f[i].parse().unwrap() };
            let (sw, sh, dw, dh) = (p(0) as i32, p(1) as i32, p(2) as i32, p(3) as i32);
            let src = Surf { w: sw, h: sh, pitch: (sw + 7) & !7, data: hex(f[10]) };
            let mut dst = Surf { w: dw, h: dh, pitch: dw, data: hex(f[11]) };
            blit_nearest(p(5) as i32, [p(6) as i32, p(7) as i32, p(8) as i32, p(9) as i32], &src, &mut dst);
            assert_eq!(dst.data, hex(f[12]), "nearest vector {n}"); n += 1;
        }
        assert_eq!(n, 300);
    }
    #[test] fn rotated_matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/blit_rot_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let p = |i: usize| -> i64 { f[i].parse().unwrap() };
            let (sw, sh, dw, dh) = (p(0) as i32, p(1) as i32, p(2) as i32, p(3) as i32);
            let mut x = p(10) as u32; let data: Vec<u8> = (0..64 * 256 + 0x100).map(|_| { x = x.wrapping_mul(1664525).wrapping_add(1013904223); ((x >> 24) & 0x7f) as u8 }).collect();
            let src = Surf { w: sw, h: sh, pitch: 64, data };
            let mut dst = Surf { w: dw, h: dh, pitch: dw, data: hex(f[11]) };
            blit_rotated(f32::from_bits(p(4) as u32), p(5) as i32, [p(6) as i32, p(7) as i32, p(8) as i32, p(9) as i32], &src, &mut dst);
            assert_eq!(dst.data, hex(f[12]), "rotated vector {n}"); n += 1;
        }
        assert_eq!(n, 300);
    }
}
