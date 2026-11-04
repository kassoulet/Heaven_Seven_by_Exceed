//! The image-program helper library (0x40d4b1.., ARGB planes of 256x256 dwords), ported 1:1 and verified against the original (re/helper_vectors.txt).
use crate::mmx::*;

/// 0x40d4b1: bilinear sample of an ARGB plane. Position = (bl, bh) = (x, y) in 0..255 with 8-bit wrap; al/ah = the x/y offsets of the second taps; `fx`/`fy` = the 15-bit fractions (ecx/edx).
pub fn sample(plane: &[u32], ebx: u16, eax: u16, fx: u16, fy: u16) -> u32 {
    let (x, y) = ((ebx & 0xff) as u8, (ebx >> 8) as u8); let (dx, dy) = ((eax & 0xff) as u8, (eax >> 8) as u8);
    let at = |x: u8, y: u8| plane[((y as usize) << 8) | x as usize] as u64;
    let (x1, y1) = (x.wrapping_add(dx), y.wrapping_add(dy));
    let (mut m0, mut m1, mut m3, mut m2) = (at(x, y), at(x1, y), at(x1, y1), at(x, y1));
    let m5 = pk([fx; 4]); let m6 = pk([fy; 4]);
    m0 = unpcklbw(m0); m1 = unpcklbw(m1); m2 = unpcklbw(m2); m3 = unpcklbw(m3);
    m0 = psllw(m0, 4); m1 = psllw(m1, 4); m2 = psllw(m2, 4); m3 = psllw(m3, 4);
    m1 = psubw(m1, m0); m3 = psubw(m3, m2); m1 = pmulhw(m1, m5); m3 = pmulhw(m3, m5);
    m1 = psllw(m1, 4); m0 = paddw(m0, m1); m3 = psllw(m3, 4); m2 = paddw(m2, m3);
    m2 = psubw(m2, m0); m2 = pmulhw(m2, m6); m0 = psraw(m0, 4); m0 = paddw(m0, m2);
    packuswb_lo(m0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn sample_matches_the_original() {
        let raw = include_bytes!("../../re/plane.bin"); let plane: Vec<u32> = raw.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let mut n = 0;
        for line in include_str!("../../re/helper_vectors.txt").lines() {
            let f: Vec<&str> = line.split(' ').collect(); if f[0] != "sample" { continue; }
            let p = |i: usize| -> u64 { f[i].parse().unwrap() };
            let got = sample(&plane, p(1) as u16, p(2) as u16, p(3) as u16, p(4) as u16);
            assert_eq!(got as u64, p(5), "sample vector {n}"); n += 1;
        }
        assert_eq!(n, 300);
    }
}
