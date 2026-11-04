//! Texture operators on 8-bit surfaces (ported from the original's op handlers, verified with re/texop_vectors.txt).
use std::collections::VecDeque;

/// opcode 24 / 0x409c7c "crack growth": every non-zero texel becomes (23 * d) & 0xff where d = the distance, through non-zero memory cells, to the nearest zero cell, moving +-1 and +-pitch in
/// *linear memory* (the original's depth-limited recursive search, bound 256, via the callback 0x409cb6, does exactly that: a texel at the right edge sees the next row's first texel as its right
/// neighbour when pitch == width, and the zero guard rows above/below end the search). `buf` is the memory around the surface, `base` the index of texel (0,0); cells outside `buf` count as zero.
/// (`cmp al,0x7f; jng` is a *signed* compare, so the intended clamp at 0x7f never triggers - the values wrap.)
pub fn crack(buf: &mut [u8], base: usize, w: usize, h: usize, pitch: usize) {
    let n = buf.len() as isize; let mut dist = vec![u16::MAX; buf.len()]; let mut q = VecDeque::new();
    let step = [1isize, -1, pitch as isize, -(pitch as isize)];
    // sources: nonzero cells that touch a zero cell or the outside get distance 1; zero cells are distance 0 (implicit)
    for i in 0..n { if buf[i as usize] != 0 {
        if step.iter().any(|&d| { let j = i + d; j < 0 || j >= n || buf[j as usize] == 0 }) { dist[i as usize] = 1; q.push_back(i); } } }
    while let Some(i) = q.pop_front() { let d = dist[i as usize];
        for &s in &step { let j = i + s; if j >= 0 && j < n && buf[j as usize] != 0 && dist[j as usize] > d + 1 { dist[j as usize] = d + 1; q.push_back(j); } } }
    for y in 0..h { for x in 0..w { let o = base + y * pitch + x; if buf[o] != 0 { let d = dist[o] as u32; buf[o] = if d >= 256 { 0 } else { ((23 * d) & 0xff) as u8 }; } } }
}


/// opcode 23 / 0x409c0f (inner part 0x409c21): 2x2 box downsample to (w/2, h/2). Each output = (sum of the 2x2 block + 2) >> 2 computed with the original's *8-bit* adds: the first texel is sign-extended (movsx), the others are added
/// to al with the carry going into ah (adc ah,0), then `add eax,2; sar eax,2`, store al - which only differs from the plain average for texels >= 0x80 (our glyph/rock data is 7-bit, but the vectors cover the full byte range).
pub fn down2(src: &[u8], w: usize, h: usize, sp: usize, dst: &mut [u8], dp: usize) {
    let (ow, oh) = (w / 2, h / 2);
    for y in 0..oh { for x in 0..ow {
        let o = 2 * y * sp + 2 * x; let b = [src[o], src[o + 1], src[o + sp], src[o + sp + 1]];
        let mut eax = b[0] as i8 as i32 as u32;
        for &v in &b[1..] { let al = (eax & 0xff) + v as u32; let carry = al >> 8; eax = (eax & !0xff) | (al & 0xff); let ah = ((eax >> 8) & 0xff) + carry; eax = (eax & !0xff00) | ((ah & 0xff) << 8); }
        eax = eax.wrapping_add(2); dst[y * dp + x] = ((eax as i32) >> 2) as u8;
    } }
}

/// opcode 26 / 0x40a3c3: multiply the surface by another 256x256 texture (tiled, starting at row 128): d = min(127, (d * s) >> 8) (pmullw, psrlw 8, packsswb = signed saturation to 127).
pub fn mul_tiled(dst: &mut [u8], w: usize, h: usize, src: &[u8]) {
    for y in 0..h { let row = (128 + y) & 255; for x in 0..w { let s = src[(row << 8) | (x & 255)] as u32; let d = dst[y * w + x] as u32; dst[y * w + x] = ((d * s) >> 8).min(127) as u8; } }
}
/// opcode 30 / 0x40a42a: copy a 256x256 texture into the surface (tiled, from row 0).
pub fn copy_tiled(dst: &mut [u8], w: usize, h: usize, src: &[u8]) {
    for y in 0..h { for x in 0..w { dst[y * w + x] = src[((y & 255) << 8) | (x & 255)]; } }
}

/// opcode 28 / 0x40a468: shaded blend of a tiled 256x256 texture into the surface (vertical ramp: the multiplier `edx` starts at 3 and grows by 4 per row up to 0xa3 while more than 40 rows remain, then falls by 4 per row;
/// horizontally a per-pixel ramp mm5 = edx * [1,0,2,3] stepping by 4*edx): per 8 pixels, with t = (d ^ 0x7f) widened and u = src widened,
/// out = paddsb(d, packsswb(((u + (r>>12)) * (t + (r>>9))) *hi r >> 7)) - the original's exact 16-bit lane arithmetic.
pub fn light_tiled(dst: &mut [u8], w: usize, h: usize, src: &[u8]) {
    use crate::mmx::*;
    let mut edx: i32 = 3;
    for y in 0..h {
        let remaining = (h - y) as i32;
        if remaining > 0x28 { if edx < 0xa3 { edx += 4; } } else { edx -= 4; }
        let m6 = pk([edx as u16; 4]); let mut mm5 = pmullw(m6, 0x0003_0002_0000_0001); let mm4 = psllw(m6, 2);
        let row = y & 255;
        for xc in 0..w / 8 {
            let o = y * w + xc * 8; let sx = (xc * 8) & 255;
            let s8 = u64::from_le_bytes(src[(row << 8) + sx..(row << 8) + sx + 8].try_into().unwrap());
            let d8 = u64::from_le_bytes(dst[o..o + 8].try_into().unwrap());
            let (mut mm0, mut mm1) = (unpcklbw(s8), unpckhbw(s8));
            let x = d8 ^ 0x7f7f_7f7f_7f7f_7f7f; let (mut mm2, mut mm3) = (unpcklbw(x), unpckhbw(x));
            let mut mm7 = psrlw(mm5, 9); mm2 = paddw(mm2, mm7); mm7 = psrlw(mm7, 3); mm0 = paddw(mm0, mm7);
            mm0 = pmullw(mm0, mm2); mm0 = pmulhw(mm0, mm5); mm5 = paddw(mm5, mm4);
            mm7 = psrlw(mm5, 9); mm3 = paddw(mm3, mm7); mm7 = psrlw(mm7, 3); mm1 = paddw(mm1, mm7);
            mm1 = pmullw(mm1, mm3); mm1 = pmulhw(mm1, mm5); mm5 = paddw(mm5, mm4);
            mm0 = psrlw(mm0, 7); mm1 = psrlw(mm1, 7); let p = packsswb(mm0, mm1);
            dst[o..o + 8].copy_from_slice(&paddsb(d8, p).to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    #[test] fn crack_matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/texop_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] == "down2" { continue; } if f[0] != "crack" { continue; }
            let (w, h, p): (usize, usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap());
            let mut d = hex(f[4]); let want = hex(f[5]);
            // data starts 2 guard rows in
            crack(&mut d, 2 * p, w, h, p);
            if d != want { let diffs: Vec<(usize, usize, u8, u8)> = (0..d.len()).filter(|&i| d[i] != want[i]).take(6).map(|i| ((i / p).wrapping_sub(2), i % p, d[i], want[i])).collect(); eprintln!("vec {n} w={w} h={h} p={p} diffs(y,x,mine,theirs) {:?} total {}", diffs, (0..d.len()).filter(|&i| d[i] != want[i]).count()); }
            assert_eq!(d, want, "vector {n}"); n += 1;
        }
        assert_eq!(n, 80);
    }

    #[test] fn down2_matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/texop_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] != "down2" { continue; }
            let (w, h, sp, dp): (usize, usize, usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap());
            let src = hex(f[5]); let want = hex(f[6]); let mut dst = vec![0u8; want.len()];
            down2(&src, w, h, sp, &mut dst, dp);
            for y in 0..h / 2 { for x in 0..w / 2 { assert_eq!(dst[y * dp + x], want[y * dp + x], "vector {n} at {x},{y}"); } } n += 1;
        }
        assert_eq!(n, 80);
    }

    #[test] fn mul_and_copy_match_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/texop_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] != "mul" && f[0] != "copy" { continue; }
            let (w, h): (usize, usize) = (f[2].parse().unwrap(), f[3].parse().unwrap()); let src = hex(f[4]); let mut d = hex(f[5]); let want = hex(f[6]);
            if f[0] == "mul" { mul_tiled(&mut d, w, h, &src) } else { copy_tiled(&mut d, w, h, &src) }
            assert_eq!(d, want, "{} vector {n}", f[0]); n += 1;
        }
        assert_eq!(n, 40);
    }

    #[test] fn light_matches_the_original() {
        let mut n = 0;
        for line in include_str!("../../re/texop_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] != "light" { continue; }
            let (w, h): (usize, usize) = (f[2].parse().unwrap(), f[3].parse().unwrap()); let src = hex(f[4]); let mut d = hex(f[5]); let want = hex(f[6]);
            light_tiled(&mut d, w, h, &src); assert_eq!(d, want, "light vector {n}"); n += 1;
        }
        assert_eq!(n, 20);
    }
}
