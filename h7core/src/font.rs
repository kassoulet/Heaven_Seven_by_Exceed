//! The bitmap font (opcode 4, 0x408e82) and the 3x3 blur (opcode 17, 0x409578), ported 1:1 - the blur is a literal transcription of the original's pipelined MMX loop
//! (see re/blur_mmx.py); verified bit-exact against the emulator's glyph surfaces (re/glyph_truth.txt).
use crate::pack::read_int;

// ---- MMX lane helpers (64-bit register = u64, 4 x u16 lanes) ----
fn lanes(x: u64) -> [u16; 4] { [x as u16, (x >> 16) as u16, (x >> 32) as u16, (x >> 48) as u16] }
fn pk(l: [u16; 4]) -> u64 { l[0] as u64 | (l[1] as u64) << 16 | (l[2] as u64) << 32 | (l[3] as u64) << 48 }
fn unpcklbw(x: u64) -> u64 { pk([x as u8 as u16, (x >> 8) as u8 as u16, (x >> 16) as u8 as u16, (x >> 24) as u8 as u16]) }
fn unpckhbw(x: u64) -> u64 { unpcklbw(x >> 32) }
fn paddw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1]), a[2].wrapping_add(b[2]), a[3].wrapping_add(b[3])]) }
fn pmullw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); pk([a[0].wrapping_mul(b[0]), a[1].wrapping_mul(b[1]), a[2].wrapping_mul(b[2]), a[3].wrapping_mul(b[3])]) }
fn pmulhw(a: u64, b: u64) -> u64 { let (a, b) = (lanes(a), lanes(b)); let h = |x: u16, y: u16| (((x as i16 as i32) * (y as i16 as i32)) >> 16) as u16; pk([h(a[0], b[0]), h(a[1], b[1]), h(a[2], b[2]), h(a[3], b[3])]) }
fn packsswb_lo(a: u64) -> u64 { let l = lanes(a); let s = |v: u16| (v as i16).clamp(-128, 127) as i8 as u8 as u64; s(l[0]) | s(l[1]) << 8 | s(l[2]) << 16 | s(l[3]) << 24 }
fn movd(b: &[u8], o: isize) -> u64 { let o = o.max(0) as usize; u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as u64 }

/// 0x409578: in-place 3x3 blur with centre weight `w` (<=100) of an 8-bit surface (`buf`, stride `s`, width `wd`, height `h`; `buf` needs >= 8 bytes of slack at the end).
pub fn blur(buf: &mut [u8], wd: usize, h: usize, s: usize, w: u32) {
    let w = w.min(100); let k = 65536 / (w + 9);
    let rep = |v: u32| pk([v as u16; 4]);
    let (mm6, mm7) = (rep(w), rep(k));
    let mut scr = vec![0u8; 0x1000 + 0x30 * ((wd + 7) / 8 + 2)];
    let sget = |scr: &Vec<u8>, o: usize| u64::from_le_bytes(scr[o..o + 8].try_into().unwrap());
    let sset = |scr: &mut Vec<u8>, o: usize, v: u64| scr[o..o + 8].copy_from_slice(&v.to_le_bytes());
    // pass 1 (row 0): 0x40959a..0x4095f1
    let (mut edi, mut ebx, mut ecx, mut edx) = (0isize, 0isize, 0usize, (wd + 7) >> 2);
    let mut mm0 = movd(buf, ebx) | movd(buf, ebx) << 32; let mut mm2 = movd(buf, edi) | movd(buf, edi) << 32;
    loop {
        let mut mm1 = movd(buf, ebx + 1); let mut mm4 = mm0; mm0 = unpcklbw(mm0); let mut mm3 = movd(buf, edi + 1); mm4 = unpckhbw(mm4);
        let mut mm5 = mm2; mm1 = unpcklbw(mm1); mm0 = paddw(mm0, mm4); mm2 = unpcklbw(mm2); mm0 = paddw(mm0, mm1); mm5 = unpckhbw(mm5);
        mm2 = paddw(mm2, mm5); mm3 = unpcklbw(mm3);
        sset(&mut scr, ecx, mm0); mm2 = paddw(mm2, mm3); sset(&mut scr, ecx + 8, mm2);
        edi += 4; ebx += 4; ecx += 0x18;
        mm0 = (movd(buf, ebx - 1) & 0xffff_ffff) | movd(buf, ebx) << 32; edx -= 1;
        mm2 = movd(buf, edi - 1); mm0 = (mm0 & 0xffff_ffff) | movd(buf, edi) << 32;
        if edx == 0 { break; }
    }
    let mut edi = 0isize;
    for row in 0..h {
        let mut bb = if row != h - 1 && h != 1 { edi + s as isize } else { edi };
        let mut pos = edi;
        let (mut ecx, mut edx, mut esi, mut eax) = (0usize, (wd + 7) >> 3, 5isize, 0isize);
        loop {
            // chunk A
            let mut m0 = movd(buf, pos); let mut m1 = sget(&scr, ecx); m0 = unpcklbw(m0); let mut m2 = movd(buf, bb + 1); m0 = pmullw(m0, mm6);
            m1 = paddw(m1, sget(&scr, ecx + 8)); let mut m5 = movd(buf, bb); m0 = paddw(m0, m1); m1 = movd(buf, bb + eax);
            m2 = unpcklbw(m2); m5 = unpcklbw(m5); m1 = unpcklbw(m1); m5 = paddw(m5, m2); m5 = paddw(m5, m1); m0 = paddw(m0, m5);
            m0 = pmulhw(m0, mm7); m0 = packsswb_lo(m0); sset(&mut scr, ecx + 0x10, m5);
            // chunk B
            let mut b1 = movd(buf, pos + 4); let mut b2 = sget(&scr, ecx + 0x18); b1 = unpcklbw(b1); let mut b3 = movd(buf, bb + esi);
            b1 = pmullw(b1, mm6); b2 = paddw(b2, sget(&scr, ecx + 0x20)); let mut b5 = movd(buf, bb + 4); b1 = paddw(b1, b2); b2 = movd(buf, bb + 3);
            b3 = unpcklbw(b3); b5 = unpcklbw(b5); b2 = unpcklbw(b2); b5 = paddw(b5, b3); b5 = paddw(b5, b2); b1 = paddw(b1, b5);
            b1 = pmulhw(b1, mm7); b1 = packsswb_lo(b1); sset(&mut scr, ecx + 0x28, b5);
            pos += 8; bb += 8; ecx += 0x30;
            let res = (m0 & 0xffff_ffff) | (b1 & 0xffff_ffff) << 32;
            buf[(pos - 8) as usize..pos as usize].copy_from_slice(&res.to_le_bytes());
            if edx == 2 { esi = 4; }
            edx -= 1; eax = -1;
            if edx == 0 { break; }
        }
        let (mut ecx, edx) = (0usize, (wd + 7) >> 3);
        for _ in 0..edx {
            let (a, b, c, d) = (sget(&scr, ecx + 8), sget(&scr, ecx + 0x10), sget(&scr, ecx + 0x20), sget(&scr, ecx + 0x28));
            sset(&mut scr, ecx, a); sset(&mut scr, ecx + 8, b); sset(&mut scr, ecx + 0x18, c); sset(&mut scr, ecx + 0x20, d); ecx += 0x30;
        }
        edi += s as isize;
    }
}

pub struct Glyph { pub code: usize, pub w: usize, pub h: usize, pub stride: usize, pub data: Vec<u8> }

/// The font stream (0x41903c, 4.9 KB, re/font_stream.bin): A bytes per column, then groups {skip, count, count x {width D, A*D column bytes}} until skip < 0.
pub fn load(stream: &[u8]) -> Vec<Glyph> {
    let mut p = 0usize; let a = read_int(stream, &mut p) as usize; let mut code = 0usize; let mut out = Vec::new();
    loop {
        let b = read_int(stream, &mut p); if b < 0 { break; }
        code += b as usize; let c = read_int(stream, &mut p);
        for _ in 0..c {
            let d = read_int(stream, &mut p) as usize; let bits = &stream[p..p + a * d]; p += a * d;
            let (w, h) = (d + 4, 8 * a); let s = (w + 7) & !7;
            let mut img = vec![0u8; s * h + 64];
            for x in 0..d { for by in 0..a { let byte = bits[x * a + by]; for bit in 0..8 { if (byte >> bit) & 1 == 0 { img[(by * 8 + bit + 1) * s + x + 2] = 0x7f; } } } }
            blur(&mut img, w, h, s, 5); img.truncate(s * h);
            out.push(Glyph { code, w, h, stride: s, data: img }); code += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn glyphs_match_the_emulator() {
        let g = load(include_bytes!("../../re/font_stream.bin")); assert_eq!(g.len(), 30);
        let mut n = 0;
        for line in include_str!("../../re/glyph_truth.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let code: usize = f[0].parse().unwrap(); let want: Vec<u8> = (0..f[4].len() / 2).map(|i| u8::from_str_radix(&f[4][2 * i..2 * i + 2], 16).unwrap()).collect();
            let gl = g.iter().find(|x| x.code == code).unwrap();
            assert_eq!((gl.w, gl.h, gl.stride), (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap()));
            assert_eq!(gl.data, want, "glyph {code}"); n += 1;
        }
        assert_eq!(n, 30);
    }
}
