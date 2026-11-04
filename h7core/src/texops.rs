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
}
