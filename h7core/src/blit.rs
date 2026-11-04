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
}
